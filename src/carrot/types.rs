// src/carrot/types.rs
//! Type definitions for Carrot Protocol integration

use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

/// Information about a deposit operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositInfo {
    pub asset_mint: Pubkey,
    pub asset_name: String,
    pub amount: u64,
    pub estimated_crt: f64,
}

/// Information about a withdrawal operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawInfo {
    pub asset_mint: Pubkey,
    pub asset_name: String,
    pub crt_amount: u64,
    pub estimated_asset: f64,
}

/// Result of a deposit operation
#[derive(Debug, Clone)]
pub struct DepositResult {
    pub signature: String,
    pub crt_received: f64,
}

/// Result of a withdrawal operation
#[derive(Debug, Clone)]
pub struct WithdrawResult {
    pub signature: String,
    pub asset_received: f64,
}

/// Balance information for Carrot Protocol
#[derive(Debug, Clone, Default)]
pub struct CarrotBalances {
    pub usdc: f64,
    pub usdt: f64,
    pub pyusd: f64,
    pub crt: f64,
}