// src/squads/types.rs
//! Type definitions and re-exports for Squads integration

use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

// Re-export core Squads types
pub use squads_v4_client_v3::types::{Member, Permissions, ProposalStatus};
pub use squads_v4_client_v3::accounts::{Multisig, Proposal};

// API Response types for Squads V4 API
#[derive(Debug, Clone, Deserialize)]
pub struct SquadsApiResponse {
    pub address: String,
    pub account: SquadsAccount,
    #[serde(rename = "defaultVault")]
    pub default_vault: String,
    pub metadata: Option<SquadsMetadata>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SquadsAccount {
    #[serde(rename = "createKey")]
    pub create_key: String,
    #[serde(rename = "configAuthority")]
    pub config_authority: String,
    pub threshold: u16,
    #[serde(rename = "timeLock")]
    pub time_lock: u64,
    #[serde(rename = "transactionIndex")]
    pub transaction_index: String,
    #[serde(rename = "staleTransactionIndex")]
    pub stale_transaction_index: String,
    #[serde(rename = "rentCollector")]
    pub rent_collector: Option<String>,
    pub bump: u8,
    pub members: Vec<SquadsApiMember>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SquadsApiMember {
    pub key: String,
    pub permissions: SquadsPermissions,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SquadsPermissions {
    pub mask: u8,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SquadsMetadata {
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: Option<u64>,
    pub image: Option<String>,
}

/// Information about a multisig account owned by the user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultisigInfo {
    /// The multisig account address
    pub address: Pubkey,
    /// The approval threshold (e.g., 2 for 2-of-3)
    pub threshold: u16,
    /// Members of the multisig
    pub members: Vec<Member>,
    /// Current transaction index
    pub transaction_index: u64,
    /// Whether the current wallet is a member
    pub is_member: bool,
    /// The default vault address
    pub vault_address: Pubkey,
    /// SOL balance in the vault
    pub vault_balance: f64,
    /// Multisig name from metadata
    pub name: String,
}

/// Information about a pending transaction that needs approval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingTransaction {
    /// The multisig this transaction belongs to
    pub multisig: Pubkey,
    /// Transaction index
    pub transaction_index: u64,
    /// Proposal address
    pub proposal: Pubkey,
    /// Transaction address
    pub transaction: Pubkey,
    /// Current proposal status
    pub status: ProposalStatus,
    /// Number of approvals received
    pub approved_count: u16,
    /// Whether the current wallet has approved
    pub has_approved: bool,
    /// Brief description of the transaction
    pub description: String,
}

/// Result of a transaction approval
#[derive(Debug, Clone)]
pub struct ApprovalResult {
    /// Transaction signature
    pub signature: String,
    /// Whether this approval met the threshold
    pub threshold_met: bool,
    /// Updated approval count
    pub approval_count: u16,
}
