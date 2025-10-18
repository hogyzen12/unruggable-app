// src/pin.rs
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use pbkdf2::{pbkdf2_hmac};
use sha2::Sha256;
use rand::RngCore;

const PBKDF2_ITERATIONS: u32 = 100_000; // iOS standard
const KEY_LENGTH: usize = 32; // 256 bits for AES-256
const SALT_LENGTH: usize = 16;
const NONCE_LENGTH: usize = 12;

/// Derive encryption key from PIN using PBKDF2
pub fn derive_key_from_pin(pin: &str, salt: &[u8]) -> [u8; KEY_LENGTH] {
    let mut key = [0u8; KEY_LENGTH];
    pbkdf2_hmac::<Sha256>(pin.as_bytes(), salt, PBKDF2_ITERATIONS, &mut key);
    key
}

/// Generate random salt
pub fn generate_salt() -> [u8; SALT_LENGTH] {
    let mut salt = [0u8; SALT_LENGTH];
    OsRng.fill_bytes(&mut salt);
    salt
}

/// Encrypt data using PIN-derived key
pub fn encrypt_with_pin(data: &[u8], pin: &str, salt: &[u8]) -> Result<Vec<u8>, String> {
    let key = derive_key_from_pin(pin, salt);
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| format!("Failed to create cipher: {}", e))?;
    
    // Generate random nonce
    let mut nonce_bytes = [0u8; NONCE_LENGTH];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    // Encrypt data
    let ciphertext = cipher.encrypt(nonce, data)
        .map_err(|e| format!("Encryption failed: {}", e))?;
    
    // Prepend nonce to ciphertext
    let mut result = nonce_bytes.to_vec();
    result.extend_from_slice(&ciphertext);
    
    Ok(result)
}

/// Decrypt data using PIN-derived key
pub fn decrypt_with_pin(encrypted_data: &[u8], pin: &str, salt: &[u8]) -> Result<Vec<u8>, String> {
    if encrypted_data.len() < NONCE_LENGTH {
        return Err("Invalid encrypted data".to_string());
    }
    
    // Extract nonce and ciphertext
    let (nonce_bytes, ciphertext) = encrypted_data.split_at(NONCE_LENGTH);
    let nonce = Nonce::from_slice(nonce_bytes);
    
    let key = derive_key_from_pin(pin, salt);
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| format!("Failed to create cipher: {}", e))?;
    
    // Decrypt data
    cipher.decrypt(nonce, ciphertext)
        .map_err(|_| "Decryption failed - incorrect PIN".to_string())
}

/// Hash PIN for storage verification (not for encryption)
pub fn hash_pin(pin: &str) -> String {
    use sha2::Digest;
    let mut hasher = Sha256::new();
    hasher.update(pin.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_decryption() {
        let pin = "123456";
        let salt = generate_salt();
        let data = b"test wallet data";
        
        let encrypted = encrypt_with_pin(data, pin, &salt).unwrap();
        let decrypted = decrypt_with_pin(&encrypted, pin, &salt).unwrap();
        
        assert_eq!(data.to_vec(), decrypted);
    }

    #[test]
    fn test_wrong_pin_fails() {
        let pin = "123456";
        let wrong_pin = "654321";
        let salt = generate_salt();
        let data = b"test wallet data";
        
        let encrypted = encrypt_with_pin(data, pin, &salt).unwrap();
        let result = decrypt_with_pin(&encrypted, wrong_pin, &salt);
        
        assert!(result.is_err());
    }
}