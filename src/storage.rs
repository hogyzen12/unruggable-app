#![allow(dead_code)]

use crate::quantum_vault::StoredVault;
use crate::wallet::{Wallet, WalletInfo};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// Android-specific imports
#[cfg(target_os = "android")]
use std::path::PathBuf;

// Custom error type that implements Send
#[derive(Debug, Clone)]
pub struct StorageError(String);

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for StorageError {}

impl From<String> for StorageError {
    fn from(s: String) -> Self {
        StorageError(s)
    }
}

impl From<&str> for StorageError {
    fn from(s: &str) -> Self {
        StorageError(s.to_string())
    }
}

#[cfg(target_os = "android")]
impl From<jni::errors::Error> for StorageError {
    fn from(e: jni::errors::Error) -> Self {
        StorageError(format!("JNI Error: {}", e))
    }
}

// Android-specific function to get the proper files directory
#[cfg(target_os = "android")]
fn get_android_files_dir() -> Result<String, StorageError> {
    use dioxus::mobile::wry::prelude::dispatch;
    use jni::objects::{JObject, JString};
    use jni::JNIEnv;

    let (tx, rx) = std::sync::mpsc::channel();

    fn run(env: &mut JNIEnv<'_>, activity: &JObject<'_>) -> Result<String, StorageError> {
        // Get the files directory (internal storage)
        let files_dir = env
            .call_method(activity, "getFilesDir", "()Ljava/io/File;", &[])?
            .l()?;

        // Get the absolute path
        let files_dir_path: JString<'_> = env
            .call_method(files_dir, "getAbsolutePath", "()Ljava/lang/String;", &[])?
            .l()?
            .into();

        // Convert to Rust string
        let files_dir_str: String = env.get_string(&files_dir_path)?.into();

        Ok(files_dir_str)
    }

    dispatch(move |env, activity, _webview| {
        let result = run(env, activity);
        tx.send(result).unwrap();
    });

    match rx.recv() {
        Ok(result) => result,
        Err(e) => Err(StorageError::from(format!("Channel receive error: {}", e))),
    }
}

// Use OnceLock instead of lazy_static for Android
#[cfg(target_os = "android")]
fn get_android_files_dir_cached() -> &'static Option<String> {
    use std::sync::OnceLock;
    static ANDROID_FILES_DIR: OnceLock<Option<String>> = OnceLock::new();
    ANDROID_FILES_DIR.get_or_init(|| match get_android_files_dir() {
        Ok(dir) => {
            log::info!("✅ Android files directory initialized: {}", dir);
            Some(dir)
        }
        Err(e) => {
            log::error!("❌ Failed to initialize Android files directory: {}", e);
            None
        }
    })
}

// Use lazy_static only on non-Android platforms
#[cfg(not(target_os = "android"))]
lazy_static::lazy_static! {
    static ref ANDROID_FILES_DIR: Option<String> = None;
}

