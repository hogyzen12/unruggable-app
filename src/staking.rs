use solana_sdk::{
    pubkey::Pubkey,
    system_instruction,
    transaction::Transaction,
    signer::Signer,
    commitment_config::CommitmentConfig,
    signature::Keypair,
    compute_budget::ComputeBudgetInstruction,
};
use solana_client::rpc_client::RpcClient;
use crate::wallet::{Wallet, WalletInfo};
use crate::hardware::HardwareWallet;
use std::sync::Arc;
use std::str::FromStr;
use ed25519_dalek::{SigningKey, VerifyingKey};

// Use the correct staking interface
use solana_sdk::stake::{
    instruction::{initialize, delegate_stake},
    state::{Authorized, Lockup},
};

#[derive(Debug, Clone)]
pub struct StakeAccountInfo {
    pub stake_account_pubkey: Pubkey,
    pub transaction_signature: String,
    pub validator_vote_account: Pubkey,
    pub staked_amount: u64, // in lamports
}

#[derive(Debug)]
pub enum StakingError {
    InvalidValidator(String),
    InvalidAmount(String),
    InsufficientBalance(String),
    TransactionFailed(String),
    RpcError(String),
    HardwareWalletError(String),
    WalletError(String),
}

impl std::fmt::Display for StakingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StakingError::InvalidValidator(msg) => write!(f, "Invalid validator: {}", msg),
            StakingError::InvalidAmount(msg) => write!(f, "Invalid amount: {}", msg),
            StakingError::InsufficientBalance(msg) => write!(f, "Insufficient balance: {}", msg),
            StakingError::TransactionFailed(msg) => write!(f, "Transaction failed: {}", msg),
            StakingError::RpcError(msg) => write!(f, "RPC error: {}", msg),
            StakingError::HardwareWalletError(msg) => write!(f, "Hardware wallet error: {}", msg),
            StakingError::WalletError(msg) => write!(f, "Wallet error: {}", msg),
        }
    }
}

impl std::error::Error for StakingError {}

