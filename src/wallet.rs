// src/wallet.rs
use crate::pin;
use bs58;
use ed25519_dalek::{Signature, Signer, SigningKey};
use rand::{rngs::OsRng, Rng};
use serde::{Deserialize, Serialize};

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
    pub fn from_private_key(private_key_bytes: &[u8], name: String) -> Result<Self, String> {
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
    pub fn to_wallet_info(&self) -> Result<WalletInfo, String> {
        Ok(WalletInfo {
            name: self.name.clone(),
            address: self.get_public_key(),
            encrypted_key: pin::encrypt_secret_string(&self.get_private_key())?,
        })
    }

    /// Deserialize from `WalletInfo`
    pub fn from_wallet_info(info: &WalletInfo) -> Result<Self, String> {
        let decrypted_key = pin::decrypt_secret_string(&info.encrypted_key)?;
        let bytes = bs58::decode(&decrypted_key)
            .into_vec()
            .map_err(|e| format!("Decode error: {}", e))?;
        Self::from_private_key(&bytes, info.name.clone())
    }

    /// Sign a message with ed25519
    pub fn sign_message(&self, message: &[u8]) -> Signature {
        self.signing_key.sign(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pin::{clear_session, generate_salt, is_encrypted_secret, unlock_session};

    #[test]
    fn wallet_info_serialization_requires_unlocked_session() {
        clear_session();

        let wallet = Wallet::new("Test Wallet".to_string());
        let err = wallet.to_wallet_info().unwrap_err();

        assert!(err.contains("locked"));
    }

    #[test]
    fn wallet_info_round_trip_encrypts_private_key() {
        clear_session();

        let wallet = Wallet::new("Test Wallet".to_string());
        let salt = generate_salt();
        unlock_session("123456", &salt).unwrap();

        let wallet_info = wallet.to_wallet_info().unwrap();
        assert!(is_encrypted_secret(&wallet_info.encrypted_key));
        assert_ne!(wallet_info.encrypted_key, wallet.get_private_key());

        let restored = Wallet::from_wallet_info(&wallet_info).unwrap();
        assert_eq!(restored.get_public_key(), wallet.get_public_key());
        assert_eq!(restored.get_private_key(), wallet.get_private_key());

        clear_session();
    }

    #[test]
    fn wallet_info_decryption_fails_after_relocking() {
        clear_session();

        let wallet = Wallet::new("Test Wallet".to_string());
        let salt = generate_salt();
        unlock_session("123456", &salt).unwrap();
        let wallet_info = wallet.to_wallet_info().unwrap();

        clear_session();
        let err = Wallet::from_wallet_info(&wallet_info).unwrap_err();
        assert!(err.contains("locked"));
    }
}