// Get the appropriate storage directory for the current platform
fn get_storage_dir() -> String {
    #[cfg(target_os = "android")]
    {
        match get_android_files_dir() {
            Ok(dir) => {
                log::info!("✅ Using Android files directory: {}", dir);
                dir
            }
            Err(e) => {
                log::error!("❌ Failed to get Android files directory: {}", e);
                log::warn!("⚠️ Falling back to current directory");
                ".".to_string()
            }
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        let home_dir = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        format!("{home_dir}/.solana_wallet_app")
    }
}

fn get_storage_dir_simple() -> String {
    #[cfg(target_os = "android")]
    {
        if let Some(ref dir) = *get_android_files_dir_cached() {
            dir.clone()
        } else {
            log::warn!("⚠️ Using fallback storage directory");
            "/data/data/com.unruggable/files".to_string() // Hardcoded fallback
        }
    }
    #[cfg(target_os = "ios")]
    {
        // Use iOS Application Support directory (better than Documents for app data)
        if let Some(home) = std::env::var_os("HOME") {
            let app_support = std::path::PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("WalletData");

            let app_support_str = app_support.to_string_lossy().to_string();
            log::info!("🍎 Using iOS Application Support: {}", app_support_str);
            app_support_str
        } else {
            log::warn!("⚠️ iOS HOME not found, using fallback");
            "./WalletData".to_string()
        }
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let home_dir = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        format!("{home_dir}/.solana_wallet_app")
    }
}

// Add iOS-specific initialization function (add this new function)
#[cfg(target_os = "ios")]
pub fn init_ios_storage() -> Result<(), String> {
    log::info!("🍎 Initializing iOS storage...");

    // Log environment info for debugging
    if let Some(home) = std::env::var_os("HOME") {
        log::info!("📱 iOS HOME: {}", home.to_string_lossy());
    } else {
        log::warn!("⚠️ iOS HOME environment variable not found");
    }

    // Get and create storage directory
    let storage_dir = get_storage_dir_simple();
    log::info!("📁 iOS storage directory: {}", storage_dir);

    // Ensure directory exists
    match ensure_storage_dir() {
        Ok(_) => {
            log::info!("✅ iOS storage directory ready");

            // Test read/write capabilities
            let test_file = format!("{}/ios_test.txt", storage_dir);
            match std::fs::write(&test_file, "iOS storage test") {
                Ok(_) => {
                    log::info!("✅ iOS write test successful");

                    // Verify we can read it back
                    match std::fs::read_to_string(&test_file) {
                        Ok(content) => {
                            if content == "iOS storage test" {
                                log::info!("✅ iOS read-write verification successful");
                                let _ = std::fs::remove_file(&test_file); // cleanup
                                Ok(())
                            } else {
                                Err("iOS read-write verification failed".to_string())
                            }
                        }
                        Err(e) => Err(format!("iOS read test failed: {}", e)),
                    }
                }
                Err(e) => Err(format!("iOS write test failed: {}", e)),
            }
        }
        Err(e) => Err(format!("iOS storage directory creation failed: {}", e)),
    }
}

// Get file paths
fn get_wallets_file_path() -> String {
    let storage_dir = get_storage_dir_simple();
    format!("{storage_dir}/wallets.json")
}

fn get_rpc_file_path() -> String {
    let storage_dir = get_storage_dir_simple();
    format!("{storage_dir}/rpc.txt")
}

fn get_jito_settings_file_path() -> String {
    let storage_dir = get_storage_dir_simple();
    format!("{storage_dir}/jito_settings.json")
}

fn get_integration_settings_file_path() -> String {
    let storage_dir = get_storage_dir_simple();
    format!("{storage_dir}/integration_settings.json")
}

fn get_quantum_vaults_file_path() -> String {
    let storage_dir = get_storage_dir_simple();
    format!("{storage_dir}/quantum_vaults.json")
}

fn get_address_book_file_path() -> String {
    let storage_dir = get_storage_dir_simple();
    format!("{storage_dir}/address_book.json")
}

fn get_send_counts_file_path() -> String {
    let storage_dir = get_storage_dir_simple();
    format!("{storage_dir}/send_counts.json")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AddressBookEntry {
    pub address: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct SendCountEntry {
    address: String,
    count: u64,
}

// Ensure storage directory exists with logging
fn ensure_storage_dir() -> Result<(), std::io::Error> {
    let storage_dir = get_storage_dir_simple();
    log::info!("Ensuring storage directory exists: {}", storage_dir);

    match std::fs::create_dir_all(&storage_dir) {
        Ok(_) => {
            log::info!("✅ Storage directory created/verified: {}", storage_dir);

            // Verify permissions by writing a test file
            let test_file = format!("{}/permission_test.txt", storage_dir);
            match std::fs::write(&test_file, "permission_test") {
                Ok(_) => {
                    log::info!("✅ Storage directory is writable");
                    let _ = std::fs::remove_file(&test_file);
                    Ok(())
                }
                Err(e) => {
                    log::error!("❌ Storage directory exists but is not writable: {}", e);
                    Err(e)
                }
            }
        }
        Err(e) => {
            log::error!(
                "❌ Failed to create storage directory {}: {}",
                storage_dir,
                e
            );
            Err(e)
        }
    }
}

#[cfg(unix)]
fn harden_file_permissions(path: &str) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)
        .map_err(|e| format!("Failed to read metadata for {}: {}", path, e))?
        .permissions();
    permissions.set_mode(0o600);
    std::fs::set_permissions(path, permissions)
        .map_err(|e| format!("Failed to set permissions on {}: {}", path, e))
}

#[cfg(not(unix))]
fn harden_file_permissions(_path: &str) -> Result<(), String> {
    Ok(())
}

fn write_secure_file(path: &str, contents: &str) -> Result<(), String> {
    std::fs::write(path, contents).map_err(|e| format!("Failed to write {}: {}", path, e))?;
    harden_file_permissions(path)?;
    Ok(())
}

// Add this function for testing Android storage
#[cfg(target_os = "android")]
pub fn ensure_android_storage_works() -> Result<(), String> {
    log::info!("🔧 Testing Android storage...");

    // Try to write a simple test file
    let test_dir = "/data/data/com.unruggable/files";

    match std::fs::create_dir_all(test_dir) {
        Ok(_) => log::info!("✅ Created storage directory: {}", test_dir),
        Err(e) => {
            log::error!("❌ Failed to create storage directory: {}", e);
            return Err(format!("Storage directory creation failed: {}", e));
        }
    }

    let test_file = format!("{}/test.txt", test_dir);
    match std::fs::write(&test_file, "test") {
        Ok(_) => {
            log::info!("✅ Storage write test successful");
            let _ = std::fs::remove_file(&test_file);
            Ok(())
        }
        Err(e) => {
            log::error!("❌ Storage write test failed: {}", e);
            Err(format!("Storage write failed: {}", e))
        }
    }
}

fn normalize_wallet_info_for_storage(wallet_info: &WalletInfo) -> Result<WalletInfo, String> {
    let mut normalized = wallet_info.clone();
    if !normalized.encrypted_key.trim().is_empty()
        && !crate::pin::is_encrypted_secret(&normalized.encrypted_key)
    {
        normalized.encrypted_key = crate::pin::encrypt_secret_string(&normalized.encrypted_key)?;
    }
    Ok(normalized)
}

fn normalize_wallets_for_storage(wallets: &[WalletInfo]) -> Result<Vec<WalletInfo>, String> {
    wallets
        .iter()
        .map(normalize_wallet_info_for_storage)
        .collect()
}

fn rewrap_wallet_info_for_current_session(wallet_info: &WalletInfo) -> Result<WalletInfo, String> {
    let mut rewrapped = wallet_info.clone();
    if !rewrapped.encrypted_key.trim().is_empty() {
        let decrypted_key = crate::pin::decrypt_secret_string(&rewrapped.encrypted_key)?;
        rewrapped.encrypted_key = crate::pin::encrypt_secret_string(&decrypted_key)?;
    }
    Ok(rewrapped)
}

fn rewrap_wallets_for_current_session(wallets: &[WalletInfo]) -> Result<Vec<WalletInfo>, String> {
    wallets
        .iter()
        .map(rewrap_wallet_info_for_current_session)
        .collect()
}

fn normalize_vault_for_storage(vault: &StoredVault) -> Result<StoredVault, String> {
    let mut normalized = vault.clone();
    if !normalized.private_key.trim().is_empty()
        && !crate::pin::is_encrypted_secret(&normalized.private_key)
    {
        normalized.private_key = crate::pin::encrypt_secret_string(&normalized.private_key)?;
    }
    Ok(normalized)
}

fn normalize_vaults_for_storage(vaults: &[StoredVault]) -> Result<Vec<StoredVault>, String> {
    vaults.iter().map(normalize_vault_for_storage).collect()
}

fn rewrap_vault_for_current_session(vault: &StoredVault) -> Result<StoredVault, String> {
    let mut rewrapped = vault.clone();
    if !rewrapped.private_key.trim().is_empty() {
        let decrypted_key = crate::pin::decrypt_secret_string(&rewrapped.private_key)?;
        rewrapped.private_key = crate::pin::encrypt_secret_string(&decrypted_key)?;
    }
    Ok(rewrapped)
}

fn rewrap_vaults_for_current_session(vaults: &[StoredVault]) -> Result<Vec<StoredVault>, String> {
    vaults
        .iter()
        .map(rewrap_vault_for_current_session)
        .collect()
}

pub fn migrate_legacy_secret_storage() -> Result<(), String> {
    if !crate::pin::has_unlocked_session() {
        return Err("Wallet secrets are locked. Enter your PIN first.".to_string());
    }

    let wallets = load_wallets_from_storage();
    let wallet_migration_needed = wallets.iter().any(|wallet| {
        !wallet.encrypted_key.trim().is_empty()
            && !crate::pin::is_encrypted_secret(&wallet.encrypted_key)
    });
    if wallet_migration_needed {
        save_wallets_to_storage(&wallets)?;
        log::info!("🔐 Migrated legacy wallet secrets to encrypted storage");
    }

    let vaults = load_quantum_vaults_from_storage();
    let vault_migration_needed = vaults.iter().any(|vault| {
        !vault.private_key.trim().is_empty() && !crate::pin::is_encrypted_secret(&vault.private_key)
    });
    if vault_migration_needed {
        save_quantum_vaults_to_storage(&vaults)?;
        log::info!("🔐 Migrated legacy quantum vault secrets to encrypted storage");
    }

    Ok(())
}

pub fn save_wallet_to_storage(wallet_info: &WalletInfo) -> Result<(), String> {
    log::info!("🔄 Attempting to save wallet: {}", wallet_info.name);

    let mut wallets = load_wallets_from_storage();
    if wallets
        .iter()
        .any(|wallet| wallet.address == wallet_info.address)
    {
        return Err("That wallet is already imported.".to_string());
    }
    wallets.push(wallet_info.clone());

    save_wallets_to_storage(&wallets)?;
    log::info!(
        "✅ Wallet successfully saved. {} wallets total",
        wallets.len()
    );
    Ok(())
}

pub fn load_wallets_from_storage() -> Vec<WalletInfo> {
    log::info!("🔄 Attempting to load wallets from storage");

    // iOS-specific initialization
    #[cfg(target_os = "ios")]
    {
        if let Err(e) = init_ios_storage() {
            log::error!("❌ iOS storage init failed: {}", e);
        }
    }

    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        let result = storage
            .get_item("wallets")
            .unwrap()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default();
        log::info!("📱 Loaded {} wallets from web storage", result.len());
        result
    }

    #[cfg(not(feature = "web"))]
    {
        let wallet_file = get_wallets_file_path();
        log::info!("📁 Looking for wallets at: {}", wallet_file);

        // Ensure storage directory exists
        if let Err(e) = ensure_storage_dir() {
            log::error!("❌ Storage directory error: {}", e);
            return Vec::new();
        }

        // Check if file exists
        if !Path::new(&wallet_file).exists() {
            log::info!("ℹ️ No existing wallet file found at: {}", wallet_file);

            // Debug: List directory contents
            let storage_dir = get_storage_dir_simple();
            if let Ok(entries) = std::fs::read_dir(&storage_dir) {
                log::info!("📂 Directory contents of {}:", storage_dir);
                for entry in entries {
                    if let Ok(entry) = entry {
                        log::info!("  - {}", entry.file_name().to_string_lossy());
                    }
                }
            }

            return Vec::new();
        }

        match std::fs::read_to_string(&wallet_file) {
            Ok(data) => {
                log::info!("📄 Read {} bytes from wallet file", data.len());
                match serde_json::from_str::<Vec<WalletInfo>>(&data) {
                    Ok(wallets) => {
                        log::info!("✅ Successfully loaded {} wallets", wallets.len());
                        for (i, wallet) in wallets.iter().enumerate() {
                            log::info!(
                                "  Wallet {}: {} ({}...)",
                                i + 1,
                                wallet.name,
                                &wallet.address[..8]
                            );
                        }
                        wallets
                    }
                    Err(e) => {
                        log::error!("❌ Failed to parse wallets from {}: {}", wallet_file, e);
                        Vec::new()
                    }
                }
            }
            Err(e) => {
                log::error!("❌ Failed to read wallets from {}: {}", wallet_file, e);
                Vec::new()
            }
        }
    }
}

pub fn import_wallet_from_key(private_key: &str, name: String) -> Result<WalletInfo, String> {
    let (key_bytes, format_name) = parse_private_key_input(private_key)?;

    let wallet_name = if name.is_empty() {
        "Imported Wallet".to_string()
    } else {
        name
    };

    log::info!(
        "🔑 Importing wallet using {} input ({} bytes)",
        format_name,
        key_bytes.len()
    );

    let wallet = Wallet::from_private_key(&key_bytes, wallet_name)?;

    wallet.to_wallet_info()
}

fn normalize_private_key_input(private_key: &str) -> String {
    let mut normalized = private_key.trim().to_string();

    loop {
        let trimmed = normalized.trim();
        let quoted = (trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\''));
        if quoted && trimmed.len() >= 2 {
            normalized = trimmed[1..trimmed.len() - 1].trim().to_string();
        } else {
            return trimmed.to_string();
        }
    }
}