/// Create and delegate a stake account
pub async fn create_stake_account(
    wallet_info: Option<&WalletInfo>,
    hardware_wallet: Option<Arc<HardwareWallet>>,
    validator_vote_account: &str,
    stake_amount_sol: f64,
    rpc_url: Option<&str>,
) -> Result<StakeAccountInfo, StakingError> {
    // Convert SOL to lamports
    let stake_amount_lamports = (stake_amount_sol * 1_000_000_000.0) as u64;
    
    // Validate minimum stake amount (0.1 SOL)
    if stake_amount_lamports < 10_000_000 {
        return Err(StakingError::InvalidAmount(
            "Minimum stake amount is 0.01 SOL".to_string()
        ));
    }

    // Parse validator vote account
    let validator_pubkey = Pubkey::from_str(validator_vote_account)
        .map_err(|_| StakingError::InvalidValidator("Invalid validator public key".to_string()))?;

    // Setup RPC client
    let rpc_url = rpc_url.unwrap_or("https://serene-stylish-mound.solana-mainnet.quiknode.pro/5489821bcd1547d9cd7b2d81f90c086e36e0e9f7/");
    let client = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());

    // Get the authority (wallet or hardware wallet)
    let authority_pubkey = if let Some(hw) = &hardware_wallet {
        let pubkey_str = hw.get_public_key().await
            .map_err(|e| StakingError::HardwareWalletError(format!("Failed to get hardware wallet pubkey: {}", e)))?;
        Pubkey::from_str(&pubkey_str)
            .map_err(|_| StakingError::HardwareWalletError("Invalid hardware wallet pubkey".to_string()))?
    } else if let Some(w) = wallet_info {
        Pubkey::from_str(&w.address)
            .map_err(|_| StakingError::WalletError("Invalid wallet address".to_string()))?
    } else {
        return Err(StakingError::WalletError("No wallet or hardware wallet provided".to_string()));
    };

    // Check balance
    let balance_lamports = client.get_balance(&authority_pubkey)
        .map_err(|e| StakingError::RpcError(format!("Failed to get balance: {}", e)))?;
    
    // Need stake amount + rent exemption + transaction fees (approximately 0.01 SOL total overhead)
    let rent_exemption = client.get_minimum_balance_for_rent_exemption(200) // stake account size
        .map_err(|e| StakingError::RpcError(format!("Failed to get rent exemption: {}", e)))?;
    
    let total_required = stake_amount_lamports + rent_exemption + 10_000_000; // 0.01 SOL for fees
    
    if balance_lamports < total_required {
        return Err(StakingError::InsufficientBalance(
            format!("Need {} SOL but only have {} SOL", 
                total_required as f64 / 1_000_000_000.0,
                balance_lamports as f64 / 1_000_000_000.0
            )
        ));
    }

    // Generate a new keypair for the stake account
    let stake_account_keypair = Keypair::new();
    let stake_account_pubkey = stake_account_keypair.pubkey();

    // Get recent blockhash
    let recent_blockhash = client.get_latest_blockhash()
        .map_err(|e| StakingError::RpcError(format!("Failed to get recent blockhash: {}", e)))?;

    // Create the transaction instructions (following the successful transaction pattern)
    let instructions = vec![
        // 1. Set compute unit price (like in the successful transaction)
        ComputeBudgetInstruction::set_compute_unit_price(125_000), // 0.13 lamports per compute unit
        
        // 2. Set compute unit limit (like in the successful transaction)  
        ComputeBudgetInstruction::set_compute_unit_limit(600_000), // 600,000 compute units
        
        // 3. Create stake account
        system_instruction::create_account(
            &authority_pubkey,
            &stake_account_pubkey,
            rent_exemption + stake_amount_lamports,
            200, // stake account size
            &solana_sdk::stake::program::id(),
        ),
        
        // 4. Initialize stake account
        initialize(
            &stake_account_pubkey,
            &Authorized {
                staker: authority_pubkey,
                withdrawer: authority_pubkey,
            },
            &Lockup::default(),
        ),
        
        // 5. Delegate stake to validator
        delegate_stake(
            &stake_account_pubkey,
            &authority_pubkey,
            &validator_pubkey,
        ),
    ];

    // Sign the transaction with the appropriate wallet
    let signature = if let Some(_hw) = &hardware_wallet {
        // For now, return error for hardware wallet - we'll implement this later
        return Err(StakingError::HardwareWalletError(
            "Hardware wallet staking not yet implemented".to_string()
        ));
    } else if let Some(w) = wallet_info {
        // Software wallet signing using existing wallet.rs methods
        let wallet = Wallet::from_wallet_info(w)
            .map_err(|e| StakingError::WalletError(format!("Failed to create wallet: {}", e)))?;
        
        // Convert ed25519_dalek SigningKey to solana_sdk Keypair
        let private_key_bytes = wallet.signing_key.to_bytes();
        let public_key_bytes = wallet.signing_key.verifying_key().to_bytes();
        
        // Create the 64-byte format that Solana SDK expects (private + public)
        let mut full_keypair_bytes = [0u8; 64];
        full_keypair_bytes[..32].copy_from_slice(&private_key_bytes);
        full_keypair_bytes[32..].copy_from_slice(&public_key_bytes);
        
        let solana_keypair = solana_sdk::signature::Keypair::from_bytes(&full_keypair_bytes)
            .map_err(|e| StakingError::WalletError(format!("Failed to create Solana keypair: {}", e)))?;
        
        // Create transaction with proper signers
        let mut transaction = Transaction::new_with_payer(&instructions, Some(&authority_pubkey));
        
        // Sign with BOTH the wallet keypair (authority) AND the stake account keypair
        // The order matters: authority first, then stake account
        transaction.sign(&[&solana_keypair, &stake_account_keypair], recent_blockhash);
        
        // Send transaction
        client.send_and_confirm_transaction(&transaction)
            .map_err(|e| StakingError::TransactionFailed(format!("Transaction failed: {}", e)))?
            .to_string()
    } else {
        return Err(StakingError::WalletError("No signing method available".to_string()));
    };

    Ok(StakeAccountInfo {
        stake_account_pubkey,
        transaction_signature: signature,
        validator_vote_account: validator_pubkey,
        staked_amount: stake_amount_lamports,
    })
}

/// Get stake account information
pub async fn get_stake_account_info(
    stake_account_pubkey: &Pubkey,
    rpc_url: Option<&str>,
) -> Result<Option<StakeAccountInfo>, StakingError> {
    let rpc_url = rpc_url.unwrap_or("https://serene-stylish-mound.solana-mainnet.quiknode.pro/5489821bcd1547d9cd7b2d81f90c086e36e0e9f7/");
    let client = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());
    
    // Get stake account data
    match client.get_account(stake_account_pubkey) {
        Ok(account) => {
            // Parse stake account data to get delegation info
            // This is a simplified version - you'd need to properly parse the stake account state
            // For now, we'll return None to indicate we need to implement proper parsing
            Ok(None)
        }
        Err(_) => Ok(None), // Account doesn't exist or other error
    }
}