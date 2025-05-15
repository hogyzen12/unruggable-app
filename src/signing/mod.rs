// src/signing/mod.rs
use crate::wallet::Wallet;
use std::error::Error;
use async_trait::async_trait;

pub mod software;
pub mod hardware;

use software::SoftwareSigner;
use hardware::HardwareSigner;

/// Trait for different transaction signing methods
#[async_trait]
pub trait TransactionSigner: Send + Sync {
    /// Get the public key of the signer
    async fn get_public_key(&self) -> Result<String, Box<dyn Error>>;
    
    /// Sign a message/transaction
    async fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, Box<dyn Error>>;
    
    /// Get a display name for the signing method
    fn get_name(&self) -> String;
    
    /// Check if the signer is available/connected
    async fn is_available(&self) -> bool;
}

/// Enum to hold different signer types
#[derive(Clone)]
pub enum SignerType {
    Software(SoftwareSigner),
    Hardware(HardwareSigner),
}

impl SignerType {
    /// Create a software signer from a wallet
    pub fn from_wallet(wallet: Wallet) -> Self {
        SignerType::Software(SoftwareSigner::new(wallet))
    }
    
    /// Create a hardware signer (attempts to connect)
    pub async fn hardware() -> Result<Self, Box<dyn Error>> {
        let signer = HardwareSigner::new().await?;
        Ok(SignerType::Hardware(signer))
    }
}

#[async_trait]
impl TransactionSigner for SignerType {
    async fn get_public_key(&self) -> Result<String, Box<dyn Error>> {
        match self {
            SignerType::Software(s) => s.get_public_key().await,
            SignerType::Hardware(h) => h.get_public_key().await,
        }
    }
    
    async fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        match self {
            SignerType::Software(s) => s.sign_message(message).await,
            SignerType::Hardware(h) => h.sign_message(message).await,
        }
    }
    
    fn get_name(&self) -> String {
        match self {
            SignerType::Software(s) => s.get_name(),
            SignerType::Hardware(h) => h.get_name(),
        }
    }
    
    async fn is_available(&self) -> bool {
        match self {
            SignerType::Software(s) => s.is_available().await,
            SignerType::Hardware(h) => h.is_available().await,
        }
    }
}