fn ensure_supported_key_length(key_bytes: Vec<u8>, format_name: &str) -> Result<Vec<u8>, String> {
    match key_bytes.len() {
        32 | 64 => Ok(key_bytes),
        len => Err(format!(
            "{} decoded to {} bytes. Expected a 32-byte private key or 64-byte Solana keypair.",
            format_name, len
        )),
    }
}

fn parse_private_key_json_object(key_str: &str) -> Result<(Vec<u8>, &'static str), String> {
    let value: serde_json::Value =
        serde_json::from_str(key_str).map_err(|e| format!("Invalid JSON object format: {}", e))?;

    let object = value
        .as_object()
        .ok_or_else(|| "Invalid JSON object format".to_string())?;

    for key in ["privateKey", "secretKey", "secret_key", "private_key"] {
        if let Some(value) = object.get(key) {
            return match value {
                serde_json::Value::String(inner) => parse_private_key_input(inner),
                serde_json::Value::Array(_) => {
                    let bytes = serde_json::from_value::<Vec<u8>>(value.clone())
                        .map_err(|e| format!("Invalid {} array format: {}", key, e))?;
                    Ok((
                        ensure_supported_key_length(bytes, "JSON object array")?,
                        "JSON object array",
                    ))
                }
                _ => Err(format!("Unsupported {} value in JSON object", key)),
            };
        }
    }

    if object.contains_key("mnemonic") || object.contains_key("seedPhrase") {
        return Err(
            "Seed phrase import is not supported here. Use the wallet's private key or keypair export."
                .to_string(),
        );
    }

    Err("Unsupported JSON object format. Use a private key, keypair array, or secretKey/privateKey field.".to_string())
}

fn parse_private_key_input(private_key: &str) -> Result<(Vec<u8>, &'static str), String> {
    let normalized = normalize_private_key_input(private_key);
    if normalized.is_empty() {
        return Err("Private key is empty".to_string());
    }

    if normalized.starts_with('[') && normalized.ends_with(']') {
        return Ok((
            ensure_supported_key_length(parse_json_array_key(&normalized)?, "JSON array")?,
            "JSON array",
        ));
    }

    if normalized.starts_with('{') && normalized.ends_with('}') {
        return parse_private_key_json_object(&normalized);
    }

    if normalized.contains(',') {
        return Ok((
            ensure_supported_key_length(
                parse_comma_separated_key(&normalized)?,
                "Comma-separated key",
            )?,
            "Comma-separated key",
        ));
    }

    let compact: String = normalized.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.is_empty() {
        return Err("Private key is empty".to_string());
    }

    if let Ok(decoded) = bs58::decode(&compact).into_vec() {
        if let Ok(bytes) = ensure_supported_key_length(decoded, "Base58 key") {
            return Ok((bytes, "Base58 key"));
        }
    }

    let hex_candidate = compact
        .strip_prefix("0x")
        .or_else(|| compact.strip_prefix("0X"))
        .unwrap_or(&compact);
    if !hex_candidate.is_empty()
        && hex_candidate.len() % 2 == 0
        && hex_candidate.chars().all(|c| c.is_ascii_hexdigit())
    {
        if let Ok(decoded) = hex::decode(hex_candidate) {
            if let Ok(bytes) = ensure_supported_key_length(decoded, "Hex key") {
                return Ok((bytes, "Hex key"));
            }
        }
    }

    if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(compact.as_bytes()) {
        if let Ok(bytes) = ensure_supported_key_length(decoded, "Base64 key") {
            return Ok((bytes, "Base64 key"));
        }
    }

    Err("Unsupported private key format. Use base58, JSON array, comma-separated bytes, hex, or base64.".to_string())
}

// Helper function to parse JSON array format
fn parse_json_array_key(key_str: &str) -> Result<Vec<u8>, String> {
    serde_json::from_str::<Vec<u8>>(key_str)
        .map_err(|e| format!("Invalid JSON array format: {}", e))
}

// Helper function to parse comma-separated format
fn parse_comma_separated_key(key_str: &str) -> Result<Vec<u8>, String> {
    key_str
        .split(',')
        .map(|s| {
            s.trim()
                .parse::<u8>()
                .map_err(|e| format!("Invalid number in key: {}", e))
        })
        .collect::<Result<Vec<u8>, String>>()
}

// Optional: Add a validation function to check key format before import
pub fn validate_key_format(private_key: &str) -> Result<String, String> {
    let (_, format_name) = parse_private_key_input(private_key)?;
    Ok(format_name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pin::{clear_session, generate_salt, unlock_session};

    fn unlock_test_session() {
        clear_session();
        let salt = generate_salt();
        unlock_session("123456", &salt).unwrap();
    }

    #[test]
    fn import_wallet_accepts_exported_base58_key() {
        unlock_test_session();

        let wallet = Wallet::new("Source".to_string());
        let imported =
            import_wallet_from_key(&wallet.get_private_key(), "Imported".to_string()).unwrap();
        let restored = Wallet::from_wallet_info(&imported).unwrap();

        assert_eq!(restored.get_public_key(), wallet.get_public_key());
        clear_session();
    }

    #[test]
    fn import_wallet_accepts_json_object_and_quoted_key() {
        unlock_test_session();

        let wallet = Wallet::new("Source".to_string());
        let wrapped = serde_json::json!({
            "privateKey": format!("\"{}\"", wallet.get_private_key())
        })
        .to_string();

        let imported = import_wallet_from_key(&wrapped, "Imported".to_string()).unwrap();
        let restored = Wallet::from_wallet_info(&imported).unwrap();

        assert_eq!(restored.get_public_key(), wallet.get_public_key());
        clear_session();
    }

    #[test]
    fn import_wallet_accepts_hex_keypair() {
        unlock_test_session();

        let wallet = Wallet::new("Source".to_string());
        let keypair_bytes = bs58::decode(wallet.get_private_key()).into_vec().unwrap();
        let hex_encoded = hex::encode(keypair_bytes);

        let imported = import_wallet_from_key(&hex_encoded, "Imported".to_string()).unwrap();
        let restored = Wallet::from_wallet_info(&imported).unwrap();

        assert_eq!(restored.get_public_key(), wallet.get_public_key());
        clear_session();
    }

    #[test]
    fn normalize_wallet_storage_does_not_double_encrypt_existing_secret() {
        unlock_test_session();

        let wallet = Wallet::new("Source".to_string());
        let wallet_info = wallet.to_wallet_info().unwrap();
        let normalized = normalize_wallet_info_for_storage(&wallet_info).unwrap();
        let restored = Wallet::from_wallet_info(&normalized).unwrap();

        assert_eq!(normalized.encrypted_key, wallet_info.encrypted_key);
        assert_eq!(restored.get_public_key(), wallet.get_public_key());
        clear_session();
    }
}

pub fn save_rpc_to_storage(rpc_url: &str) {
    log::info!("🔄 Saving RPC URL to storage");

    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        storage.set_item("custom_rpc", rpc_url).unwrap();
    }

    #[cfg(not(feature = "web"))]
    {
        if let Ok(_) = ensure_storage_dir() {
            let rpc_file = get_rpc_file_path();
            match std::fs::write(&rpc_file, rpc_url) {
                Ok(_) => log::info!("✅ RPC URL saved to: {}", rpc_file),
                Err(e) => log::error!("❌ Failed to write RPC to {}: {}", rpc_file, e),
            }
        }
    }
}

pub fn load_rpc_from_storage() -> Option<String> {
    log::info!("🔄 Loading RPC URL from storage");

    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        storage.get_item("custom_rpc").unwrap()
    }

    #[cfg(not(feature = "web"))]
    {
        let rpc_file = get_rpc_file_path();
        match std::fs::read_to_string(&rpc_file) {
            Ok(data) => {
                let result = Some(data.trim().to_string());
                log::info!("✅ RPC URL loaded from storage");
                result
            }
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::error!("❌ Failed to read RPC from {}: {}", rpc_file, e);
                }
                None
            }
        }
    }
}

