#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Stored vault data for app
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredVault {
    pub name: String,
    pub address: String,
    pub pubkey_hash: String, // hex encoded
    pub private_key: String, // base64 encoded for storage
    pub bump: u8,
    pub created_at: u64,
    pub used: bool,
}
