use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tokio::sync::oneshot;
use crate::bridge::protocol::{BridgeRequest, BridgeResponse};
use crate::wallet::Wallet;
use crate::storage;
use solana_client::rpc_client::RpcClient;
use solana_sdk::transaction::VersionedTransaction;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq)]
pub struct PendingBridgeRequest {
    pub id: u64,
    pub origin: String,
    pub request: BridgeRequest,
}

/// Shared state for the bridge handler
pub struct BridgeHandler {
    current_wallet: Arc<Mutex<Option<Wallet>>>,
    pending_requests: Arc<Mutex<Vec<PendingBridgeRequest>>>,
    pending_waiters: Arc<Mutex<HashMap<u64, oneshot::Sender<BridgeResponse>>>>,
    enabled: Arc<AtomicBool>,
}

impl BridgeHandler {
    pub fn new() -> Self {
        Self {
            current_wallet: Arc::new(Mutex::new(None)),
            pending_requests: Arc::new(Mutex::new(Vec::new())),
            pending_waiters: Arc::new(Mutex::new(HashMap::new())),
            enabled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Load wallet with PIN
    pub fn load_wallet_with_pin(&self, pin: &str) -> Result<(), Box<dyn std::error::Error>> {
        println!("🔐 BridgeHandler: load_wallet_with_pin called");

        let wallets = storage::load_wallets_from_storage();
        println!("📂 BridgeHandler: Found {} wallets", wallets.len());

        if wallets.is_empty() {
            return Err("No wallets found".into());
        }

        // Use first wallet
        let wallet_info = &wallets[0];
        println!("👛 BridgeHandler: Loading wallet: {}", wallet_info.name);

        // Verify PIN
        println!("🔑 BridgeHandler: Verifying PIN");
        let _salt = storage::verify_pin(pin)?;
        println!("✓ BridgeHandler: PIN verified");

        // Load wallet - the PIN verification is just for UI lock,
        // wallets are stored as base58 keypairs, not encrypted with PIN
        println!("👛 BridgeHandler: Loading wallet from storage");
        let wallet = Wallet::from_wallet_info(wallet_info)?;
        println!("✓ BridgeHandler: Wallet loaded successfully");

        let mut current = self.current_wallet.lock().unwrap();
        *current = Some(wallet);
        println!("✅ BridgeHandler: Wallet stored in bridge handler");

        Ok(())
    }

    /// Check if wallet is loaded
    pub fn is_wallet_loaded(&self) -> bool {
        let wallet = self.current_wallet.lock().unwrap();
        wallet.is_some()
    }

    /// Update the current wallet (for wallet switching)
    pub fn update_wallet(&self, wallet: Wallet) {
        println!("🔄 BridgeHandler: Updating wallet to {}", wallet.name);
        let mut current = self.current_wallet.lock().unwrap();
        *current = Some(wallet);
        println!("✅ BridgeHandler: Wallet updated successfully");
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
        println!("🔌 BridgeHandler: Enabled set to {}", enabled);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Get current wallet public key (for checking if wallet changed)
    pub fn get_current_pubkey(&self) -> Option<String> {
        let wallet = self.current_wallet.lock().unwrap();
        wallet.as_ref().map(|w| w.get_public_key())
    }

    /// Load wallet without PIN (for non-PIN protected wallets)
    pub fn load_wallet_no_pin(&self) -> Result<(), Box<dyn std::error::Error>> {
        let wallets = storage::load_wallets_from_storage();

        if wallets.is_empty() {
            return Err("No wallets found".into());
        }

        if storage::has_pin() {
            return Err("Wallet is PIN-protected".into());
        }

        let wallet_info = &wallets[0];
        let wallet = Wallet::from_wallet_info(wallet_info)?;

        let mut current = self.current_wallet.lock().unwrap();
        *current = Some(wallet);

        Ok(())
    }

    pub fn pending_requests(&self) -> Vec<PendingBridgeRequest> {
        let pending = self.pending_requests.lock().unwrap();
        pending.clone()
    }

    pub async fn approve_request(&self, id: u64) -> Result<(), String> {
        let request = {
            let pending = self.pending_requests.lock().unwrap();
            pending.iter().find(|req| req.id == id).cloned()
        };

        let request = request.ok_or_else(|| "Request not found".to_string())?;

        let response = match request.request {
            BridgeRequest::SignTransaction { transaction, .. } => {
                self.sign_transaction(&transaction).await
            }
            BridgeRequest::SignAndSendTransaction { transaction, .. } => {
                self.sign_and_send_transaction(&transaction).await
            }
            BridgeRequest::SignMessage { message, .. } => {
                self.sign_message(&message).await
            }
            _ => BridgeResponse::Error {
                message: "Unsupported request".to_string(),
            },
        };

        self.resolve_request(id, response)
    }

    pub async fn reject_request(&self, id: u64, reason: String) -> Result<(), String> {
        self.resolve_request(id, BridgeResponse::Rejected { reason })
    }

    fn resolve_request(&self, id: u64, response: BridgeResponse) -> Result<(), String> {
        {
            let mut pending = self.pending_requests.lock().unwrap();
            pending.retain(|req| req.id != id);
        }

        let sender = {
            let mut waiters = self.pending_waiters.lock().unwrap();
            waiters.remove(&id)
        };

        if let Some(sender) = sender {
            let _ = sender.send(response);
            Ok(())
        } else {
            Err("Pending request channel missing".to_string())
        }
    }

    /// Handle incoming bridge requests
    pub async fn handle_request(&self, request: BridgeRequest) -> BridgeResponse {
        match request {
            BridgeRequest::Ping => {
                println!("📡 Bridge: Received ping");
                BridgeResponse::Pong
            },

            BridgeRequest::Connect { origin } => {
                if !self.is_enabled() {
                    return BridgeResponse::Error {
                        message: "Browser extension is disabled in settings.".to_string(),
                    };
                }
                println!("🔗 Bridge: Connect request from {}", origin);

                // Check if wallet is loaded
                let is_loaded = self.is_wallet_loaded();
                println!("🔍 Bridge: Wallet loaded status: {}", is_loaded);

                // Only connect if wallet is already unlocked
                let wallet = self.current_wallet.lock().unwrap();

                match wallet.as_ref() {
                    Some(w) => {
                        let pubkey = w.get_public_key();
                        println!("✅ Bridge: Connected with pubkey {}", pubkey);
                        BridgeResponse::Connected {
                            public_key: pubkey
                        }
                    },
                    None => {
                        println!("⚠️  Bridge: Wallet not unlocked (current_wallet is None)");
                        BridgeResponse::Error {
                            message: "Please unlock your Unruggable desktop app first.".to_string()
                        }
                    }
                }
            },

            BridgeRequest::SignTransaction { origin, transaction } => {
                if !self.is_enabled() {
                    return BridgeResponse::Error {
                        message: "Browser extension is disabled in settings.".to_string(),
                    };
                }
                println!("✍️  Bridge: Sign transaction request from {}", origin);
                if !self.is_wallet_loaded() {
                    return BridgeResponse::Error {
                        message: "No wallet loaded. Please unlock your desktop wallet first.".to_string(),
                    };
                }
                self.enqueue_pending_request(BridgeRequest::SignTransaction { origin, transaction }).await
            },

            BridgeRequest::SignAndSendTransaction { origin, transaction } => {
                if !self.is_enabled() {
                    return BridgeResponse::Error {
                        message: "Browser extension is disabled in settings.".to_string(),
                    };
                }
                println!("✍️  Bridge: Sign and send transaction request from {}", origin);
                if !self.is_wallet_loaded() {
                    return BridgeResponse::Error {
                        message: "No wallet loaded. Please unlock your desktop wallet first.".to_string(),
                    };
                }
                self.enqueue_pending_request(BridgeRequest::SignAndSendTransaction { origin, transaction }).await
            },

            BridgeRequest::SignMessage { origin, message } => {
                if !self.is_enabled() {
                    return BridgeResponse::Error {
                        message: "Browser extension is disabled in settings.".to_string(),
                    };
                }
                println!("✍️  Bridge: Sign message request from {}", origin);
                if !self.is_wallet_loaded() {
                    return BridgeResponse::Error {
                        message: "No wallet loaded. Please unlock your desktop wallet first.".to_string(),
                    };
                }
                self.enqueue_pending_request(BridgeRequest::SignMessage { origin, message }).await
            },

            BridgeRequest::Disconnect { origin } => {
                if !self.is_enabled() {
                    return BridgeResponse::Error {
                        message: "Browser extension is disabled in settings.".to_string(),
                    };
                }
                println!("👋 Bridge: Disconnect request from {}", origin);
                // Just acknowledge, we don't actually disconnect the wallet
                BridgeResponse::Error {
                    message: "OK".to_string()
                }
            },

            BridgeRequest::GetPublicKey => {
                if !self.is_enabled() {
                    return BridgeResponse::Error {
                        message: "Browser extension is disabled in settings.".to_string(),
                    };
                }
                println!("🔑 Bridge: Get public key request");

                let wallet = self.current_wallet.lock().unwrap();

                match wallet.as_ref() {
                    Some(w) => {
                        let pubkey = w.get_public_key();
                        let wallet_name = w.name.clone();
                        println!("✅ Bridge: Returning public key: {} ({})", pubkey, wallet_name);
                        BridgeResponse::PublicKey {
                            public_key: pubkey,
                            wallet_name,
                        }
                    },
                    None => {
                        println!("⚠️  Bridge: Wallet not unlocked");
                        BridgeResponse::Error {
                            message: "Wallet is locked. Please unlock your Unruggable desktop app.".to_string()
                        }
                    }
                }
            },

            BridgeRequest::GetBalance => {
                if !self.is_enabled() {
                    return BridgeResponse::Error {
                        message: "Browser extension is disabled in settings.".to_string(),
                    };
                }
                println!("💰 Bridge: Get balance request");

                let wallet = self.current_wallet.lock().unwrap();

                match wallet.as_ref() {
                    Some(w) => {
                        let pubkey = w.get_public_key();
                        drop(wallet); // Release lock before RPC call

                        // Get RPC URL from storage or use default
                        let rpc_url = storage::load_rpc_from_storage()
                            .unwrap_or_else(|| "https://johna-k3cr1v-fast-mainnet.helius-rpc.com".to_string());

                        // Create RPC client and fetch balance
                        let client = RpcClient::new(rpc_url);

                        match solana_sdk::pubkey::Pubkey::from_str(&pubkey) {
                            Ok(pubkey_parsed) => {
                                match client.get_balance(&pubkey_parsed) {
                                    Ok(lamports) => {
                                        let balance = lamports as f64 / 1_000_000_000.0; // Convert lamports to SOL
                                        println!("✅ Bridge: Balance: {} SOL", balance);
                                        BridgeResponse::Balance { balance }
                                    },
                                    Err(e) => {
                                        println!("❌ Bridge: Failed to fetch balance: {}", e);
                                        BridgeResponse::Error {
                                            message: format!("Failed to fetch balance: {}", e)
                                        }
                                    }
                                }
                            },
                            Err(e) => {
                                BridgeResponse::Error {
                                    message: format!("Invalid public key: {}", e)
                                }
                            }
                        }
                    },
                    None => {
                        println!("⚠️  Bridge: Wallet not unlocked");
                        BridgeResponse::Error {
                            message: "Wallet is locked. Please unlock your Unruggable desktop app.".to_string()
                        }
                    }
                }
            },
        }
    }

    async fn enqueue_pending_request(&self, request: BridgeRequest) -> BridgeResponse {
        static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);
        let id = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let origin = match &request {
            BridgeRequest::SignTransaction { origin, .. } => origin.clone(),
            BridgeRequest::SignAndSendTransaction { origin, .. } => origin.clone(),
            BridgeRequest::SignMessage { origin, .. } => origin.clone(),
            _ => "unknown".to_string(),
        };

        let (tx, rx) = oneshot::channel();

        {
            let mut pending = self.pending_requests.lock().unwrap();
            pending.push(PendingBridgeRequest { id, origin, request: request.clone() });
        }
        {
            let mut waiters = self.pending_waiters.lock().unwrap();
            waiters.insert(id, tx);
        }

        match rx.await {
            Ok(response) => response,
            Err(_) => BridgeResponse::Error {
                message: "Signing request cancelled".to_string(),
            },
        }
    }

    async fn sign_transaction(&self, transaction: &str) -> BridgeResponse {
        let wallet_guard = self.current_wallet.lock().unwrap();
        let wallet = match wallet_guard.as_ref() {
            Some(w) => w,
            None => {
                return BridgeResponse::Error {
                    message: "No wallet loaded. Please unlock your desktop wallet first.".to_string(),
                };
            }
        };

        let tx_bytes = match bs58::decode(transaction).into_vec() {
            Ok(bytes) => bytes,
            Err(e) => {
                return BridgeResponse::Error {
                    message: format!("Invalid transaction encoding: {}", e),
                };
            }
        };

        use solana_sdk::{pubkey::Pubkey, signature::Signature, transaction::VersionedTransaction};
        let mut versioned_tx = match bincode::deserialize::<VersionedTransaction>(&tx_bytes) {
            Ok(tx) => tx,
            Err(e) => {
                return BridgeResponse::Error {
                    message: format!("Failed to deserialize transaction: {}", e),
                };
            }
        };

        let message_bytes = versioned_tx.message.serialize();
        let signature = wallet.sign_message(&message_bytes);
        let signature_bytes = signature.to_bytes();
        let signature_preview = bs58::encode(&signature_bytes).into_string();
        println!("✅ Bridge: Transaction signed with signature: {}...", &signature_preview[..20]);

        let signer_pubkey = match Pubkey::from_str(&wallet.get_public_key()) {
            Ok(key) => key,
            Err(e) => {
                return BridgeResponse::Error {
                    message: format!("Invalid wallet pubkey: {}", e),
                };
            }
        };

        let required_signers = versioned_tx.message.header().num_required_signatures as usize;
        let signer_index = versioned_tx.message.static_account_keys()
            .iter()
            .take(required_signers)
            .position(|key| key == &signer_pubkey);

        let signer_index = match signer_index {
            Some(index) => index,
            None => {
                return BridgeResponse::Error {
                    message: "Wallet pubkey not found among required signers.".to_string(),
                };
            }
        };

        if versioned_tx.signatures.len() != required_signers {
            versioned_tx.signatures = vec![Signature::default(); required_signers];
        }

        versioned_tx.signatures[signer_index] = Signature::from(signature_bytes);
        println!("✅ Bridge: Signature inserted into transaction");

        let signed_tx_bytes = match bincode::serialize(&versioned_tx) {
            Ok(bytes) => bytes,
            Err(e) => {
                return BridgeResponse::Error {
                    message: format!("Failed to serialize signed transaction: {}", e),
                };
            }
        };
        let signed_tx_base58 = bs58::encode(&signed_tx_bytes).into_string();
        println!("📦 Signed transaction encoded (length: {})", signed_tx_bytes.len());

        BridgeResponse::TransactionSigned {
            signature: signature_preview,
            signed_transaction: signed_tx_base58,
        }
    }

    async fn sign_and_send_transaction(&self, transaction: &str) -> BridgeResponse {
        let signed_response = match self.sign_transaction(transaction).await {
            BridgeResponse::TransactionSigned { signed_transaction, .. } => signed_transaction,
            other => return other,
        };

        let signed_tx_bytes = match bs58::decode(&signed_response).into_vec() {
            Ok(bytes) => bytes,
            Err(e) => {
                return BridgeResponse::Error {
                    message: format!("Failed to decode signed transaction: {}", e),
                };
            }
        };

        let versioned_tx: VersionedTransaction = match bincode::deserialize(&signed_tx_bytes) {
            Ok(tx) => tx,
            Err(e) => {
                return BridgeResponse::Error {
                    message: format!("Failed to deserialize signed transaction: {}", e),
                };
            }
        };

        let rpc_url = storage::load_rpc_from_storage()
            .unwrap_or_else(|| "https://johna-k3cr1v-fast-mainnet.helius-rpc.com".to_string());
        println!("🌐 Bridge: Using RPC: {}", rpc_url);

        let client = RpcClient::new(rpc_url);

        match client.send_transaction(&versioned_tx) {
            Ok(sig) => {
                let sig_string = sig.to_string();
                println!("✅ Bridge: Transaction sent successfully!");
                println!("🔗 On-chain Signature: {}", sig_string);

                BridgeResponse::TransactionSigned {
                    signature: sig_string,
                    signed_transaction: signed_response,
                }
            }
            Err(e) => {
                println!("❌ Bridge: Failed to send transaction: {}", e);
                BridgeResponse::Error {
                    message: format!("Transaction send failed: {}", e),
                }
            }
        }
    }

    async fn sign_message(&self, message: &str) -> BridgeResponse {
        let wallet = self.current_wallet.lock().unwrap();
        let wallet = match wallet.as_ref() {
            Some(w) => w,
            None => {
                return BridgeResponse::Error {
                    message: "No wallet loaded. Please unlock your desktop wallet first.".to_string(),
                };
            }
        };

        match bs58::decode(message).into_vec() {
            Ok(msg_bytes) => {
                let signature = wallet.sign_message(&msg_bytes);
                let sig_bytes = signature.to_bytes();
                let sig_base58 = bs58::encode(&sig_bytes).into_string();

                println!("✅ Bridge: Message signed");
                BridgeResponse::MessageSigned { signature: sig_base58 }
            }
            Err(e) => BridgeResponse::Error {
                message: format!("Invalid message encoding: {}", e),
            },
        }
    }
}
