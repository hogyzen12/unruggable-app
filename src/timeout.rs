//! Timeout protection for transactions
//! 
//! Adds a pre-instruction that aborts the entire transaction if the current slot
//! exceeds a deadline (current_slot + slots_ahead). This prevents delayed or
//! replayed transactions from executing.
//!
//! Program ID: 23MzuyVH6EKGbUHq7GjBY6ydSCVoZQYDmzeKVdDBKWNQ
//! On-chain behavior: Returns Custom(1) if current_slot > max_slot, else returns 0

use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    sysvar,
};
use std::str::FromStr;

/// Timeout program ID (deployed on mainnet)
pub const TIMEOUT_PROGRAM_ID: &str = "23MzuyVH6EKGbUHq7GjBY6ydSCVoZQYDmzeKVdDBKWNQ";

/// Default slot window for timeout (roughly a few seconds on mainnet)
pub const DEFAULT_SLOT_WINDOW: u64 = 24;

/// Clock sysvar ID (required account for timeout instruction)
pub const CLOCK_SYSVAR: Pubkey = sysvar::clock::ID;

/// Build a timeout instruction for a specific max_slot
/// 
/// # Arguments
/// * `max_slot` - The maximum slot number before transaction expires
/// 
/// # Returns
/// An instruction that checks if current_slot <= max_slot
pub fn build_timeout_instruction(max_slot: u64) -> Result<Instruction, String> {
    let program_id = Pubkey::from_str(TIMEOUT_PROGRAM_ID)
        .map_err(|e| format!("Invalid timeout program ID: {}", e))?;
    
    // Instruction data: u64 little-endian (max_slot)
    let mut data = vec![0u8; 8];
    data.copy_from_slice(&max_slot.to_le_bytes());
    
    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(CLOCK_SYSVAR, false),
        ],
        data,
    })
}

/// Build a timeout instruction based on current slot + slots_ahead
/// 
/// # Arguments
/// * `current_slot` - The current slot number from RPC
/// * `slots_ahead` - How many slots ahead to set the deadline (default: 24)
/// 
/// # Returns
/// A timeout instruction that will abort if transaction exceeds the deadline
pub fn build_timeout_instruction_from_current(
    current_slot: u64,
    slots_ahead: u64,
) -> Result<Instruction, String> {
    let max_slot = current_slot
        .checked_add(slots_ahead)
        .ok_or_else(|| "Slot overflow when calculating timeout".to_string())?;
    
    build_timeout_instruction(max_slot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_timeout_instruction() {
        let max_slot = 1000u64;
        let ix = build_timeout_instruction(max_slot).unwrap();
        
        // Verify instruction data (u64 LE)
        assert_eq!(ix.data.len(), 8);
        assert_eq!(u64::from_le_bytes(ix.data.try_into().unwrap()), max_slot);
        
        // Verify program ID
        assert_eq!(ix.program_id.to_string(), TIMEOUT_PROGRAM_ID);
        
        // Verify accounts (single readonly Clock sysvar)
        assert_eq!(ix.accounts.len(), 1);
        assert_eq!(ix.accounts[0].pubkey, CLOCK_SYSVAR);
        assert!(!ix.accounts[0].is_signer);
        assert!(!ix.accounts[0].is_writable);
    }

    #[test]
    fn test_build_from_current() {
        let current = 100u64;
        let ahead = DEFAULT_SLOT_WINDOW;
        let ix = build_timeout_instruction_from_current(current, ahead).unwrap();
        
        let max_slot = u64::from_le_bytes(ix.data.try_into().unwrap());
        assert_eq!(max_slot, current + ahead);
    }

    #[test]
    fn test_overflow_protection() {
        let result = build_timeout_instruction_from_current(u64::MAX, 1);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("overflow"));
    }
}