// src/signing/software.rs
use crate::wallet::Wallet;
use crate::signing::TransactionSigner;
use async_trait::async_trait;
use std::error::Error;

#[derive(Clone)]
pub struct SoftwareSigner {
    wallet: Wallet,
}

impl SoftwareSigner {
    pub fn new(wallet: Wallet) -> Self {
        Self { wallet }
    }
}

#[async_trait]
impl TransactionSigner for SoftwareSigner {
    async fn get_public_key(&self) -> Result<String, Box<dyn Error>> {
        Ok(self.wallet.get_public_key())
    }
    
    async fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        let signature = self.wallet.sign_message(message);
        Ok(signature.to_bytes().to_vec())
    }
    
    fn get_name(&self) -> String {
        format!("Software Wallet: {}", self.wallet.name)
    }
    
    async fn is_available(&self) -> bool {
        true // Software wallet is always available
    }
}