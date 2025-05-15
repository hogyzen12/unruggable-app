use crate::wallet::{Wallet, WalletInfo};
use serde::{Deserialize, Serialize};

// Helper functions - Make them all public
pub fn save_wallet_to_storage(wallet_info: &WalletInfo) {
    let mut wallets = load_wallets_from_storage();
    wallets.push(wallet_info.clone());
    
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        let serialized = serde_json::to_string(&wallets).unwrap();
        storage.set_item("wallets", &serialized).unwrap();
    }
    
    #[cfg(not(feature = "web"))]
    {
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let wallet_file = format!("{home_dir}/.solana_wallet_app/wallets.json");
        std::fs::create_dir_all(format!("{home_dir}/.solana_wallet_app")).ok();
        std::fs::write(wallet_file, serde_json::to_string(&wallets).unwrap()).ok();
    }
}

pub fn load_wallets_from_storage() -> Vec<WalletInfo> {
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        storage.get_item("wallets")
            .unwrap()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default()
    }
    
    #[cfg(not(feature = "web"))]
    {
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let wallet_file = format!("{home_dir}/.solana_wallet_app/wallets.json");
        std::fs::read_to_string(wallet_file)
            .ok()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default()
    }
}

pub fn import_wallet_from_key(private_key: &str, name: String) -> Result<WalletInfo, String> {
    let private_key = private_key.trim();
    
    // Try to decode the base58 key
    let key_bytes = bs58::decode(private_key)
        .into_vec()
        .map_err(|e| format!("Invalid base58 format: {}", e))?;
    
    // Create wallet with proper name
    let wallet_name = if name.is_empty() { 
        "Imported Wallet".to_string() 
    } else { 
        name 
    };
    
    let wallet = Wallet::from_private_key(&key_bytes, wallet_name)?;
    
    Ok(wallet.to_wallet_info())
}

// Add RPC storage functions
pub fn save_rpc_to_storage(rpc_url: &str) {
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        storage.set_item("custom_rpc", rpc_url).unwrap();
    }
    
    #[cfg(not(feature = "web"))]
    {
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let rpc_file = format!("{home_dir}/.solana_wallet_app/rpc.txt");
        std::fs::create_dir_all(format!("{home_dir}/.solana_wallet_app")).ok();
        std::fs::write(rpc_file, rpc_url).ok();
    }
}

pub fn load_rpc_from_storage() -> Option<String> {
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        storage.get_item("custom_rpc").unwrap()
    }
    
    #[cfg(not(feature = "web"))]
    {
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let rpc_file = format!("{home_dir}/.solana_wallet_app/rpc.txt");
        std::fs::read_to_string(rpc_file).ok()
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
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let rpc_file = format!("{home_dir}/.solana_wallet_app/rpc.txt");
        std::fs::remove_file(rpc_file).ok();
    }
}