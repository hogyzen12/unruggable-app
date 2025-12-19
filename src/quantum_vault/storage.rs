use serde::{Deserialize, Serialize};
use solana_winternitz::privkey::WinternitzPrivkey;

/// Stored quantum vault information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredQuantumVault {
    pub address: String,
    pub pubkey_hash: String,
    pub private_key: String, // base64 encoded for storage
    pub bump: u8,
    pub created_at: u64, // timestamp
    pub used: bool, // true if vault has been split (one-time use)
}

const VAULTS_STORAGE_KEY: &str = "quantum_vaults";

/// Store a quantum vault in localStorage
pub fn store_quantum_vault(vault: &StoredQuantumVault) -> Result<(), String> {
    let window = web_sys::window().ok_or("No window object")?;
    let storage = window
        .local_storage()
        .map_err(|_| "Failed to access localStorage")?
        .ok_or("localStorage not available")?;
    
    // Get existing vaults
    let mut vaults = get_all_quantum_vaults()?;
    
    // Add new vault
    vaults.push(vault.clone());
    
    // Serialize and store
    let serialized = serde_json::to_string(&vaults)
        .map_err(|e| format!("Failed to serialize vaults: {}", e))?;
    
    storage
        .set_item(VAULTS_STORAGE_KEY, &serialized)
        .map_err(|_| "Failed to store vaults")?;
    
    Ok(())
}

/// Get all quantum vaults from localStorage
pub fn get_all_quantum_vaults() -> Result<Vec<StoredQuantumVault>, String> {
    let window = web_sys::window().ok_or("No window object")?;
    let storage = window
        .local_storage()
        .map_err(|_| "Failed to access localStorage")?
        .ok_or("localStorage not available")?;
    
    match storage.get_item(VAULTS_STORAGE_KEY).map_err(|_| "Failed to read from localStorage")? {
        Some(data) => {
            serde_json::from_str(&data)
                .map_err(|e| format!("Failed to deserialize vaults: {}", e))
        }
        None => Ok(Vec::new()),
    }
}

/// Get a specific quantum vault by address
pub fn get_quantum_vault(address: &str) -> Result<Option<StoredQuantumVault>, String> {
    let vaults = get_all_quantum_vaults()?;
    Ok(vaults.into_iter().find(|v| v.address == address))
}

/// Mark a vault as used after splitting
pub fn mark_vault_as_used(address: &str) -> Result<(), String> {
    let window = web_sys::window().ok_or("No window object")?;
    let storage = window
        .local_storage()
        .map_err(|_| "Failed to access localStorage")?
        .ok_or("localStorage not available")?;
    
    let mut vaults = get_all_quantum_vaults()?;
    
    // Find and mark vault as used
    if let Some(vault) = vaults.iter_mut().find(|v| v.address == address) {
        vault.used = true;
    } else {
        return Err("Vault not found".to_string());
    }
    
    // Serialize and store
    let serialized = serde_json::to_string(&vaults)
        .map_err(|e| format!("Failed to serialize vaults: {}", e))?;
    
    storage
        .set_item(VAULTS_STORAGE_KEY, &serialized)
        .map_err(|_| "Failed to store vaults")?;
    
    Ok(())
}

/// Delete a quantum vault from storage
pub fn delete_quantum_vault(address: &str) -> Result<(), String> {
    let window = web_sys::window().ok_or("No window object")?;
    let storage = window
        .local_storage()
        .map_err(|_| "Failed to access localStorage")?
        .ok_or("localStorage not available")?;
    
    let mut vaults = get_all_quantum_vaults()?;
    vaults.retain(|v| v.address != address);
    
    // Serialize and store
    let serialized = serde_json::to_string(&vaults)
        .map_err(|e| format!("Failed to serialize vaults: {}", e))?;
    
    storage
        .set_item(VAULTS_STORAGE_KEY, &serialized)
        .map_err(|_| "Failed to store vaults")?;
    
    Ok(())
}

/// Encode private key to base64 for storage
pub fn encode_private_key(privkey: &WinternitzPrivkey) -> String {
    // This is a placeholder - in real implementation, you'd properly serialize the key
    base64::encode(format!("{:?}", privkey))
}

/// Decode private key from base64 storage
pub fn decode_private_key(encoded: &str) -> Result<WinternitzPrivkey, String> {
    // This is a placeholder - in real implementation, you'd properly deserialize the key
    let _decoded = base64::decode(encoded)
        .map_err(|e| format!("Failed to decode private key: {}", e))?;
    
    // For now, just generate a new key - this needs proper implementation
    Ok(WinternitzPrivkey::generate())
}