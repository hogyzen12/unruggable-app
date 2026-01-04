
// src/signing/hardware.rs
use crate::signing::TransactionSigner;
use crate::hardware::HardwareWallet;
use async_trait::async_trait;
use std::error::Error;
use std::sync::Arc;

#[derive(Clone)]
pub struct HardwareSigner {
    wallet: Arc<HardwareWallet>,
}

impl HardwareSigner {
    /// Create a new hardware signer and attempt to connect
    pub async fn new() -> Result<Self, Box<dyn Error>> {
        let wallet = Arc::new(HardwareWallet::new());
        wallet.connect().await?;
        Ok(Self { wallet })
    }
    
    /// Create a hardware signer from an existing wallet
    pub fn from_wallet(wallet: Arc<HardwareWallet>) -> Self {
        Self { wallet }
    }
}

#[async_trait]
impl TransactionSigner for HardwareSigner {
    async fn get_public_key(&self) -> Result<String, Box<dyn Error>> {
        self.wallet.get_public_key().await
    }
    
    async fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("📤 HARDWARE WALLET - UNSIGNED TRANSACTION DETAILS");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        // Log message details
        println!("📊 Message Length: {} bytes", message.len());
        println!("");
        
        // Print message in hex format (chunked for readability)
        println!("🔢 Raw Message (Hex):");
        for (i, chunk) in message.chunks(32).enumerate() {
            let hex_string: String = chunk.iter()
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<_>>()
                .join(" ");
            println!("   [{:04}] {}", i * 32, hex_string);
        }
        println!("");
        
        // Attempt to parse as VersionedMessage to show transaction structure
        use solana_sdk::message::VersionedMessage;
        match bincode::deserialize::<VersionedMessage>(message) {
            Ok(versioned_msg) => {
                println!("✅ Successfully parsed VersionedMessage:");
                println!("");
                
                match &versioned_msg {
                    VersionedMessage::Legacy(legacy_msg) => {
                        println!("📋 Message Type: Legacy");
                        println!("   Required Signatures: {}", legacy_msg.header.num_required_signatures);
                        println!("   Readonly Signed Accounts: {}", legacy_msg.header.num_readonly_signed_accounts);
                        println!("   Readonly Unsigned Accounts: {}", legacy_msg.header.num_readonly_unsigned_accounts);
                        println!("   Number of Instructions: {}", legacy_msg.instructions.len());
                        println!("   Number of Account Keys: {}", legacy_msg.account_keys.len());
                        println!("");
                        
                        println!("📝 Account Keys:");
                        for (i, key) in legacy_msg.account_keys.iter().enumerate() {
                            let role = if i == 0 {
                                "Fee Payer"
                            } else if i < legacy_msg.header.num_required_signatures as usize {
                                "Signer"
                            } else {
                                "Account"
                            };
                            println!("   [{}] {} ({})", i, key, role);
                        }
                        println!("");
                        
                        println!("🔧 Instructions:");
                        for (i, ix) in legacy_msg.instructions.iter().enumerate() {
                            let program_key = &legacy_msg.account_keys[ix.program_id_index as usize];
                            println!("   Instruction {}:", i + 1);
                            println!("      Program: {}", program_key);
                            println!("      Accounts: {} account indices", ix.accounts.len());
                            println!("      Data: {} bytes", ix.data.len());
                        }
                        println!("");
                        
                        println!("🔗 Recent Blockhash: {}", legacy_msg.recent_blockhash);
                    }
                    VersionedMessage::V0(v0_msg) => {
                        println!("📋 Message Type: V0 (with Address Lookup Tables)");
                        println!("   Required Signatures: {}", v0_msg.header.num_required_signatures);
                        println!("   Readonly Signed Accounts: {}", v0_msg.header.num_readonly_signed_accounts);
                        println!("   Readonly Unsigned Accounts: {}", v0_msg.header.num_readonly_unsigned_accounts);
                        println!("   Number of Instructions: {}", v0_msg.instructions.len());
                        println!("   Number of Account Keys: {}", v0_msg.account_keys.len());
                        println!("   Address Table Lookups: {}", v0_msg.address_table_lookups.len());
                        println!("");
                        
                        println!("📝 Static Account Keys:");
                        for (i, key) in v0_msg.account_keys.iter().enumerate() {
                            let role = if i == 0 {
                                "Fee Payer"
                            } else if i < v0_msg.header.num_required_signatures as usize {
                                "Signer"
                            } else {
                                "Account"
                            };
                            println!("   [{}] {} ({})", i, key, role);
                        }
                        println!("");
                        
                        println!("🔧 Instructions:");
                        for (i, ix) in v0_msg.instructions.iter().enumerate() {
                            if (ix.program_id_index as usize) < v0_msg.account_keys.len() {
                                let program_key = &v0_msg.account_keys[ix.program_id_index as usize];
                                println!("   Instruction {}:", i + 1);
                                println!("      Program: {}", program_key);
                                println!("      Accounts: {} account indices", ix.accounts.len());
                                println!("      Data: {} bytes", ix.data.len());
                            }
                        }
                        println!("");
                        
                        println!("🔗 Recent Blockhash: {}", v0_msg.recent_blockhash);
                    }
                }
            }
            Err(e) => {
                println!("⚠️  Could not parse as VersionedMessage: {}", e);
                println!("   This might be a raw message hash or different format");
            }
        }
        
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("🔐 Sending to hardware wallet for signature...");
        println!("");
        
        // For Solana transactions, the message is already the serialized transaction
        // We need to sign it directly and return the signature
        let signature = self.wallet.sign_message(message).await?;
        
        println!("✅ Hardware wallet returned signature: {} bytes", signature.len());
        println!("");
        
        // Ensure the signature is exactly 64 bytes
        if signature.len() != 64 {
            return Err(format!("Invalid signature length: expected 64, got {}", signature.len()).into());
        }
        
        Ok(signature)
    }
    
    fn get_name(&self) -> String {
        "Hardware Wallet".to_string()
    }
    
    async fn is_available(&self) -> bool {
        self.wallet.is_connected().await
    }
}