pub fn clear_rpc_storage() {
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        storage.remove_item("custom_rpc").unwrap();
    }

    #[cfg(not(target_os = "android"))]
    {
        let rpc_file = get_rpc_file_path();
        match std::fs::remove_file(&rpc_file) {
            Ok(_) => log::info!("✅ RPC file removed"),
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::error!("❌ Failed to remove RPC file {}: {}", rpc_file, e);
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct JitoSettings {
    pub jito_tx: bool,
    pub jito_bundles: bool,
}

impl Default for JitoSettings {
    fn default() -> Self {
        Self {
            jito_tx: true,
            jito_bundles: false,
        }
    }
}

pub fn save_jito_settings_to_storage(settings: &JitoSettings) {
    log::info!("🔄 Saving Jito settings to storage");

    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        let serialized = serde_json::to_string(settings).unwrap();
        storage.set_item("jito_settings", &serialized).unwrap();
    }

    #[cfg(not(feature = "web"))]
    {
        if let Ok(_) = ensure_storage_dir() {
            let jito_file = get_jito_settings_file_path();
            match serde_json::to_string_pretty(settings) {
                Ok(serialized) => match std::fs::write(&jito_file, serialized) {
                    Ok(_) => log::info!("✅ Jito settings saved to: {}", jito_file),
                    Err(e) => {
                        log::error!("❌ Failed to write Jito settings to {}: {}", jito_file, e)
                    }
                },
                Err(e) => log::error!("❌ Failed to serialize Jito settings: {}", e),
            }
        }
    }
}

pub fn load_jito_settings_from_storage() -> JitoSettings {
    log::info!("🔄 Loading Jito settings from storage");

    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        storage
            .get_item("jito_settings")
            .unwrap()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default()
    }

    #[cfg(not(feature = "web"))]
    {
        let jito_file = get_jito_settings_file_path();
        match std::fs::read_to_string(&jito_file) {
            Ok(data) => match serde_json::from_str(&data) {
                Ok(settings) => {
                    log::info!("✅ Jito settings loaded from storage");
                    settings
                }
                Err(e) => {
                    log::error!("❌ Failed to parse Jito settings from {}: {}", jito_file, e);
                    JitoSettings::default()
                }
            },
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::error!("❌ Failed to read Jito settings from {}: {}", jito_file, e);
                }
                JitoSettings::default()
            }
        }
    }
}

pub fn get_current_jito_settings() -> JitoSettings {
    load_jito_settings_from_storage()
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct IntegrationSettings {
    pub lend: bool,
    pub squads: bool,
    pub carrot: bool,
    pub bonk_stake: bool,
    pub quantum: bool,
    pub eject: bool,
    pub privacy: bool,
    pub retire: bool,
}

impl Default for IntegrationSettings {
    fn default() -> Self {
        Self {
            lend: false,
            squads: false,
            carrot: false,
            bonk_stake: false,
            quantum: false,
            eject: false,
            privacy: false,
            retire: false,
        }
    }
}

pub fn save_integration_settings_to_storage(settings: &IntegrationSettings) {
    log::info!("🔄 Saving integration settings to storage");

    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        let serialized = serde_json::to_string(settings).unwrap();
        storage
            .set_item("integration_settings", &serialized)
            .unwrap();
    }

    #[cfg(not(feature = "web"))]
    {
        if let Ok(_) = ensure_storage_dir() {
            let settings_file = get_integration_settings_file_path();
            match serde_json::to_string_pretty(settings) {
                Ok(serialized) => match std::fs::write(&settings_file, serialized) {
                    Ok(_) => log::info!("✅ Integration settings saved to: {}", settings_file),
                    Err(e) => log::error!(
                        "❌ Failed to write integration settings to {}: {}",
                        settings_file,
                        e
                    ),
                },
                Err(e) => log::error!("❌ Failed to serialize integration settings: {}", e),
            }
        }
    }
}

pub fn load_integration_settings_from_storage() -> IntegrationSettings {
    log::info!("🔄 Loading integration settings from storage");

    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        storage
            .get_item("integration_settings")
            .unwrap()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default()
    }

    #[cfg(not(feature = "web"))]
    {
        let settings_file = get_integration_settings_file_path();
        match std::fs::read_to_string(&settings_file) {
            Ok(data) => match serde_json::from_str(&data) {
                Ok(settings) => {
                    log::info!("✅ Integration settings loaded from storage");
                    settings
                }
                Err(e) => {
                    log::error!(
                        "❌ Failed to parse integration settings from {}: {}",
                        settings_file,
                        e
                    );
                    IntegrationSettings::default()
                }
            },
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::error!(
                        "❌ Failed to read integration settings from {}: {}",
                        settings_file,
                        e
                    );
                }
                IntegrationSettings::default()
            }
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Quantum Vault Storage Functions
// ══════════════════════════════════════════════════════════════════════════════

/// Save a quantum vault to storage
pub fn save_quantum_vault_to_storage(vault: &StoredVault) -> Result<(), String> {
    log::info!("🔐 Attempting to save quantum vault: {}", vault.name);

    let mut vaults = load_quantum_vaults_from_storage();
    vaults.push(vault.clone());

    save_quantum_vaults_to_storage(&vaults)?;
    log::info!(
        "✅ Quantum vault successfully saved. {} quantum vaults total",
        vaults.len()
    );
    Ok(())
}

/// Load all quantum vaults from storage
pub fn load_quantum_vaults_from_storage() -> Vec<StoredVault> {
    log::info!("🔐 Attempting to load quantum vaults from storage");

    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        let result = storage
            .get_item("quantum_vaults")
            .unwrap()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default();
        log::info!("📱 Loaded {} quantum vaults from web storage", result.len());
        result
    }

    #[cfg(not(feature = "web"))]
    {
        let vault_file = get_quantum_vaults_file_path();

        if let Err(e) = ensure_storage_dir() {
            log::error!("❌ Storage directory error: {}", e);
            return Vec::new();
        }

        if !Path::new(&vault_file).exists() {
            log::info!("ℹ️ No existing quantum vault file found");
            return Vec::new();
        }

        match std::fs::read_to_string(&vault_file) {
            Ok(data) => match serde_json::from_str::<Vec<StoredVault>>(&data) {
                Ok(vaults) => vaults,
                Err(e) => {
                    log::error!("❌ Failed to parse quantum vaults: {}", e);
                    Vec::new()
                }
            },
            Err(e) => {
                log::error!("❌ Failed to read quantum vaults: {}", e);
                Vec::new()
            }
        }
    }
}

/// Mark a quantum vault as used after splitting
pub fn mark_quantum_vault_as_used(vault_address: &str) {
    log::info!("🔐 Marking quantum vault as used: {}", vault_address);

    let mut vaults = load_quantum_vaults_from_storage();

    if let Some(vault) = vaults.iter_mut().find(|v| v.address == vault_address) {
        vault.used = true;
        if let Err(e) = save_quantum_vaults_to_storage(&vaults) {
            log::error!(
                "❌ Failed to persist used quantum vault {}: {}",
                vault_address,
                e
            );
        } else {
            log::info!("✅ Quantum vault marked as used");
        }
    } else {
        log::warn!("⚠️ Quantum vault not found: {}", vault_address);
    }
}

/// Delete a quantum vault from storage
pub fn delete_quantum_vault_from_storage(vault_address: &str) {
    log::info!("🔐 Attempting to delete quantum vault: {}", vault_address);

    let mut vaults = load_quantum_vaults_from_storage();
    let original_count = vaults.len();

    vaults.retain(|vault| vault.address != vault_address);

    if vaults.len() < original_count {
        log::info!("✅ Quantum vault {} removed from memory", vault_address);
        if let Err(e) = save_quantum_vaults_to_storage(&vaults) {
            log::error!(
                "❌ Failed to persist quantum vault deletion for {}: {}",
                vault_address,
                e
            );
        } else {
            log::info!(
                "✅ Quantum vault deletion completed. {} vaults remaining.",
                vaults.len()
            );
        }
    } else {
        log::warn!("⚠️ Quantum vault {} not found in storage", vault_address);
    }
}

/// Save quantum vaults list to storage
pub fn save_quantum_vaults_to_storage(vaults: &Vec<StoredVault>) -> Result<(), String> {
    log::info!("🔐 Saving {} quantum vaults to storage", vaults.len());
    let normalized_vaults = normalize_vaults_for_storage(vaults)?;

    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        let serialized = serde_json::to_string(&normalized_vaults)
            .map_err(|e| format!("Failed to serialize quantum vaults: {}", e))?;
        storage
            .set_item("quantum_vaults", &serialized)
            .map_err(|_| "Failed to save quantum vaults to web storage".to_string())?;
        log::info!("✅ Quantum vaults saved to web storage");
        Ok(())
    }

    #[cfg(not(feature = "web"))]
    {
        ensure_storage_dir().map_err(|e| format!("Failed to ensure storage directory: {}", e))?;

        let vault_file = get_quantum_vaults_file_path();
        let serialized = serde_json::to_string_pretty(&normalized_vaults)
            .map_err(|e| format!("Failed to serialize quantum vaults: {}", e))?;

        write_secure_file(&vault_file, &serialized)?;

        log::info!("✅ Quantum vaults successfully saved to: {}", vault_file);
        Ok(())
    }
}

