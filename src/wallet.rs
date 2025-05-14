// src/wallet.rs
use ed25519_dalek::{SigningKey, VerifyingKey, Signer, Signature};
use rand::{rngs::OsRng, Rng};
use serde::{Deserialize, Serialize};
use bs58;

/// Persistable wallet info for storage or serialization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WalletInfo {
    pub name: String,
    pub address: String,
    pub encrypted_key: String,
}

/// In-memory wallet holding an ed25519 signing key
#[derive(Debug, Clone)]
pub struct Wallet {
    pub signing_key: SigningKey,
    pub name: String,
}

impl Wallet {
    /// Generate a new random wallet
    pub fn new(name: String) -> Self {
        let mut csprng = OsRng;
        let secret_bytes: [u8; 32] = csprng.gen();
        let signing_key = SigningKey::from_bytes(&secret_bytes);
        Self { signing_key, name }
    }

    /// Reconstruct from a raw private key (32 or 64 bytes)
    pub fn from_private_key(
        private_key_bytes: &[u8],
        name: String,
    ) -> Result<Self, String> {
        match private_key_bytes.len() {
            32 => {
                let mut key_bytes = [0u8; 32];
                key_bytes.copy_from_slice(private_key_bytes);
                let signing_key = SigningKey::from_bytes(&key_bytes);
                Ok(Self { signing_key, name })
            }
            64 => {
                let mut key_bytes = [0u8; 32];
                key_bytes.copy_from_slice(&private_key_bytes[..32]);
                let signing_key = SigningKey::from_bytes(&key_bytes);
                let verifying_key = signing_key.verifying_key();
                let expected_pub = &private_key_bytes[32..];
                if verifying_key.as_bytes() != expected_pub {
                    return Err("Public key does not match private key".into());
                }
                Ok(Self { signing_key, name })
            }
            len => Err(format!("Invalid key length: {} bytes", len)),
        }
    }

    /// Base58-encoded Solana-style public key
    pub fn get_public_key(&self) -> String {
        let vk = self.signing_key.verifying_key();
        bs58::encode(vk.as_bytes()).into_string()
    }

    /// Base58-encoded Solana-compatible keypair (64 bytes)
    pub fn get_private_key(&self) -> String {
        let vk = self.signing_key.verifying_key();
        let mut buf = Vec::with_capacity(64);
        buf.extend_from_slice(&self.signing_key.to_bytes());
        buf.extend_from_slice(vk.as_bytes());
        bs58::encode(buf).into_string()
    }

    /// Only the 32-byte private key, base58-encoded
    pub fn get_private_key_only(&self) -> String {
        bs58::encode(self.signing_key.to_bytes()).into_string()
    }

    /// Serialize into `WalletInfo`
    pub fn to_wallet_info(&self) -> WalletInfo {
        WalletInfo {
            name: self.name.clone(),
            address: self.get_public_key(),
            encrypted_key: self.get_private_key(),
        }
    }

    /// Deserialize from `WalletInfo`
    pub fn from_wallet_info(info: &WalletInfo) -> Result<Self, String> {
        let bytes = bs58::decode(&info.encrypted_key)
            .into_vec()
            .map_err(|e| format!("Decode error: {}", e))?;
        Self::from_private_key(&bytes, info.name.clone())
    }

    /// Sign a transaction message (serialized transaction)
    pub fn sign_transaction(&self, message: &[u8]) -> String {
        let signature = self.signing_key.sign(message);
        bs58::encode(signature.to_bytes()).into_string()
    }

    /// Sign a message with ed25519
    pub fn sign_message(&self, message: &[u8]) -> Signature {
        self.signing_key.sign(message)
    }

    /// Get the verifying key (public key)
    pub fn get_verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Sign a message and return the signature bytes
    pub fn sign_message_bytes(&self, message: &[u8]) -> Vec<u8> {
        let signature = self.signing_key.sign(message);
        signature.to_bytes().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wallet_creation() {
        let wallet = Wallet::new("Test Wallet".to_string());
        assert!(!wallet.get_public_key().is_empty());
        assert!(!wallet.get_private_key().is_empty());
    }

    #[test]
    fn test_signing() {
        let wallet = Wallet::new("Test Wallet".to_string());
        let message = b"Hello, Solana!";
        let signature = wallet.sign_message(message);
        
        // Verify the signature
        let verifying_key = wallet.get_verifying_key();
        assert!(verifying_key.verify(message, &signature).is_ok());
    }

    #[test]
    fn test_import_export() {
        let wallet1 = Wallet::new("Test Wallet".to_string());
        let info = wallet1.to_wallet_info();
        let wallet2 = Wallet::from_wallet_info(&info).unwrap();
        
        assert_eq!(wallet1.get_public_key(), wallet2.get_public_key());
        assert_eq!(wallet1.get_private_key(), wallet2.get_private_key());
    }

    #[test]
    fn test_solana_keypair_import() {
        // Test importing a Solana CLI keypair
        let keypair_b58 = "4UzFMkVbk1q6ApxvDS8inUxg4cMBxCQRVXRx5msqQyktbi1QkJkt574Jda6BjZThSJi54CHfVoLFdVFX8XFn233L";
        let bytes = bs58::decode(keypair_b58).into_vec().unwrap();
        let wallet = Wallet::from_private_key(&bytes, "Test".into()).unwrap();
        let pubkey = wallet.get_public_key();
        println!("Public key: {}", pubkey);
        
        // Verify the keypair can be exported and re-imported
        let exported = wallet.get_private_key();
        let imported_bytes = bs58::decode(&exported).into_vec().unwrap();
        let wallet2 = Wallet::from_private_key(&imported_bytes, "Test2".into()).unwrap();
        assert_eq!(wallet.get_public_key(), wallet2.get_public_key());
    }
}