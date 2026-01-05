// src/bonk_staking/types.rs
//! BONK staking types

/// Result of a successful stake operation
#[derive(Debug, Clone)]
pub struct StakeResult {
    pub signature: String,
    pub amount: u64,
    pub duration_days: u64,
}

/// Represents an active stake position
#[derive(Debug, Clone)]
pub struct StakePosition {
    pub receipt_address: String,
    pub amount: f64,
    pub duration_days: u64,
    pub unlock_time: String,
    pub multiplier: f64,
    pub is_unlocked: bool,
}