pub fn load_address_book_from_storage() -> Vec<AddressBookEntry> {
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        return storage
            .get_item("address_book")
            .unwrap()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default();
    }

    #[cfg(not(feature = "web"))]
    {
        let file_path = get_address_book_file_path();
        if let Err(e) = ensure_storage_dir() {
            log::error!("❌ Storage directory error: {}", e);
            return Vec::new();
        }

        if !Path::new(&file_path).exists() {
            return Vec::new();
        }

        match std::fs::read_to_string(&file_path) {
            Ok(data) => serde_json::from_str::<Vec<AddressBookEntry>>(&data).unwrap_or_default(),
            Err(e) => {
                log::error!("❌ Failed to read address book: {}", e);
                Vec::new()
            }
        }
    }
}

pub fn save_address_book_to_storage(entries: &Vec<AddressBookEntry>) {
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        let serialized = serde_json::to_string(entries).unwrap_or_default();
        let _ = storage.set_item("address_book", &serialized);
        return;
    }

    #[cfg(not(feature = "web"))]
    {
        if let Err(e) = ensure_storage_dir() {
            log::error!("❌ Storage directory error: {}", e);
            return;
        }
        let file_path = get_address_book_file_path();
        if let Ok(serialized) = serde_json::to_string_pretty(entries) {
            if let Err(e) = std::fs::write(&file_path, &serialized) {
                log::error!("❌ Failed to write address book: {}", e);
            }
        }
    }
}

pub fn upsert_address_book_entry(address: &str, label: &str) {
    let mut entries = load_address_book_from_storage();
    if let Some(existing) = entries.iter_mut().find(|entry| entry.address == address) {
        existing.label = label.to_string();
    } else {
        entries.push(AddressBookEntry {
            address: address.to_string(),
            label: label.to_string(),
        });
    }
    save_address_book_to_storage(&entries);
}

pub fn remove_address_book_entry(address: &str) {
    let mut entries = load_address_book_from_storage();
    entries.retain(|entry| entry.address != address);
    save_address_book_to_storage(&entries);
}

pub fn get_address_book_label(address: &str) -> Option<String> {
    load_address_book_from_storage()
        .into_iter()
        .find(|entry| entry.address == address)
        .map(|entry| entry.label)
}

fn load_send_counts_from_storage() -> HashMap<String, u64> {
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        let entries: Vec<SendCountEntry> = storage
            .get_item("send_counts")
            .unwrap()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default();
        return entries.into_iter().map(|e| (e.address, e.count)).collect();
    }

    #[cfg(not(feature = "web"))]
    {
        let file_path = get_send_counts_file_path();
        if let Err(e) = ensure_storage_dir() {
            log::error!("❌ Storage directory error: {}", e);
            return HashMap::new();
        }

        if !Path::new(&file_path).exists() {
            return HashMap::new();
        }

        match std::fs::read_to_string(&file_path) {
            Ok(data) => {
                let entries: Vec<SendCountEntry> = serde_json::from_str(&data).unwrap_or_default();
                entries.into_iter().map(|e| (e.address, e.count)).collect()
            }
            Err(e) => {
                log::error!("❌ Failed to read send counts: {}", e);
                HashMap::new()
            }
        }
    }
}

fn save_send_counts_to_storage(counts: &HashMap<String, u64>) {
    let entries: Vec<SendCountEntry> = counts
        .iter()
        .map(|(address, count)| SendCountEntry {
            address: address.clone(),
            count: *count,
        })
        .collect();

    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        let serialized = serde_json::to_string(&entries).unwrap_or_default();
        let _ = storage.set_item("send_counts", &serialized);
        return;
    }

    #[cfg(not(feature = "web"))]
    {
        if let Err(e) = ensure_storage_dir() {
            log::error!("❌ Storage directory error: {}", e);
            return;
        }
        let file_path = get_send_counts_file_path();
        if let Ok(serialized) = serde_json::to_string_pretty(&entries) {
            if let Err(e) = std::fs::write(&file_path, &serialized) {
                log::error!("❌ Failed to write send counts: {}", e);
            }
        }
    }
}

pub fn increment_send_count(address: &str) -> u64 {
    let mut counts = load_send_counts_from_storage();
    let counter = counts.entry(address.to_string()).or_insert(0);
    *counter += 1;
    let updated = *counter;
    save_send_counts_to_storage(&counts);
    updated
}

pub fn get_send_count(address: &str) -> u64 {
    load_send_counts_from_storage()
        .get(address)
        .copied()
        .unwrap_or(0)
}

/// Delete a wallet by address from storage
pub fn delete_wallet_from_storage(wallet_address: &str) {
    log::info!("🔄 Attempting to delete wallet: {}", wallet_address);

    let mut wallets = load_wallets_from_storage();
    let original_count = wallets.len();

    // Remove wallet with matching address
    wallets.retain(|wallet| wallet.address != wallet_address);

    if wallets.len() < original_count {
        log::info!("✅ Wallet {} removed from memory", wallet_address);

        // Save updated wallet list
        if let Err(e) = save_wallets_to_storage(&wallets) {
            log::error!(
                "❌ Failed to persist wallet deletion for {}: {}",
                wallet_address,
                e
            );
        } else {
            log::info!(
                "✅ Wallet deletion completed. {} wallets remaining.",
                wallets.len()
            );
        }
    } else {
        log::warn!("⚠️ Wallet {} not found in storage", wallet_address);
    }
}

/// Save wallets list to storage (only add this if it doesn't already exist in your storage.rs)
pub fn save_wallets_to_storage(wallets: &Vec<WalletInfo>) -> Result<(), String> {
    log::info!("🔄 Saving {} wallets to storage", wallets.len());
    let normalized_wallets = normalize_wallets_for_storage(wallets)?;

    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        let serialized = serde_json::to_string(&normalized_wallets)
            .map_err(|e| format!("Failed to serialize wallets: {}", e))?;
        storage
            .set_item("wallets", &serialized)
            .map_err(|_| "Failed to save wallets to web storage".to_string())?;
        log::info!("✅ Wallets saved to web storage");
        Ok(())
    }

    #[cfg(not(feature = "web"))]
    {
        ensure_storage_dir().map_err(|e| format!("Failed to ensure storage directory: {}", e))?;

        let wallet_file = get_wallets_file_path();
        let serialized = serde_json::to_string_pretty(&normalized_wallets)
            .map_err(|e| format!("Failed to serialize wallets: {}", e))?;

        write_secure_file(&wallet_file, &serialized)?;

        log::info!("✅ Wallets successfully saved to: {}", wallet_file);
        Ok(())
    }
}

pub fn has_completed_onboarding() -> bool {
    log::info!("🔄 Checking onboarding status");

    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        storage
            .get_item("onboarding_completed")
            .unwrap()
            .map(|val| val == "true")
            .unwrap_or(false)
    }

    #[cfg(not(feature = "web"))]
    {
        let storage_dir = get_storage_dir_simple();
        let onboarding_file = format!("{}/onboarding_completed.txt", storage_dir);

        match std::fs::read_to_string(&onboarding_file) {
            Ok(data) => {
                let completed = data.trim() == "true";
                log::info!("✅ Onboarding status: {}", completed);
                completed
            }
            Err(_) => {
                log::info!("📝 No onboarding file found - first launch");
                false
            }
        }
    }
}

