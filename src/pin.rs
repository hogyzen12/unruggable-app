// src/pin.rs
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use base64::Engine;
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::sync::{Mutex, OnceLock};

const LEGACY_PBKDF2_ITERATIONS: u32 = 600_000;
const MIN_CALIBRATED_PBKDF2_ITERATIONS: u32 = 50_000;
const MAX_CALIBRATED_PBKDF2_ITERATIONS: u32 = 200_000;
const PBKDF2_CALIBRATION_SAMPLE_ITERATIONS: u32 = 25_000;
const PBKDF2_TARGET_UNLOCK_MS: u128 = 250;
const PBKDF2_ROUNDING_GRANULARITY: u32 = 5_000;
const KEY_LENGTH: usize = 32;
const SALT_LENGTH: usize = 16;
const NONCE_LENGTH: usize = 12;
const ENCRYPTED_SECRET_PREFIX: &str = "enc-v1:";

fn session_key_slot() -> &'static Mutex<Vec<[u8; KEY_LENGTH]>> {
    static SESSION_KEYS: OnceLock<Mutex<Vec<[u8; KEY_LENGTH]>>> = OnceLock::new();
    SESSION_KEYS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Derive the AES key used to encrypt secrets from the app PIN.
pub fn derive_key_from_pin(pin: &str, salt: &[u8]) -> [u8; KEY_LENGTH] {
    derive_key_from_pin_with_iterations(pin, salt, default_pbkdf2_iterations())
}

pub fn derive_key_from_pin_with_iterations(
    pin: &str,
    salt: &[u8],
    iterations: u32,
) -> [u8; KEY_LENGTH] {
    let mut key = [0u8; KEY_LENGTH];
    pbkdf2_hmac::<Sha256>(pin.as_bytes(), salt, iterations, &mut key);
    key
}

pub fn default_pbkdf2_iterations() -> u32 {
    LEGACY_PBKDF2_ITERATIONS
}

pub fn stored_pbkdf2_iterations(iterations: u32) -> u32 {
    if iterations == 0 {
        default_pbkdf2_iterations()
    } else {
        iterations
    }
}

fn round_pbkdf2_iterations(iterations: u32) -> u32 {
    let rounded = ((iterations + PBKDF2_ROUNDING_GRANULARITY - 1) / PBKDF2_ROUNDING_GRANULARITY)
        * PBKDF2_ROUNDING_GRANULARITY;
    rounded.clamp(
        MIN_CALIBRATED_PBKDF2_ITERATIONS,
        MAX_CALIBRATED_PBKDF2_ITERATIONS,
    )
}

pub fn calibrate_pbkdf2_iterations() -> u32 {
    let calibration_started = std::time::Instant::now();
    let salt = [0u8; SALT_LENGTH];
    let mut sample_key =
        derive_key_from_pin_with_iterations("000000", &salt, PBKDF2_CALIBRATION_SAMPLE_ITERATIONS);
    let sample_ms = calibration_started.elapsed().as_millis().max(1);
    sample_key.fill(0);

    let estimated_iterations = ((PBKDF2_CALIBRATION_SAMPLE_ITERATIONS as u128
        * PBKDF2_TARGET_UNLOCK_MS)
        / sample_ms) as u32;
    let calibrated = round_pbkdf2_iterations(estimated_iterations);

    println!(
        "PIN KDF calibration: sample={}ms sample_iterations={} chosen_iterations={}",
        sample_ms, PBKDF2_CALIBRATION_SAMPLE_ITERATIONS, calibrated
    );

    calibrated
}

pub(crate) fn hash_derived_key(key: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key);
    format!("{:x}", hasher.finalize())
}

