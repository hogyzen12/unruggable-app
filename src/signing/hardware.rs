#![allow(dead_code)]

use crate::hardware::HardwareWallet;
use crate::signing::TransactionSigner;
use async_trait::async_trait;
use std::error::Error;
use std::sync::Arc;

#[derive(Clone)]
pub struct HardwareSigner {
    wallet: Arc<HardwareWallet>,
}

impl HardwareSigner {
    pub async fn new() -> Result<Self, Box<dyn Error>> {
        let wallet = Arc::new(HardwareWallet::new());
        wallet.connect().await?;
        wallet
            .get_public_key()
            .await
            .map_err(|e| format!("Hardware wallet is connected but not ready for signing: {e}"))?;
        Ok(Self { wallet })
    }

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
        let signature = self.wallet.sign_message(message).await?;
        if signature.len() != 64 {
            return Err(format!(
                "Invalid signature length: expected 64, got {}",
                signature.len()
            )
            .into());
        }
        Ok(signature)
    }

    fn get_name(&self) -> String {
        "Hardware Wallet".to_string()
    }

    async fn is_available(&self) -> bool {
        self.wallet.is_connected().await
    }

    fn is_hardware(&self) -> bool {
        true
    }
}
