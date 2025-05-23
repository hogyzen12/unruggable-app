use crate::wallet::{Wallet, WalletInfo};
use serde::{Deserialize, Serialize};
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

// Alternative simpler approach using lazy static
#[cfg(target_os = "android")]
lazy_static::lazy_static! {
    static ref ANDROID_FILES_DIR: Option<String> = {
        match get_android_files_dir() {
            Ok(dir) => {
                log::info!("✅ Android files directory initialized: {}", dir);
                Some(dir)
            }
            Err(e) => {
                log::error!("❌ Failed to initialize Android files directory: {}", e);
                None
            }
        }
    };
}

// Simplified storage directory function
fn get_storage_dir_simple() -> String {
    #[cfg(target_os = "android")]
    {
        if let Some(ref dir) = *ANDROID_FILES_DIR {
            dir.clone()
        } else {
            log::warn!("⚠️ Using fallback storage directory");
            ".".to_string()
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
            log::error!("❌ Failed to create storage directory {}: {}", storage_dir, e);
            Err(e)
        }
    }
}

pub fn save_wallet_to_storage(wallet_info: &WalletInfo) {
    log::info!("🔄 Attempting to save wallet: {}", wallet_info.name);
    
    let mut wallets = load_wallets_from_storage();
    wallets.push(wallet_info.clone());
    
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        let serialized = serde_json::to_string(&wallets).unwrap();
        storage.set_item("wallets", &serialized).unwrap();
        log::info!("✅ Wallet saved to web storage");
    }
    
    #[cfg(not(feature = "web"))]
    {
        match ensure_storage_dir() {
            Ok(_) => {
                let wallet_file = get_wallets_file_path();
                log::info!("📁 Saving to file: {}", wallet_file);
                
                match serde_json::to_string_pretty(&wallets) {
                    Ok(serialized) => {
                        match std::fs::write(&wallet_file, &serialized) {
                            Ok(_) => {
                                log::info!("✅ Wallet successfully saved to: {}", wallet_file);
                                log::info!("📊 Saved {} wallets total", wallets.len());
                                
                                // Verify the save by reading it back
                                match std::fs::read_to_string(&wallet_file) {
                                    Ok(read_back) => {
                                        if read_back == serialized {
                                            log::info!("✅ Write verification successful");
                                        } else {
                                            log::error!("❌ Write verification failed - content mismatch");
                                        }
                                    }
                                    Err(e) => {
                                        log::error!("❌ Write verification failed - cannot read back: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("❌ Failed to write wallets to {}: {}", wallet_file, e);
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("❌ Failed to serialize wallets: {}", e);
                    }
                }
            }
            Err(e) => {
                log::error!("❌ Failed to ensure storage directory: {}", e);
            }
        }
    }
}

pub fn load_wallets_from_storage() -> Vec<WalletInfo> {
    log::info!("🔄 Attempting to load wallets from storage");
    
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        let result = storage.get_item("wallets")
            .unwrap()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default();
        log::info!("📱 Loaded {} wallets from web storage", result.len());
        result
    }
    
    #[cfg(not(feature = "web"))]
    {
        let wallet_file = get_wallets_file_path();
        log::info!("📁 Loading from file: {}", wallet_file);
        
        // Check if file exists
        if !Path::new(&wallet_file).exists() {
            log::info!("ℹ️ Wallet file doesn't exist yet: {}", wallet_file);
            return Vec::new();
        }
        
        match std::fs::read_to_string(&wallet_file) {
            Ok(data) => {
                log::info!("📄 Read {} bytes from wallet file", data.len());
                match serde_json::from_str::<Vec<WalletInfo>>(&data) {
                    Ok(wallets) => {
                        log::info!("✅ Successfully loaded {} wallets", wallets.len());
                        for (i, wallet) in wallets.iter().enumerate() {
                            log::info!("  Wallet {}: {}", i + 1, wallet.name);
                        }
                        wallets
                    }
                    Err(e) => {
                        log::error!("❌ Failed to parse wallets from {}: {}", wallet_file, e);
                        log::error!("📄 File contents: {}", data);
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
    let private_key = private_key.trim();
    
    let key_bytes = bs58::decode(private_key)
        .into_vec()
        .map_err(|e| format!("Invalid base58 format: {}", e))?;
    
    let wallet_name = if name.is_empty() { 
        "Imported Wallet".to_string() 
    } else { 
        name 
    };
    
    let wallet = Wallet::from_private_key(&key_bytes, wallet_name)?;
    
    Ok(wallet.to_wallet_info())
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
    
    #[cfg(not(feature = "web"))]
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
                Ok(serialized) => {
                    match std::fs::write(&jito_file, serialized) {
                        Ok(_) => log::info!("✅ Jito settings saved to: {}", jito_file),
                        Err(e) => log::error!("❌ Failed to write Jito settings to {}: {}", jito_file, e),
                    }
                }
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
            Ok(data) => {
                match serde_json::from_str(&data) {
                    Ok(settings) => {
                        log::info!("✅ Jito settings loaded from storage");
                        settings
                    }
                    Err(e) => {
                        log::error!("❌ Failed to parse Jito settings from {}: {}", jito_file, e);
                        JitoSettings::default()
                    }
                }
            }
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

// Enhanced debug function
pub fn debug_storage_info() {
    log::info!("🔍 === STORAGE DEBUG INFO ===");
    log::info!("Platform: {}", std::env::consts::OS);
    log::info!("Architecture: {}", std::env::consts::ARCH);
    
    #[cfg(target_os = "android")]
    {
        log::info!("🤖 Android environment variables:");
        for (key, value) in std::env::vars() {
            if key.contains("ANDROID") || key.contains("DATA") || key.contains("FILES") || key.contains("STORAGE") {
                log::info!("  {}: {}", key, value);
            }
        }
        
        log::info!("🔄 Android files dir from lazy static: {:?}", *ANDROID_FILES_DIR);
    }
    
    let storage_dir = get_storage_dir_simple();
    log::info!("📁 Final storage directory: {}", storage_dir);
    log::info!("📄 Wallets file: {}", get_wallets_file_path());
    log::info!("🔗 RPC file: {}", get_rpc_file_path());
    log::info!("⚡ Jito settings file: {}", get_jito_settings_file_path());
    
    // Check if storage directory exists
    if Path::new(&storage_dir).exists() {
        log::info!("✅ Storage directory exists");
        
        // List contents
        match std::fs::read_dir(&storage_dir) {
            Ok(entries) => {
                log::info!("📋 Storage directory contents:");
                for entry in entries.flatten() {
                    if let Ok(metadata) = entry.metadata() {
                        log::info!("  - {} ({} bytes)", entry.file_name().to_string_lossy(), metadata.len());
                    } else {
                        log::info!("  - {}", entry.file_name().to_string_lossy());
                    }
                }
            }
            Err(e) => {
                log::error!("❌ Cannot read storage directory: {}", e);
            }
        }
    } else {
        log::warn!("⚠️ Storage directory does not exist");
    }
    
    // Test storage operations
    match ensure_storage_dir() {
        Ok(_) => log::info!("✅ Storage directory creation: OK"),
        Err(e) => log::error!("❌ Storage directory creation failed: {}", e),
    }
    
    log::info!("🔍 === END STORAGE DEBUG INFO ===");
}