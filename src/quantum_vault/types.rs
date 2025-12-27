use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

/// Information about a quantum vault
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultInfo {
    pub address: Pubkey,
    pub balance: u64,
    pub exists: bool,
    pub is_quantum_vault: bool,
    pub owner: Pubkey,
    pub bump: u8,
    pub pubkey_hash: [u8; 32],
}

impl VaultInfo {
    pub fn balance_sol(&self) -> f64 {
        self.balance as f64 / solana_sdk::native_token::LAMPORTS_PER_SOL as f64
    }

    pub fn is_empty(&self) -> bool {
        self.balance == 0
    }
}

/// Result of a vault split operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitResult {
    pub transaction_signature: String,
    pub split_vault: Pubkey,
    pub refund_vault: Pubkey,
    pub split_amount: u64,
    pub refund_amount: u64,
}

/// Types of vault operations
#[derive(Debug, Clone, PartialEq)]
pub enum VaultOperation {
    Create,
    Deposit,
    Split,
    ViewBalance,
}

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