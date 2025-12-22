use std::sync::{Arc, Mutex};
use crate::bridge::protocol::{BridgeRequest, BridgeResponse};
use crate::wallet::Wallet;
use crate::storage;
use solana_client::rpc_client::RpcClient;
use solana_sdk::signature::Signature;
use std::str::FromStr;

/// Shared state for the bridge handler
pub struct BridgeHandler {
    current_wallet: Arc<Mutex<Option<Wallet>>>,
}

impl BridgeHandler {
    pub fn new() -> Self {
        Self {
            current_wallet: Arc::new(Mutex::new(None)),
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

    /// Handle incoming bridge requests
    pub fn handle_request(&self, request: BridgeRequest) -> BridgeResponse {
        match request {
            BridgeRequest::Ping => {
                println!("📡 Bridge: Received ping");
                BridgeResponse::Pong
            },

            BridgeRequest::Connect { origin } => {
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
                println!("✍️  Bridge: Sign and send transaction request from {}", origin);

                let wallet = self.current_wallet.lock().unwrap();

                match wallet.as_ref() {
                    Some(w) => {
                        // Decode base58 transaction
                        match bs58::decode(&transaction).into_vec() {
                            Ok(mut tx_bytes) => {
                                // Sign the transaction message
                                let signature_base58 = w.sign_transaction(&tx_bytes);
                                println!("✅ Bridge: Transaction signed with signature: {}...", &signature_base58[..20]);

                                // Decode the signature from base58
                                let signature_bytes = match bs58::decode(&signature_base58).into_vec() {
                                    Ok(bytes) => bytes,
                                    Err(e) => {
                                        return BridgeResponse::Error {
                                            message: format!("Failed to decode signature: {}", e)
                                        };
                                    }
                                };

                                if signature_bytes.len() != 64 {
                                    return BridgeResponse::Error {
                                        message: format!("Invalid signature length: {} (expected 64)", signature_bytes.len())
                                    };
                                }

                                // Insert the signature into the transaction at position 1
                                if tx_bytes.len() < 65 {
                                    return BridgeResponse::Error {
                                        message: "Transaction too short".to_string()
                                    };
                                }

                                // Copy signature into the transaction (at byte 1, after the signature count)
                                tx_bytes[1..65].copy_from_slice(&signature_bytes[..64]);
                                println!("✅ Bridge: Signature inserted into transaction");

                                // Get RPC URL from storage or use default
                                let rpc_url = storage::load_rpc_from_storage()
                                    .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
                                println!("🌐 Bridge: Using RPC: {}", rpc_url);

                                // Create RPC client and send transaction
                                let client = RpcClient::new(rpc_url);

                                // Create a versioned transaction from the bytes
                                use solana_sdk::transaction::VersionedTransaction;
                                let versioned_tx = match bincode::deserialize::<VersionedTransaction>(&tx_bytes) {
                                    Ok(tx) => tx,
                                    Err(e) => {
                                        return BridgeResponse::Error {
                                            message: format!("Failed to deserialize transaction: {}", e)
                                        };
                                    }
                                };

                                // Send the transaction
                                match client.send_transaction(&versioned_tx) {
                                    Ok(sig) => {
                                        let sig_string = sig.to_string();
                                        println!("✅ Bridge: Transaction sent successfully!");
                                        println!("🔗 On-chain Signature: {}", sig_string);

                                        // Also encode the full signed transaction for the extension
                                        let signed_tx_base58 = bs58::encode(&tx_bytes).into_string();
                                        println!("📦 Signed transaction encoded (length: {})", tx_bytes.len());

                                        BridgeResponse::TransactionSigned {
                                            signature: sig_string,
                                            signed_transaction: signed_tx_base58,
                                        }
                                    },
                                    Err(e) => {
                                        println!("❌ Bridge: Failed to send transaction: {}", e);
                                        BridgeResponse::Error {
                                            message: format!("Transaction send failed: {}", e)
                                        }
                                    }
                                }
                            },
                            Err(e) => {
                                BridgeResponse::Error {
                                    message: format!("Invalid transaction encoding: {}", e)
                                }
                            }
                        }
                    },
                    None => {
                        BridgeResponse::Error {
                            message: "No wallet loaded. Please unlock your desktop wallet first.".to_string()
                        }
                    }
                }
            },

            BridgeRequest::SignMessage { origin, message } => {
                println!("✍️  Bridge: Sign message request from {}", origin);

                let wallet = self.current_wallet.lock().unwrap();

                match wallet.as_ref() {
                    Some(w) => {
                        // Decode base58 message
                        match bs58::decode(&message).into_vec() {
                            Ok(msg_bytes) => {
                                // Sign the message
                                let signature = w.sign_message(&msg_bytes);
                                let sig_bytes = signature.to_bytes();
                                let sig_base58 = bs58::encode(&sig_bytes).into_string();

                                println!("✅ Bridge: Message signed");
                                BridgeResponse::MessageSigned {
                                    signature: sig_base58
                                }
                            },
                            Err(e) => {
                                BridgeResponse::Error {
                                    message: format!("Invalid message encoding: {}", e)
                                }
                            }
                        }
                    },
                    None => {
                        BridgeResponse::Error {
                            message: "No wallet loaded. Please unlock your desktop wallet first.".to_string()
                        }
                    }
                }
            },

            BridgeRequest::Disconnect { origin } => {
                println!("👋 Bridge: Disconnect request from {}", origin);
                // Just acknowledge, we don't actually disconnect the wallet
                BridgeResponse::Error {
                    message: "OK".to_string()
                }
            },
        }
    }
}
