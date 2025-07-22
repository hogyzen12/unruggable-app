use solana_sdk::{
    pubkey::Pubkey,
    system_instruction,
    transaction::Transaction,
    signer::Signer,
    commitment_config::CommitmentConfig,
    signature::Keypair,
    compute_budget::ComputeBudgetInstruction,
    account::Account,
};
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
use solana_client::rpc_filter::{RpcFilterType, Memcmp, MemcmpEncodedBytes};
use solana_account_decoder;
use crate::wallet::{Wallet, WalletInfo};
use crate::hardware::HardwareWallet;
use crate::validators::{ValidatorInfo, get_recommended_validators};
use std::sync::Arc;
use std::str::FromStr;
use ed25519_dalek::{SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

// Use the correct staking interface
use solana_sdk::stake::{
    instruction::{initialize, delegate_stake},
    state::{Authorized, Lockup, StakeState},
};

#[derive(Debug, Clone)]
pub struct StakeAccountInfo {
    pub stake_account_pubkey: Pubkey,
    pub transaction_signature: String,
    pub validator_vote_account: Pubkey,
    pub staked_amount: u64, // in lamports
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedStakeAccount {
    pub pubkey: Pubkey,
    pub state: StakeAccountState,
    pub balance: u64, // in lamports
    pub rent_exempt_reserve: u64,
    pub validator_vote_account: Option<Pubkey>,
    pub validator_name: String,
    pub activation_epoch: Option<u64>,
    pub deactivation_epoch: Option<u64>,
    pub stake_authority: Pubkey,
    pub withdraw_authority: Pubkey,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StakeAccountState {
    Uninitialized,
    Initialized,
    Delegated,
    RewardsPool,
}

impl std::fmt::Display for StakeAccountState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StakeAccountState::Uninitialized => write!(f, "Uninitialized"),
            StakeAccountState::Initialized => write!(f, "Initialized"),
            StakeAccountState::Delegated => write!(f, "Active"),
            StakeAccountState::RewardsPool => write!(f, "Rewards Pool"),
        }
    }
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
    AccountParsingError(String),
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
            StakingError::AccountParsingError(msg) => write!(f, "Account parsing error: {}", msg),
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
    
    // Validate minimum stake amount (0.01 SOL)
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

/// Scan all stake accounts owned by a wallet
pub async fn scan_stake_accounts(
    wallet_address: &str,
    rpc_url: Option<&str>,
) -> Result<Vec<DetailedStakeAccount>, StakingError> {
    let authority_pubkey = Pubkey::from_str(wallet_address)
        .map_err(|_| StakingError::WalletError("Invalid wallet address".to_string()))?;

    let rpc_url = rpc_url.unwrap_or("https://serene-stylish-mound.solana-mainnet.quiknode.pro/5489821bcd1547d9cd7b2d81f90c086e36e0e9f7/");
    let client = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());

    // Get all stake program accounts where the authority matches our wallet
    let stake_program_id = solana_sdk::stake::program::id();
    
    // Search for accounts where the staker authority is our wallet
    let staker_filter = RpcFilterType::Memcmp(Memcmp::new(
        44, // offset for staker authority in stake account
        MemcmpEncodedBytes::Bytes(authority_pubkey.to_bytes().to_vec()),
    ));
    
    // Search for accounts where the withdrawer authority is our wallet  
    let withdrawer_filter = RpcFilterType::Memcmp(Memcmp::new(
        76, // offset for withdrawer authority in stake account
        MemcmpEncodedBytes::Bytes(authority_pubkey.to_bytes().to_vec()),
    ));

    let config = RpcProgramAccountsConfig {
        filters: Some(vec![staker_filter]),
        account_config: RpcAccountInfoConfig {
            encoding: Some(solana_account_decoder::UiAccountEncoding::Base64),
            data_slice: None,
            commitment: Some(CommitmentConfig::confirmed()),
            min_context_slot: None,
        },
        with_context: None,
        sort_results: None,
    };

    // Get accounts where we are the staker
    let staker_accounts = client.get_program_accounts_with_config(&stake_program_id, config.clone())
        .map_err(|e| StakingError::RpcError(format!("Failed to get stake accounts: {}", e)))?;

    // Get accounts where we are the withdrawer (but different from staker accounts)
    let config_withdrawer = RpcProgramAccountsConfig {
        filters: Some(vec![withdrawer_filter]),
        account_config: config.account_config.clone(),
        with_context: None,
        sort_results: None,
    };

    let withdrawer_accounts = client.get_program_accounts_with_config(&stake_program_id, config_withdrawer)
        .map_err(|e| StakingError::RpcError(format!("Failed to get withdrawer stake accounts: {}", e)))?;

    // Combine and deduplicate accounts
    let mut all_accounts = staker_accounts;
    for (pubkey, account) in withdrawer_accounts {
        if !all_accounts.iter().any(|(existing_pubkey, _)| *existing_pubkey == pubkey) {
            all_accounts.push((pubkey, account));
        }
    }

    // Parse each stake account
    let mut detailed_accounts = Vec::new();
    let validators = get_recommended_validators();

    for (pubkey, account) in all_accounts {
        if let Ok(detailed_account) = parse_stake_account(pubkey, account, &validators).await {
            detailed_accounts.push(detailed_account);
        }
    }

    Ok(detailed_accounts)
}