pub fn mark_onboarding_completed() {
    log::info!("✅ Marking onboarding as completed");

    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        storage.set_item("onboarding_completed", "true").unwrap();
    }

    #[cfg(not(feature = "web"))]
    {
        if let Ok(_) = ensure_storage_dir() {
            let storage_dir = get_storage_dir_simple();
            let onboarding_file = format!("{}/onboarding_completed.txt", storage_dir);

            match std::fs::write(&onboarding_file, "true") {
                Ok(_) => log::info!("✅ Onboarding completion saved"),
                Err(e) => log::error!("❌ Failed to save onboarding status: {}", e),
            }
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// PIN Storage Functions
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinData {
    pub pin_hash: String,
    pub salt: Vec<u8>,
    #[serde(default = "crate::pin::default_pbkdf2_iterations")]
    pub kdf_iterations: u32,
    #[serde(default)]
    pub previous_kdf_iterations: Option<u32>,
    #[serde(default)]
    pub failed_attempts: u32,
}

#[derive(Debug)]
struct SuccessfulPinUnlock {
    salt: Vec<u8>,
    kdf_iterations: u32,
    previous_kdf_iterations: Option<u32>,
    updated_pin_data: Option<PinData>,
}

fn get_pin_file_path() -> String {
    let storage_dir = get_storage_dir_simple();
    format!("{}/pin.json", storage_dir)
}

fn validate_pin_code(pin: &str) -> Result<(), String> {
    if pin.len() == 6 && pin.chars().all(|c| c.is_ascii_digit()) {
        Ok(())
    } else {
        Err("PIN must be exactly 6 digits.".to_string())
    }
}

/// Check if a PIN is set
pub fn has_pin() -> bool {
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        storage.get_item("pin_data").unwrap().is_some()
    }

    #[cfg(not(feature = "web"))]
    {
        let pin_file = get_pin_file_path();
        Path::new(&pin_file).exists()
    }
}

/// Save PIN hash and salt
pub fn save_pin(pin: &str) -> Result<(), String> {
    use crate::pin::{
        calibrate_pbkdf2_iterations, derive_key_from_pin_with_iterations, generate_salt,
        hash_derived_key, unlock_session_with_key,
    };
    let total_started = std::time::Instant::now();

    validate_pin_code(pin)?;
    log::info!("🔐 Saving PIN to storage");

    let salt = generate_salt();
    let calibration_started = std::time::Instant::now();
    let kdf_iterations = calibrate_pbkdf2_iterations();
    let calibration_ms = calibration_started.elapsed().as_millis();
    let derive_started = std::time::Instant::now();
    let mut derived_key = Some(derive_key_from_pin_with_iterations(
        pin,
        &salt,
        kdf_iterations,
    ));
    let derive_ms = derive_started.elapsed().as_millis();

    let hash_started = std::time::Instant::now();
    let pin_hash = hash_derived_key(derived_key.as_ref().expect("derived key must exist"));
    let hash_ms = hash_started.elapsed().as_millis();

    let pin_data = PinData {
        pin_hash,
        salt: salt.to_vec(),
        kdf_iterations,
        previous_kdf_iterations: None,
        failed_attempts: 0,
    };

    let persist_started = std::time::Instant::now();
    let result = (|| -> Result<(), String> {
        #[cfg(feature = "web")]
        {
            use wasm_bindgen::JsCast;
            let window = web_sys::window().unwrap();
            let storage = window.local_storage().unwrap().unwrap();
            let serialized = serde_json::to_string(&pin_data)
                .map_err(|e| format!("Failed to serialize PIN data: {}", e))?;
            storage
                .set_item("pin_data", &serialized)
                .map_err(|_| "Failed to save PIN to web storage".to_string())?;
            log::info!("✅ PIN saved to web storage");
        }

        #[cfg(not(feature = "web"))]
        {
            ensure_storage_dir()
                .map_err(|e| format!("Failed to ensure storage directory: {}", e))?;

            let pin_file = get_pin_file_path();
            let serialized = serde_json::to_string_pretty(&pin_data)
                .map_err(|e| format!("Failed to serialize PIN data: {}", e))?;

            write_secure_file(&pin_file, &serialized)?;

            log::info!("✅ PIN saved to: {}", pin_file);
        }

        let persist_ms = persist_started.elapsed().as_millis();

        let unlock_started = std::time::Instant::now();
        unlock_session_with_key(derived_key.take().expect("derived key must exist"))?;
        let unlock_ms = unlock_started.elapsed().as_millis();

        let migrate_started = std::time::Instant::now();
        migrate_legacy_secret_storage()?;
        let migrate_ms = migrate_started.elapsed().as_millis();

        println!(
            "PIN save timings: calibrate={}ms iterations={} derive={}ms hash={}ms persist={}ms unlock={}ms migrate={}ms total={}ms",
            calibration_ms,
            kdf_iterations,
            derive_ms,
            hash_ms,
            persist_ms,
            unlock_ms,
            migrate_ms,
            total_started.elapsed().as_millis()
        );

        Ok(())
    })();

    if let Some(mut key) = derived_key.take() {
        key.fill(0);
    }

    result
}

fn verify_pin_fast(pin: &str) -> Result<SuccessfulPinUnlock, String> {
    use crate::pin::{
        clear_session, derive_key_from_pin_with_iterations, hash_derived_key, legacy_hash_pin,
        stored_pbkdf2_iterations, unlock_session_with_fallback_key,
    };

    if is_pin_locked() {
        return Err("PIN is locked due to too many failed attempts".to_string());
    }

    let total_started = std::time::Instant::now();
    let mut pin_data = load_pin_data()?;
    let kdf_iterations = stored_pbkdf2_iterations(pin_data.kdf_iterations);
    let previous_kdf_iterations = pin_data
        .previous_kdf_iterations
        .map(stored_pbkdf2_iterations)
        .filter(|previous| *previous != kdf_iterations);

    let derive_started = std::time::Instant::now();
    let mut derived_key = Some(derive_key_from_pin_with_iterations(
        pin,
        &pin_data.salt,
        kdf_iterations,
    ));
    let derive_ms = derive_started.elapsed().as_millis();

    let hash_started = std::time::Instant::now();
    let pin_hash = hash_derived_key(derived_key.as_ref().expect("derived key must exist"));
    let hash_ms = hash_started.elapsed().as_millis();

    let mut needs_hash_upgrade = false;
    let pin_matches = if pin_hash == pin_data.pin_hash {
        true
    } else {
        let legacy_hash_started = std::time::Instant::now();
        let legacy_pin_hash = legacy_hash_pin(pin);
        let legacy_hash_ms = legacy_hash_started.elapsed().as_millis();
        let legacy_match = legacy_pin_hash == pin_data.pin_hash;
        needs_hash_upgrade = legacy_match;
        println!(
            "PIN legacy hash fallback: matched={} took={}ms",
            legacy_match, legacy_hash_ms
        );
        legacy_match
    };

    if pin_matches {
        let persist_needed = pin_data.failed_attempts != 0 || needs_hash_upgrade;
        let updated_pin_data = if persist_needed {
            pin_data.pin_hash = pin_hash;
            pin_data.failed_attempts = 0;
            Some(pin_data.clone())
        } else {
            None
        };

        let fallback_started = std::time::Instant::now();
        let fallback_key = previous_kdf_iterations
            .map(|iterations| derive_key_from_pin_with_iterations(pin, &pin_data.salt, iterations));
        let fallback_ms = fallback_started.elapsed().as_millis();

        let unlock_started = std::time::Instant::now();
        unlock_session_with_fallback_key(
            derived_key.take().expect("derived key must exist"),
            fallback_key,
        )?;
        let unlock_ms = unlock_started.elapsed().as_millis();

        println!(
            "PIN verify fast timings: iterations={} fallback_iterations={:?} derive={}ms hash={}ms fallback={}ms unlock={}ms total={}ms persist_needed={} legacy_upgrade={}",
            kdf_iterations,
            previous_kdf_iterations,
            derive_ms,
            hash_ms,
            fallback_ms,
            unlock_ms,
            total_started.elapsed().as_millis(),
            persist_needed,
            needs_hash_upgrade
        );

        if let Some(mut key) = derived_key.take() {
            key.fill(0);
        }

        Ok(SuccessfulPinUnlock {
            salt: pin_data.salt.clone(),
            kdf_iterations,
            previous_kdf_iterations,
            updated_pin_data,
        })
    } else {
        if let Some(mut key) = derived_key.take() {
            key.fill(0);
        }
        clear_session();
        pin_data.failed_attempts += 1;
        let _ = save_pin_data(&pin_data);
        println!(
            "PIN verify failure timings: iterations={} derive={}ms hash={}ms total={}ms attempts={}/10",
            kdf_iterations,
            derive_ms,
            hash_ms,
            total_started.elapsed().as_millis(),
            pin_data.failed_attempts
        );

        if pin_data.failed_attempts >= 10 {
            Err("PIN locked due to too many failed attempts".to_string())
        } else {
            Err(format!(
                "Incorrect PIN. {} attempts remaining",
                10 - pin_data.failed_attempts
            ))
        }
    }
}

fn finalize_pin_kdf_rotation(pin_data: &mut PinData) -> Result<(), String> {
    let rotation_started = std::time::Instant::now();

    let wallets_started = std::time::Instant::now();
    let rotated_wallets = rewrap_wallets_for_current_session(&load_wallets_from_storage())?;
    save_wallets_to_storage(&rotated_wallets)?;
    let wallets_ms = wallets_started.elapsed().as_millis();

    let vaults_started = std::time::Instant::now();
    let rotated_vaults = rewrap_vaults_for_current_session(&load_quantum_vaults_from_storage())?;
    save_quantum_vaults_to_storage(&rotated_vaults)?;
    let vaults_ms = vaults_started.elapsed().as_millis();

    pin_data.previous_kdf_iterations = None;
    let persist_started = std::time::Instant::now();
    save_pin_data(pin_data)?;
    let persist_ms = persist_started.elapsed().as_millis();

    crate::pin::clear_session_fallback_keys()?;

    println!(
        "PIN KDF rotation finalized: iterations={} wallets={}ms vaults={}ms persist={}ms total={}ms",
        pin_data.kdf_iterations,
        wallets_ms,
        vaults_ms,
        persist_ms,
        rotation_started.elapsed().as_millis()
    );

    Ok(())
}

fn maybe_rotate_pin_kdf(unlock: &SuccessfulPinUnlock, pin: Option<&str>) -> Result<(), String> {
    use crate::pin::{
        calibrate_pbkdf2_iterations, derive_key_from_pin_with_iterations, hash_derived_key,
        unlock_session_with_fallback_key,
    };

    if unlock.previous_kdf_iterations.is_some() {
        let mut pin_data = unlock.updated_pin_data.clone().unwrap_or(load_pin_data()?);
        pin_data.kdf_iterations = unlock.kdf_iterations;
        pin_data.previous_kdf_iterations = unlock.previous_kdf_iterations;
        pin_data.failed_attempts = 0;

        println!(
            "PIN KDF rotation resuming: current_iterations={} previous_iterations={:?}",
            unlock.kdf_iterations, unlock.previous_kdf_iterations
        );

        return finalize_pin_kdf_rotation(&mut pin_data);
    }

    let calibrated_iterations = calibrate_pbkdf2_iterations();
    let target_iterations = calibrated_iterations.min(unlock.kdf_iterations);
    if target_iterations >= unlock.kdf_iterations {
        println!(
            "PIN KDF rotation not needed: current_iterations={} calibrated_iterations={}",
            unlock.kdf_iterations, calibrated_iterations
        );
        return Ok(());
    }

    let Some(pin) = pin else {
        println!(
            "PIN KDF rotation skipped: current_iterations={} target_iterations={} reason=missing_pin",
            unlock.kdf_iterations,
            target_iterations
        );
        return Ok(());
    };

    let rotation_started = std::time::Instant::now();
    let mut pin_data = unlock.updated_pin_data.clone().unwrap_or(load_pin_data()?);
    let previous_iterations = unlock.kdf_iterations;
    let mut next_key = Some(derive_key_from_pin_with_iterations(
        pin,
        &unlock.salt,
        target_iterations,
    ));
    let next_hash = hash_derived_key(next_key.as_ref().expect("derived key must exist"));
    let fallback_key = derive_key_from_pin_with_iterations(pin, &unlock.salt, previous_iterations);

    pin_data.pin_hash = next_hash;
    pin_data.kdf_iterations = target_iterations;
    pin_data.previous_kdf_iterations = Some(previous_iterations);
    pin_data.failed_attempts = 0;

    save_pin_data(&pin_data)?;
    unlock_session_with_fallback_key(
        next_key.take().expect("derived key must exist"),
        Some(fallback_key),
    )?;

    if let Some(mut key) = next_key.take() {
        key.fill(0);
    }

    println!(
        "PIN KDF rotation transition prepared: from_iterations={} to_iterations={} total={}ms",
        previous_iterations,
        target_iterations,
        rotation_started.elapsed().as_millis()
    );

    finalize_pin_kdf_rotation(&mut pin_data)
}

fn complete_successful_pin_unlock(
    unlock: SuccessfulPinUnlock,
    mut pin_for_rotation: Option<String>,
) -> Result<Vec<u8>, String> {
    let housekeeping_started = std::time::Instant::now();
    let salt = unlock.salt.clone();

    let result = (|| -> Result<(), String> {
        if let Some(pin_data) = unlock.updated_pin_data.as_ref() {
            let persist_started = std::time::Instant::now();
            save_pin_data(pin_data)?;
            println!(
                "PIN unlock housekeeping persisted verifier reset in {}ms",
                persist_started.elapsed().as_millis()
            );
        }

        let migrate_started = std::time::Instant::now();
        migrate_legacy_secret_storage()?;
        println!(
            "PIN unlock housekeeping migration finished in {}ms (total {}ms)",
            migrate_started.elapsed().as_millis(),
            housekeeping_started.elapsed().as_millis()
        );

        maybe_rotate_pin_kdf(&unlock, pin_for_rotation.as_deref())?;

        println!(
            "PIN unlock housekeeping complete in {}ms",
            housekeeping_started.elapsed().as_millis()
        );

        Ok(())
    })();

    if let Some(mut pin) = pin_for_rotation.take() {
        crate::pin::wipe_secret_string(&mut pin);
    }

    result.map(|_| salt)
}

#[cfg(feature = "web")]
pub async fn save_pin_async(pin: String) -> Result<(), String> {
    let mut pin = pin;
    let result = save_pin(&pin);
    crate::pin::wipe_secret_string(&mut pin);
    result
}

#[cfg(not(feature = "web"))]
pub async fn save_pin_async(pin: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let mut pin = pin;
        let result = save_pin(&pin);
        crate::pin::wipe_secret_string(&mut pin);
        result
    })
    .await
    .map_err(|e| format!("PIN save task failed: {e}"))?
}

