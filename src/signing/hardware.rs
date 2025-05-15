
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
        // For Solana transactions, the message is already the serialized transaction
        // We need to sign it directly and return the signature
        let signature = self.wallet.sign_message(message).await?;
        
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