/// Legacy plain SHA-256 PIN hash kept only for one-time upgrade compatibility.
pub fn legacy_hash_pin(pin: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(pin.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Hash PIN for storage verification using the same salt-backed KDF as data encryption.
#[allow(dead_code)]
pub fn hash_pin(pin: &str, salt: &[u8]) -> String {
    let mut key = derive_key_from_pin(pin, salt);
    let hash = hash_derived_key(&key);
    key.fill(0);
    hash
}

/// Generate random salt
pub fn generate_salt() -> [u8; SALT_LENGTH] {
    let mut salt = [0u8; SALT_LENGTH];
    OsRng.fill_bytes(&mut salt);
    salt
}

fn current_session_key() -> Result<[u8; KEY_LENGTH], String> {
    let guard = session_key_slot()
        .lock()
        .map_err(|_| "Failed to access PIN session".to_string())?;
    guard
        .first()
        .copied()
        .ok_or_else(|| "Wallet secrets are locked. Enter your PIN first.".to_string())
}

fn current_session_keys() -> Result<Vec<[u8; KEY_LENGTH]>, String> {
    let guard = session_key_slot()
        .lock()
        .map_err(|_| "Failed to access PIN session".to_string())?;
    if guard.is_empty() {
        Err("Wallet secrets are locked. Enter your PIN first.".to_string())
    } else {
        Ok(guard.clone())
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn unlock_session(pin: &str, salt: &[u8]) -> Result<(), String> {
    let key = derive_key_from_pin(pin, salt);
    unlock_session_with_key(key)
}

pub fn unlock_session_with_key(key: [u8; KEY_LENGTH]) -> Result<(), String> {
    unlock_session_with_fallback_key(key, None)
}

pub fn unlock_session_with_fallback_key(
    mut primary_key: [u8; KEY_LENGTH],
    mut fallback_key: Option<[u8; KEY_LENGTH]>,
) -> Result<(), String> {
    let mut guard = session_key_slot().lock().map_err(|_| {
        primary_key.fill(0);
        if let Some(mut key) = fallback_key.take() {
            key.fill(0);
        }
        "Failed to access PIN session".to_string()
    })?;
    for mut existing in guard.drain(..) {
        existing.fill(0);
    }
    guard.push(primary_key);
    if let Some(fallback) = fallback_key {
        if guard[0] != fallback {
            guard.push(fallback);
        }
    }
    Ok(())
}

pub fn clear_session_fallback_keys() -> Result<(), String> {
    let mut guard = session_key_slot()
        .lock()
        .map_err(|_| "Failed to access PIN session".to_string())?;
    if guard.is_empty() {
        return Err("Wallet secrets are locked. Enter your PIN first.".to_string());
    }
    for key in guard.iter_mut().skip(1) {
        key.fill(0);
    }
    guard.truncate(1);
    Ok(())
}

pub fn clear_session() {
    if let Ok(mut guard) = session_key_slot().lock() {
        for mut key in guard.drain(..) {
            key.fill(0);
        }
    }
}

/// Best-effort wipe for short-lived PIN strings after use.
pub fn wipe_secret_string(secret: &mut String) {
    unsafe {
        secret.as_mut_vec().fill(0);
    }
    secret.clear();
}

pub fn has_unlocked_session() -> bool {
    session_key_slot()
        .lock()
        .map(|guard| !guard.is_empty())
        .unwrap_or(false)
}

pub fn is_encrypted_secret(value: &str) -> bool {
    value.starts_with(ENCRYPTED_SECRET_PREFIX)
}

pub fn encrypt_secret_string(secret: &str) -> Result<String, String> {
    if secret.is_empty() {
        return Ok(String::new());
    }
    if is_encrypted_secret(secret) {
        return Ok(secret.to_string());
    }

    let key = current_session_key()?;
    let cipher =
        Aes256Gcm::new_from_slice(&key).map_err(|e| format!("Failed to create cipher: {e}"))?;

    let mut nonce_bytes = [0u8; NONCE_LENGTH];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, secret.as_bytes())
        .map_err(|e| format!("Encryption failed: {e}"))?;

    let mut payload = nonce_bytes.to_vec();
    payload.extend_from_slice(&ciphertext);

    Ok(format!(
        "{ENCRYPTED_SECRET_PREFIX}{}",
        base64::engine::general_purpose::STANDARD.encode(payload)
    ))
}

pub fn decrypt_secret_string(secret: &str) -> Result<String, String> {
    if secret.is_empty() {
        return Ok(String::new());
    }

    let Some(encoded_payload) = secret.strip_prefix(ENCRYPTED_SECRET_PREFIX) else {
        return Ok(secret.to_string());
    };

    let payload = base64::engine::general_purpose::STANDARD
        .decode(encoded_payload.as_bytes())
        .map_err(|e| format!("Encrypted secret is invalid base64: {e}"))?;

    if payload.len() < NONCE_LENGTH {
        return Err("Encrypted secret payload is too short".to_string());
    }

    let (nonce_bytes, ciphertext) = payload.split_at(NONCE_LENGTH);
    let nonce = Nonce::from_slice(nonce_bytes);
    let mut session_keys = current_session_keys()?;
    let mut plaintext = None;

    for key in &session_keys {
        let cipher =
            Aes256Gcm::new_from_slice(key).map_err(|e| format!("Failed to create cipher: {e}"))?;
        if let Ok(decrypted) = cipher.decrypt(nonce, ciphertext) {
            plaintext = Some(decrypted);
            break;
        }
    }

    for key in &mut session_keys {
        key.fill(0);
    }

    let plaintext =
        plaintext.ok_or_else(|| "Failed to decrypt stored secret. Check your PIN.".to_string())?;

    String::from_utf8(plaintext).map_err(|e| format!("Decrypted secret is not valid UTF-8: {e}"))
}

#[cfg(test)]
pub fn encrypt_with_pin(data: &[u8], pin: &str, salt: &[u8]) -> Result<Vec<u8>, String> {
    let key = derive_key_from_pin(pin, salt);
    let cipher =
        Aes256Gcm::new_from_slice(&key).map_err(|e| format!("Failed to create cipher: {e}"))?;

    let mut nonce_bytes = [0u8; NONCE_LENGTH];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, data)
        .map_err(|e| format!("Encryption failed: {e}"))?;

    let mut result = nonce_bytes.to_vec();
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

#[cfg(test)]
pub fn decrypt_with_pin(encrypted_data: &[u8], pin: &str, salt: &[u8]) -> Result<Vec<u8>, String> {
    if encrypted_data.len() < NONCE_LENGTH {
        return Err("Invalid encrypted data".to_string());
    }

    let (nonce_bytes, ciphertext) = encrypted_data.split_at(NONCE_LENGTH);
    let nonce = Nonce::from_slice(nonce_bytes);

    let key = derive_key_from_pin(pin, salt);
    let cipher =
        Aes256Gcm::new_from_slice(&key).map_err(|e| format!("Failed to create cipher: {e}"))?;

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "Decryption failed - incorrect PIN".to_string())
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

    #[test]
    fn test_secret_string_round_trip_requires_unlocked_session() {
        clear_session();

        let err = encrypt_secret_string("super secret").unwrap_err();
        assert!(err.contains("locked"));

        let pin = "123456";
        let salt = generate_salt();
        unlock_session(pin, &salt).unwrap();

        let encrypted = encrypt_secret_string("super secret").unwrap();
        assert!(is_encrypted_secret(&encrypted));

        let decrypted = decrypt_secret_string(&encrypted).unwrap();
        assert_eq!(decrypted, "super secret");

        clear_session();
        let err = decrypt_secret_string(&encrypted).unwrap_err();
        assert!(err.contains("locked"));
    }

    #[test]
    fn test_plaintext_secret_passthrough_for_legacy_records() {
        clear_session();
        let plaintext = "legacy-base58-private-key";
        assert_eq!(decrypt_secret_string(plaintext).unwrap(), plaintext);
    }
}