/// Verify PIN and return salt if correct
pub fn verify_pin(pin: &str) -> Result<Vec<u8>, String> {
    let unlock = verify_pin_fast(pin)?;
    let result = complete_successful_pin_unlock(unlock, Some(pin.to_string()));
    if result.is_ok() {
        println!("PIN verified successfully");
    }
    result
}

#[cfg(feature = "web")]
pub async fn verify_pin_async(pin: String) -> Result<Vec<u8>, String> {
    let unlock = verify_pin_fast(&pin)?;
    let result = complete_successful_pin_unlock(unlock, Some(pin));
    if result.is_ok() {
        println!("PIN verified successfully");
    }
    result
}

#[cfg(not(feature = "web"))]
pub async fn verify_pin_async(pin: String) -> Result<Vec<u8>, String> {
    let (mut pin, unlock_result) = tokio::task::spawn_blocking(move || {
        let result = verify_pin_fast(&pin);
        (pin, result)
    })
    .await
    .map_err(|e| format!("PIN verification task failed: {e}"))?;

    let unlock = match unlock_result {
        Ok(unlock) => unlock,
        Err(e) => {
            crate::pin::wipe_secret_string(&mut pin);
            return Err(e);
        }
    };

    let salt = unlock.salt.clone();

    tokio::spawn(async move {
        let result =
            tokio::task::spawn_blocking(move || complete_successful_pin_unlock(unlock, Some(pin)))
                .await
                .map_err(|e| format!("PIN housekeeping task failed: {e}"))
                .and_then(|result| result);

        match result {
            Ok(_) => println!("PIN verified successfully"),
            Err(e) => println!("PIN unlock housekeeping failed: {}", e),
        }
    });

    Ok(salt)
}

pub fn verify_pin_for_sensitive_action(pin: &str) -> Result<(), String> {
    use crate::pin::{
        derive_key_from_pin_with_iterations, hash_derived_key, legacy_hash_pin,
        stored_pbkdf2_iterations,
    };

    validate_pin_code(pin)?;

    if is_pin_locked() {
        return Err("PIN is locked due to too many failed attempts".to_string());
    }

    let total_started = std::time::Instant::now();
    let mut pin_data = load_pin_data()?;
    let kdf_iterations = stored_pbkdf2_iterations(pin_data.kdf_iterations);

    let derive_started = std::time::Instant::now();
    let mut derived_key = Some(derive_key_from_pin_with_iterations(
        pin,
        &pin_data.salt,
        kdf_iterations,
    ));
    let derive_ms = derive_started.elapsed().as_millis();

    let hash_started = std::time::Instant::now();
    let pin_hash = hash_derived_key(derived_key.as_ref().expect("derived key must exist"));
    let hash_ms = hash_started.elapsed().as_millis();

    let mut needs_hash_upgrade = false;
    let pin_matches = if pin_hash == pin_data.pin_hash {
        true
    } else {
        let legacy_hash_started = std::time::Instant::now();
        let legacy_pin_hash = legacy_hash_pin(pin);
        let legacy_hash_ms = legacy_hash_started.elapsed().as_millis();
        let legacy_match = legacy_pin_hash == pin_data.pin_hash;
        needs_hash_upgrade = legacy_match;
        println!(
            "PIN step-up legacy hash fallback: matched={} took={}ms",
            legacy_match, legacy_hash_ms
        );
        legacy_match
    };

    if let Some(mut key) = derived_key.take() {
        key.fill(0);
    }

    if pin_matches {
        if pin_data.failed_attempts != 0 || needs_hash_upgrade {
            pin_data.pin_hash = pin_hash;
            pin_data.failed_attempts = 0;
            save_pin_data(&pin_data)?;
        }

        println!(
            "PIN step-up verified: iterations={} derive={}ms hash={}ms total={}ms",
            kdf_iterations,
            derive_ms,
            hash_ms,
            total_started.elapsed().as_millis()
        );

        Ok(())
    } else {
        pin_data.failed_attempts += 1;
        let _ = save_pin_data(&pin_data);

        println!(
            "PIN step-up failure: iterations={} derive={}ms hash={}ms total={}ms attempts={}/10",
            kdf_iterations,
            derive_ms,
            hash_ms,
            total_started.elapsed().as_millis(),
            pin_data.failed_attempts
        );

        if pin_data.failed_attempts >= 10 {
            crate::pin::clear_session();
            Err("PIN locked due to too many failed attempts".to_string())
        } else {
            Err(format!(
                "Incorrect PIN. {} attempts remaining",
                10 - pin_data.failed_attempts
            ))
        }
    }
}