/// Parse a stake account's data into a detailed structure
async fn parse_stake_account(
    pubkey: Pubkey,
    account: Account,
    validators: &[ValidatorInfo],
) -> Result<DetailedStakeAccount, StakingError> {
    // Parse the stake account data
    let stake_state = bincode::deserialize::<StakeState>(&account.data)
        .map_err(|e| StakingError::AccountParsingError(format!("Failed to deserialize stake state: {}", e)))?;

    let (state, validator_vote_account, activation_epoch, deactivation_epoch, stake_authority, withdraw_authority) = match stake_state {
        StakeState::Uninitialized => {
            (StakeAccountState::Uninitialized, None, None, None, pubkey, pubkey)
        }
        StakeState::Initialized(meta) => {
            (StakeAccountState::Initialized, None, None, None, meta.authorized.staker, meta.authorized.withdrawer)
        }
        StakeState::Stake(meta, stake) => {
            let activation_epoch = if stake.delegation.activation_epoch == u64::MAX {
                None
            } else {
                Some(stake.delegation.activation_epoch)
            };
            
            let deactivation_epoch = if stake.delegation.deactivation_epoch == u64::MAX {
                None
            } else {
                Some(stake.delegation.deactivation_epoch)
            };

            (
                StakeAccountState::Delegated,
                Some(stake.delegation.voter_pubkey),
                activation_epoch,
                deactivation_epoch,
                meta.authorized.staker,
                meta.authorized.withdrawer,
            )
        }
        StakeState::RewardsPool => {
            (StakeAccountState::RewardsPool, None, None, None, pubkey, pubkey)
        }
    };

    // Find validator name
    let validator_name = if let Some(vote_account) = validator_vote_account {
        validators
            .iter()
            .find(|v| v.vote_account == vote_account.to_string())
            .map(|v| v.name.clone())
            .unwrap_or_else(|| format!("{}...{}", 
                vote_account.to_string().chars().take(4).collect::<String>(),
                vote_account.to_string().chars().rev().take(4).collect::<String>()
            ))
    } else {
        "No Validator".to_string()
    };

    // Rent exempt reserve is typically 2.3 SOL for stake accounts
    let rent_exempt_reserve = 2_282_880; // This should be fetched from RPC but hardcoded for now

    Ok(DetailedStakeAccount {
        pubkey,
        state,
        balance: account.lamports,
        rent_exempt_reserve,
        validator_vote_account,
        validator_name,
        activation_epoch,
        deactivation_epoch,
        stake_authority,
        withdraw_authority,
    })
}

/// Get stake account information (original function - keeping for compatibility)
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
            let validators = get_recommended_validators();
            if let Ok(detailed_account) = parse_stake_account(*stake_account_pubkey, account, &validators).await {
                if let Some(validator_vote_account) = detailed_account.validator_vote_account {
                    return Ok(Some(StakeAccountInfo {
                        stake_account_pubkey: *stake_account_pubkey,
                        transaction_signature: "".to_string(), // No signature for existing accounts
                        validator_vote_account,
                        staked_amount: detailed_account.balance - detailed_account.rent_exempt_reserve,
                    }));
                }
            }
            Ok(None)
        }
        Err(_) => Ok(None), // Account doesn't exist or other error
    }
}