#[cfg(feature = "web")]
pub async fn verify_pin_for_sensitive_action_async(pin: String) -> Result<(), String> {
    let mut pin = pin;
    let result = verify_pin_for_sensitive_action(&pin);
    crate::pin::wipe_secret_string(&mut pin);
    result
}

#[cfg(not(feature = "web"))]
pub async fn verify_pin_for_sensitive_action_async(pin: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let mut pin = pin;
        let result = verify_pin_for_sensitive_action(&pin);
        crate::pin::wipe_secret_string(&mut pin);
        result
    })
    .await
    .map_err(|e| format!("PIN verification task failed: {e}"))?
}

/// Check if PIN is locked
pub fn is_pin_locked() -> bool {
    if let Ok(pin_data) = load_pin_data() {
        pin_data.failed_attempts >= 10
    } else {
        false
    }
}

/// Get salt for encryption (used when PIN is already verified)
pub fn get_pin_salt() -> Result<Vec<u8>, String> {
    let pin_data = load_pin_data()?;
    Ok(pin_data.salt)
}

/// Load PIN data from storage
fn load_pin_data() -> Result<PinData, String> {
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        let data = storage
            .get_item("pin_data")
            .map_err(|_| "Failed to access web storage".to_string())?
            .ok_or_else(|| "No PIN data found".to_string())?;

        serde_json::from_str(&data).map_err(|e| format!("Failed to parse PIN data: {}", e))
    }

    #[cfg(not(feature = "web"))]
    {
        let pin_file = get_pin_file_path();
        let data =
            std::fs::read_to_string(&pin_file).map_err(|_| "No PIN data found".to_string())?;

        serde_json::from_str(&data).map_err(|e| format!("Failed to parse PIN data: {}", e))
    }
}

/// Save PIN data to storage
fn save_pin_data(pin_data: &PinData) -> Result<(), String> {
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        let serialized = serde_json::to_string(pin_data)
            .map_err(|e| format!("Failed to serialize PIN data: {}", e))?;
        storage
            .set_item("pin_data", &serialized)
            .map_err(|_| "Failed to save PIN data to web storage".to_string())?;
        Ok(())
    }

    #[cfg(not(feature = "web"))]
    {
        let pin_file = get_pin_file_path();
        let serialized = serde_json::to_string_pretty(pin_data)
            .map_err(|e| format!("Failed to serialize PIN data: {}", e))?;

        write_secure_file(&pin_file, &serialized)?;

        Ok(())
    }
}

pub fn change_pin(current_pin: &str, new_pin: &str) -> Result<(), String> {
    use crate::pin::{
        calibrate_pbkdf2_iterations, clear_session_fallback_keys,
        derive_key_from_pin_with_iterations, generate_salt, hash_derived_key,
        stored_pbkdf2_iterations, unlock_session_with_fallback_key,
    };

    validate_pin_code(current_pin)?;
    validate_pin_code(new_pin)?;

    if current_pin == new_pin {
        return Err("New PIN must be different from the current PIN.".to_string());
    }

    let unlock = verify_pin_fast(current_pin)?;
    complete_successful_pin_unlock(unlock, Some(current_pin.to_string()))?;

    let previous_pin_data = load_pin_data()?;
    let original_wallets = load_wallets_from_storage();
    let original_vaults = load_quantum_vaults_from_storage();
    let previous_iterations = stored_pbkdf2_iterations(previous_pin_data.kdf_iterations);
    let previous_key = derive_key_from_pin_with_iterations(
        current_pin,
        &previous_pin_data.salt,
        previous_iterations,
    );

    let new_salt = generate_salt();
    let new_iterations = calibrate_pbkdf2_iterations();
    let mut next_key = Some(derive_key_from_pin_with_iterations(
        new_pin,
        &new_salt,
        new_iterations,
    ));
    let next_hash = hash_derived_key(next_key.as_ref().expect("derived key must exist"));
    let next_pin_data = PinData {
        pin_hash: next_hash,
        salt: new_salt.to_vec(),
        kdf_iterations: new_iterations,
        previous_kdf_iterations: None,
        failed_attempts: 0,
    };

    let change_started = std::time::Instant::now();
    let change_result = (|| -> Result<(), String> {
        unlock_session_with_fallback_key(
            next_key.take().expect("derived key must exist"),
            Some(previous_key),
        )?;

        let rewrapped_wallets = rewrap_wallets_for_current_session(&original_wallets)?;
        let rewrapped_vaults = rewrap_vaults_for_current_session(&original_vaults)?;

        save_wallets_to_storage(&rewrapped_wallets)?;
        save_quantum_vaults_to_storage(&rewrapped_vaults)?;
        save_pin_data(&next_pin_data)?;
        clear_session_fallback_keys()?;

        println!(
            "PIN change completed: iterations={} total={}ms wallets={} vaults={}",
            new_iterations,
            change_started.elapsed().as_millis(),
            original_wallets.len(),
            original_vaults.len()
        );

        Ok(())
    })();

    if let Some(mut key) = next_key.take() {
        key.fill(0);
    }

    if let Err(error) = change_result {
        let restore_key = derive_key_from_pin_with_iterations(
            current_pin,
            &previous_pin_data.salt,
            previous_iterations,
        );
        let _ = unlock_session_with_fallback_key(restore_key, None);
        let _ = save_wallets_to_storage(&original_wallets);
        let _ = save_quantum_vaults_to_storage(&original_vaults);
        let _ = save_pin_data(&previous_pin_data);

        return Err(format!("Failed to change PIN: {}", error));
    }

    Ok(())
}

/// Remove PIN from storage
pub fn remove_pin() -> Result<(), String> {
    log::info!("🔐 Removing PIN from storage");

    let has_wallet_secrets = load_wallets_from_storage()
        .iter()
        .any(|wallet| !wallet.encrypted_key.trim().is_empty());
    let has_vault_secrets = load_quantum_vaults_from_storage()
        .iter()
        .any(|vault| !vault.private_key.trim().is_empty());

    if has_wallet_secrets || has_vault_secrets {
        return Err(
            "Removing the PIN is not supported while encrypted wallet data exists.".to_string(),
        );
    }

    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        storage
            .remove_item("pin_data")
            .map_err(|_| "Failed to remove PIN from web storage".to_string())?;
        log::info!("✅ PIN removed from web storage");
    }

    #[cfg(not(feature = "web"))]
    {
        let pin_file = get_pin_file_path();
        std::fs::remove_file(&pin_file).map_err(|e| format!("Failed to remove PIN file: {}", e))?;
        log::info!("✅ PIN removed from storage");
    }

    crate::pin::clear_session();
    Ok(())
}

#[cfg(feature = "web")]
pub async fn change_pin_async(current_pin: String, new_pin: String) -> Result<(), String> {
    let mut current_pin = current_pin;
    let mut new_pin = new_pin;
    let result = change_pin(&current_pin, &new_pin);
    crate::pin::wipe_secret_string(&mut current_pin);
    crate::pin::wipe_secret_string(&mut new_pin);
    result
}

#[cfg(not(feature = "web"))]
pub async fn change_pin_async(current_pin: String, new_pin: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let mut current_pin = current_pin;
        let mut new_pin = new_pin;
        let result = change_pin(&current_pin, &new_pin);
        crate::pin::wipe_secret_string(&mut current_pin);
        crate::pin::wipe_secret_string(&mut new_pin);
        result
    })
    .await
    .map_err(|e| format!("PIN change task failed: {e}"))?
}
