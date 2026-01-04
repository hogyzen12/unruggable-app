// src/staking.rs
use solana_sdk::{
    pubkey::Pubkey,
    transaction::VersionedTransaction,
    message::{Message, VersionedMessage},
    signature::{Signature as SolanaSignature, Keypair, Signer}, // Add Signer trait
    hash::Hash,
};
use solana_system_interface::instruction as system_instruction;
use solana_stake_interface::instruction::merge;
use crate::wallet::{Wallet, WalletInfo};
use crate::hardware::HardwareWallet;
use crate::signing::{TransactionSigner, software::SoftwareSigner, hardware::HardwareSigner};
use crate::storage::get_current_jito_settings;
use crate::transaction::TransactionClient;
use crate::rpc::{ get_balance, get_minimum_balance_for_rent_exemption };
use crate::rpc::{get_stake_accounts_by_owner, get_epoch_info, StakeAccountRpcData, EpochInfo};
use crate::timeout;
use std::sync::Arc;
use std::str::FromStr;
use std::error::Error;
use std::collections::HashMap;
use bincode;
use bs58;
use reqwest::Client;
use serde_json::{Value, json};

// Use the correct staking interface
use solana_stake_interface::{
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

// Extended struct for stake_modal.rs compatibility
#[derive(Debug, Clone)]
pub struct DetailedStakeAccount {
    pub pubkey: Pubkey,
    pub balance: u64,
    pub rent_exempt_reserve: u64,
    pub state: StakeAccountState,
    pub validator_name: String,
    pub activation_epoch: Option<u64>,
    pub deactivation_epoch: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct MergeGroup {
    pub accounts: Vec<DetailedStakeAccount>,
    pub merge_type: MergeType,
    pub total_amount: u64,
    pub validator_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MergeType {
    TwoDeactivated,
    InactiveIntoActivating,
    TwoActivated { voter_pubkey: String },
    TwoActivatingSameEpoch { voter_pubkey: String, activation_epoch: u64 },
}

impl std::fmt::Display for MergeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MergeType::TwoDeactivated => write!(f, "Merge Deactivated Accounts"),
            MergeType::InactiveIntoActivating => write!(f, "Merge into Activating Account"),
            MergeType::TwoActivated { .. } => write!(f, "Merge Active Accounts"),
            MergeType::TwoActivatingSameEpoch { activation_epoch, .. } => {
                write!(f, "Merge Activating Accounts (Epoch {})", activation_epoch)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
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

/// Enhanced staking client that supports Jito transactions
pub struct StakingClient {
    transaction_client: TransactionClient,
    rpc_url: String, // Keep this if needed for raw JSON-RPC fallback
}

impl StakingClient {
    /// Create a new staking client
    pub fn new(rpc_url: Option<&str>) -> Self {
        let url = rpc_url.unwrap_or("https://johna-k3cr1v-fast-mainnet.helius-rpc.com");
        Self {
            transaction_client: TransactionClient::new(Some(url)),
            rpc_url: url.to_string(),
        }
    }

    /// Apply Jito modifications to staking instructions (same as transfer logic)
    fn apply_jito_modifications(
        &self,
        from_pubkey: &Pubkey,
        instructions: &mut Vec<solana_sdk::instruction::Instruction>,
    ) -> Result<(), Box<dyn Error>> {
        // First Jito address (as per your existing implementation)
        let jito_address1 = Pubkey::from_str("juLesoSmdTcRtzjCzYzRoHrnF8GhVu6KCV7uxq7nJGp")?;
        
        // Second Jito address (as per your existing implementation)
        let jito_address2 = Pubkey::from_str("DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL")?;

        // Add two transfer instructions as tips to Jito (same as transfers)
        let tip_instruction1 = system_instruction::transfer(
            from_pubkey,
            &jito_address1,
            100_000, // 0.0001 SOL in lamports
        );

        let tip_instruction2 = system_instruction::transfer(
            from_pubkey,
            &jito_address2,
            100_000, // 0.0001 SOL in lamports
        );

        // Add the tip instructions to the existing instructions list
        instructions.push(tip_instruction1);
        instructions.push(tip_instruction2);

        println!("Added Jito tip instructions to staking transaction");
        Ok(())
    }

    /// Send a signed staking transaction with Jito support (same logic as transfers)
    async fn send_staking_transaction(&self, signed_tx: &str) -> Result<String, Box<dyn Error>> {
        // Check Jito settings
        let jito_settings = get_current_jito_settings();
        
        // Use the same transaction sending logic as regular transfers
        let client = Client::new();
        
        // Prepare the request, potentially with Jito-specific parameters
        let request = if jito_settings.jito_tx {
            // If JitoTx is enabled, use base58 encoding and skip preflight as required by Jito
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "sendTransaction",
                "params": [
                    signed_tx,
                    {
                        "encoding": "base58",
                        "skipPreflight": true, // Jito requires skipPreflight=true
                        "preflightCommitment": "finalized"
                    }
                ]
            })
        } else {
            // Regular transaction submission
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "sendTransaction",
                "params": [
                    signed_tx,
                    {
                        "encoding": "base58",
                        "skipPreflight": false,
                        "preflightCommitment": "finalized"
                    }
                ]
            })
        };

        let response = client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await?;

        let json: Value = response.json().await?;
        
        println!("Send staking transaction response: {:?}", json);
        
        if let Some(error) = json.get("error") {
            Err(format!("Staking transaction error: {:?}", error).into())
        } else if let Some(result) = json["result"].as_str() {
            Ok(result.to_string())
        } else {
            Err(format!("Unknown error sending staking transaction: {:?}", json).into())
        }
    }

    /// Create and delegate a stake account with Jito support
    pub async fn create_stake_account_with_jito(
        &self,
        signer: &dyn TransactionSigner,
        validator_vote_account: &str,
        stake_amount_sol: f64,
        is_hardware_wallet: bool,
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

        // Get the authority public key from signer
        let authority_pubkey_str = signer.get_public_key().await
            .map_err(|e| StakingError::WalletError(format!("Failed to get public key: {}", e)))?;
        let authority_pubkey = Pubkey::from_str(&authority_pubkey_str)
            .map_err(|_| StakingError::WalletError("Invalid wallet address".to_string()))?;

        let balance_lamports = get_balance(&authority_pubkey.to_string(), Some(&self.rpc_url)).await
            .map_err(|e| StakingError::RpcError(format!("Failed to get balance: {}", e)))?;
        
        let account_size = 200;

        let rent_exemption = get_minimum_balance_for_rent_exemption(account_size, Some(&self.rpc_url))
            .await
            .map_err(|e| StakingError::RpcError(format!("Failed to get rent exemption: {}", e)))?;

    
        // Calculate total required including Jito tips if enabled
        let jito_settings = get_current_jito_settings();
        let jito_tip_amount = if jito_settings.jito_tx { 200_000 } else { 0 }; // 0.0002 SOL total for tips
        let total_required = stake_amount_lamports + rent_exemption + 5_000_000 + jito_tip_amount; // 0.005 SOL for fees + Jito tips
        
        if balance_lamports < (total_required as f64 / 1_000_000_000.0) {
            return Err(StakingError::InsufficientBalance(
                format!("Need {} SOL but only have {} SOL (including Jito tips if enabled)", 
                    total_required as f64 / 1_000_000_000.0,
                    balance_lamports as f64 / 1_000_000_000.0
                )
            ));
        }

        // Generate a new keypair for the stake account
        let stake_account_keypair = Keypair::new();
        let stake_account_pubkey = stake_account_keypair.pubkey(); // This should work now with Signer trait

        // Get current slot and build timeout instruction (FIRST)
        let current_slot = self.transaction_client.get_current_slot().await
            .map_err(|e| StakingError::RpcError(format!("Failed to get current slot: {}", e)))?;
        let timeout_ix = timeout::build_timeout_instruction_from_current(
            current_slot,
            timeout::DEFAULT_SLOT_WINDOW,
        )
            .map_err(|e| StakingError::TransactionFailed(format!("Failed to build timeout instruction: {}", e)))?;
        println!("Added timeout protection: current_slot={}, max_slot={}", 
            current_slot, current_slot + timeout::DEFAULT_SLOT_WINDOW);

        // Get recent blockhash
        let recent_blockhash = self.transaction_client.get_recent_blockhash().await
            .map_err(|e| StakingError::RpcError(format!("Failed to get recent blockhash: {}", e)))?;

        // Create the staking instructions with timeout FIRST
        let mut instructions = vec![
            // 0. Timeout protection (FIRST)
            timeout_ix,
            
            // 1. Create stake account
            system_instruction::create_account(
                &authority_pubkey,
                &stake_account_pubkey,
                rent_exemption + stake_amount_lamports,
                200, // stake account size
                &Pubkey::from_str("Stake11111111111111111111111111111111111111").unwrap(),
            ),
            
            // 2. Initialize stake account
            initialize(
                &stake_account_pubkey,
                &Authorized {
                    staker: authority_pubkey,
                    withdrawer: authority_pubkey,
                },
                &Lockup::default(),
            ),
            
            // 3. Delegate stake to validator
            delegate_stake(
                &stake_account_pubkey,
                &authority_pubkey,
                &validator_pubkey,
            ),
        ];

        // Apply Jito modifications if JitoTx is enabled AND not using hardware wallet
        if jito_settings.jito_tx && !is_hardware_wallet {
            println!("JitoTx is enabled, applying Jito modifications to staking transaction");
            self.apply_jito_modifications(&authority_pubkey, &mut instructions)
                .map_err(|e| StakingError::TransactionFailed(format!("Failed to apply Jito modifications: {}", e)))?;
        } else if is_hardware_wallet {
            println!("Hardware wallet detected - skipping Jito tips");
        }

        // Create a message with all instructions
        let mut message = Message::new(&instructions, Some(&authority_pubkey));
        message.recent_blockhash = recent_blockhash;
        
        // Create a VersionedTransaction with empty signatures
        let mut transaction = VersionedTransaction {
            signatures: vec![SolanaSignature::default(); message.header.num_required_signatures as usize],
            message: VersionedMessage::Legacy(message),
        };
        
        println!("Number of signatures expected for staking transaction: {}", transaction.message.header().num_required_signatures);
        
        // Serialize the transaction message for signing
        let message_bytes = transaction.message.serialize();
        
        // Sign the message with our signer (wallet or hardware wallet)
        let signature_bytes = signer.sign_message(&message_bytes).await
            .map_err(|e| StakingError::WalletError(format!("Failed to sign transaction: {}", e)))?;
        
        // Convert to solana signature (expect exactly 64 bytes)
        if signature_bytes.len() != 64 {
            return Err(StakingError::WalletError(format!("Invalid signature length: expected 64, got {}", signature_bytes.len())));
        }
        
        let mut sig_array = [0u8; 64];
        sig_array.copy_from_slice(&signature_bytes);
        let solana_signature = SolanaSignature::from(sig_array);
        
        // We need to handle the stake account keypair separately since it's generated locally
        // Create a transaction and sign with BOTH the wallet signer AND the stake account keypair
        let legacy_message = match &transaction.message {
            VersionedMessage::Legacy(msg) => msg.clone(),
            _ => return Err(StakingError::TransactionFailed("Expected legacy message".to_string())),
        };
        
        let mut legacy_transaction = solana_sdk::transaction::Transaction {
            signatures: vec![SolanaSignature::default(); legacy_message.header.num_required_signatures as usize],
            message: legacy_message,
        };
        
        // Sign with the stake account keypair first
        legacy_transaction.partial_sign(&[&stake_account_keypair], recent_blockhash);
        
        // Then manually add the wallet signature
        // The wallet signature should be the first signature since the wallet is the fee payer
        legacy_transaction.signatures[0] = solana_signature;
        
        // Serialize the entire transaction with signatures
        let serialized_transaction = bincode::serialize(&legacy_transaction)
            .map_err(|e| StakingError::TransactionFailed(format!("Failed to serialize transaction: {}", e)))?;
        let encoded_transaction = bs58::encode(serialized_transaction).into_string();
        
        println!("Serialized staking transaction: {} bytes", encoded_transaction.len());
        
        // Send the transaction using our Jito-aware sending method
        let signature = self.send_staking_transaction(&encoded_transaction).await
            .map_err(|e| StakingError::TransactionFailed(format!("Failed to send staking transaction: {}", e)))?;

        Ok(StakeAccountInfo {
            stake_account_pubkey,
            transaction_signature: signature,
            validator_vote_account: validator_pubkey,
            staked_amount: stake_amount_lamports,
        })
    }
}

/// Create and delegate a stake account (updated to use Jito)
pub async fn create_stake_account(
    wallet_info: Option<&WalletInfo>,
    hardware_wallet: Option<Arc<HardwareWallet>>,
    validator_vote_account: &str,
    stake_amount_sol: f64,
    rpc_url: Option<&str>,
) -> Result<StakeAccountInfo, StakingError> {
    let staking_client = StakingClient::new(rpc_url);
    
    let is_hardware_wallet = hardware_wallet.is_some();
    
    // Create the appropriate signer based on what's provided
    let signer: Box<dyn TransactionSigner> = if let Some(ref hw) = hardware_wallet {
        // Create HardwareSigner from the HardwareWallet
        Box::new(HardwareSigner::from_wallet(hw.clone()))
    } else if let Some(w) = wallet_info {
        let wallet = Wallet::from_wallet_info(w)
            .map_err(|e| StakingError::WalletError(format!("Failed to create wallet: {}", e)))?;
        // Create SoftwareSigner from the Wallet
        Box::new(SoftwareSigner::new(wallet))
    } else {
        return Err(StakingError::WalletError("No wallet or hardware wallet provided".to_string()));
    };

    staking_client.create_stake_account_with_jito(signer.as_ref(), validator_vote_account, stake_amount_sol, is_hardware_wallet).await
}

/// Convert RPC stake account data to DetailedStakeAccount format
fn convert_rpc_to_detailed_stake_account(
    rpc_data: &StakeAccountRpcData,
    current_epoch: u64,
) -> Result<DetailedStakeAccount, StakingError> {
    let pubkey = Pubkey::from_str(&rpc_data.pubkey)
        .map_err(|_| StakingError::RpcError("Invalid stake account pubkey".to_string()))?;
    
    let balance = rpc_data.account.lamports;
    let rent_exempt_reserve = rpc_data.account.data.parsed.info.meta.rent_exempt_reserve
        .parse::<u64>()
        .unwrap_or(0);
    
    // Extract activation and deactivation epochs
    let (activation_epoch, deactivation_epoch, validator_name) = if let Some(stake_details) = &rpc_data.account.data.parsed.info.stake {
        let activation_epoch = stake_details.delegation.activation_epoch
            .parse::<u64>()
            .ok();
        let deactivation_epoch = stake_details.delegation.deactivation_epoch
            .parse::<u64>()
            .ok()
            .filter(|&epoch| epoch != u64::MAX); // Filter out max value (means no deactivation)
        
        // For now, use vote account as validator name (you could enhance this with a lookup)
        let validator_name = format!("Validator {}", &stake_details.delegation.voter[0..8]);
        
        (activation_epoch, deactivation_epoch, validator_name)
    } else {
        (None, None, "Unknown Validator".to_string())
    };
    
    // Determine stake account state
    let state = if let Some(stake_details) = &rpc_data.account.data.parsed.info.stake {
        let activation_epoch_num = stake_details.delegation.activation_epoch
            .parse::<u64>()
            .unwrap_or(u64::MAX);
        let deactivation_epoch_num = stake_details.delegation.deactivation_epoch
            .parse::<u64>()
            .unwrap_or(u64::MAX);
        
        if deactivation_epoch_num != u64::MAX && deactivation_epoch_num <= current_epoch {
            StakeAccountState::Uninitialized // Deactivated
        } else if activation_epoch_num <= current_epoch {
            StakeAccountState::Delegated // Active
        } else {
            StakeAccountState::Initialized // Activating
        }
    } else {
        StakeAccountState::Initialized
    };
    
    Ok(DetailedStakeAccount {
        pubkey,
        balance,
        rent_exempt_reserve,
        state,
        validator_name,
        activation_epoch,
        deactivation_epoch,
    })
}

/// Scan for stake accounts using the new RPC function
pub async fn scan_stake_accounts(
    wallet_address: &str,
    rpc_url: Option<&str>,
) -> Result<Vec<DetailedStakeAccount>, StakingError> {
    println!("🔍 Starting stake account scan for wallet: {}", wallet_address);
    
    // Get current epoch info to determine activation status
    let epoch_info = get_epoch_info(rpc_url).await
        .map_err(|e| StakingError::RpcError(format!("Failed to get epoch info: {}", e)))?;
    
    println!("📅 Current epoch: {}", epoch_info.epoch);
    
    // Get stake accounts using the new RPC function
    let rpc_stake_accounts = get_stake_accounts_by_owner(wallet_address, rpc_url).await
        .map_err(|e| StakingError::RpcError(format!("Failed to get stake accounts: {}", e)))?;
    
    println!("🎯 Found {} raw stake accounts from RPC", rpc_stake_accounts.len());
    
    // Convert RPC data to our detailed format
    let mut detailed_accounts = Vec::new();
    
    for rpc_account in &rpc_stake_accounts {
        match convert_rpc_to_detailed_stake_account(rpc_account, epoch_info.epoch) {
            Ok(detailed) => {
                detailed_accounts.push(detailed);
            }
            Err(e) => {
                println!("⚠️  Failed to convert stake account {}: {}", rpc_account.pubkey, e);
            }
        }
    }
    
    // Log summary
    let total_staked: u64 = detailed_accounts.iter()
        .map(|acc| acc.balance.saturating_sub(acc.rent_exempt_reserve))
        .sum();
    
    let active_count = detailed_accounts.iter()
        .filter(|acc| matches!(acc.state, StakeAccountState::Delegated))
        .count();
    
    println!("📈 SUMMARY: {} accounts, {} active, {:.6} SOL total staked", 
        detailed_accounts.len(), 
        active_count, 
        total_staked as f64 / 1_000_000_000.0
    );
    
    Ok(detailed_accounts)
}

/// Get stake account information
pub async fn get_stake_account_info(
    _stake_account_pubkey: &Pubkey,
    _rpc_url: Option<&str>,
) -> Result<Option<StakeAccountInfo>, StakingError> {
    // This is a placeholder - you would implement actual stake account parsing here
    // For now, we'll return None
    Ok(None)
}

/// Find all possible merge groups from a list of stake accounts
/// Groups active stake accounts by validator if there are 2 or more
/// Find all possible merge groups from a list of stake accounts
/// Groups active stake accounts by validator if there are 2 or more
pub fn find_mergeable_stake_accounts(
    accounts: &[DetailedStakeAccount],
    _current_epoch: u64,
) -> Vec<MergeGroup> {
    let mut merge_groups = Vec::new();
    
    println!("🔍 Analyzing {} accounts for merge opportunities...", accounts.len());
    
    // Group ACTIVE accounts by validator
    let mut by_validator: HashMap<String, Vec<DetailedStakeAccount>> = HashMap::new();
    
    for account in accounts {
        if account.state == StakeAccountState::Delegated {
            let validator_key = account.validator_name.clone();
            by_validator.entry(validator_key).or_insert_with(Vec::new).push(account.clone());
        }
    }
    
    for (validator_name, active_accounts) in by_validator {
        if active_accounts.len() >= 2 {
            // Print before moving values
            println!("✅ Found merge group for validator {} with {} active accounts", 
                     validator_name, active_accounts.len());
            
            let total_amount: u64 = active_accounts.iter()
                .map(|acc| acc.balance.saturating_sub(acc.rent_exempt_reserve))
                .sum();
            
            // Extract voter_pubkey from validator_name (trim "Validator " prefix)
            let voter_pubkey = validator_name.trim_start_matches("Validator ").trim().to_string();
            
            merge_groups.push(MergeGroup {
                accounts: active_accounts,  // Move here after printing
                merge_type: MergeType::TwoActivated { voter_pubkey },
                total_amount,
                validator_name,  // Move string here
            });
        }
    }
    
    let total_groups = merge_groups.len();
    println!("🎯 Found {} merge opportunities total", total_groups);
    
    if total_groups == 0 {
        println!("💡 No merge opportunities found. This is normal if:");
        println!("   - No validators have 2+ active stake accounts");
    }
    
    merge_groups
}

/// Build merge transaction instructions for a group of mergeable stake accounts
pub async fn build_merge_transaction(
    merge_group: &MergeGroup,
    authority_pubkey: &Pubkey,
    _rpc_url: Option<&str>,
) -> Result<Vec<solana_sdk::instruction::Instruction>, StakingError> {
    println!("🔗 Building merge transaction for {} accounts", merge_group.accounts.len());
    
    if merge_group.accounts.len() < 2 {
        return Err(StakingError::InvalidAmount("Need at least 2 accounts to merge".to_string()));
    }

    let mut instructions = Vec::new();
    let destination_account = &merge_group.accounts[0];
    
    // Create merge instructions: merge all accounts into the first one
    for source_account in merge_group.accounts.iter().skip(1) {
        let merge_instructions = merge(
            &destination_account.pubkey,  // destination (keep this one)
            &source_account.pubkey,       // source (will be closed after merge)
            authority_pubkey,             // stake authority
        );
        // The merge function returns Vec<Instruction>, so extend instead of push
        instructions.extend(merge_instructions);
    }
    
    println!("✅ Built {} merge instructions", instructions.len());
    Ok(instructions)
}

/// Execute merge operation for a group of stake accounts
pub async fn merge_stake_accounts(
    merge_group: &MergeGroup,
    wallet_info: Option<&WalletInfo>,
    hardware_wallet: Option<Arc<HardwareWallet>>,
    rpc_url: Option<&str>,
) -> Result<String, StakingError> {
    println!("🔄 MERGE OPERATION: Merging {} accounts", merge_group.accounts.len());
    
    // Create signer (reuse existing pattern)
    let signer: Box<dyn TransactionSigner> = if let Some(ref hw) = hardware_wallet {
        Box::new(HardwareSigner::from_wallet(hw.clone()))
    } else if let Some(w) = wallet_info {
        let wallet = Wallet::from_wallet_info(w)
            .map_err(|e| StakingError::WalletError(format!("Failed to create wallet: {}", e)))?;
        Box::new(SoftwareSigner::new(wallet))
    } else {
        return Err(StakingError::WalletError("No wallet provided".to_string()));
    };

    // Get authority pubkey
    let authority_pubkey_str = signer.get_public_key().await
        .map_err(|e| StakingError::WalletError(format!("Failed to get public key: {}", e)))?;
    let authority_pubkey = Pubkey::from_str(&authority_pubkey_str)
        .map_err(|_| StakingError::WalletError("Invalid wallet address".to_string()))?;

    // Build merge instructions
    let mut instructions = build_merge_transaction(merge_group, &authority_pubkey, rpc_url).await?;

    // Get current slot and add timeout instruction (FIRST)
    let staking_client = StakingClient::new(rpc_url);
    let current_slot = staking_client.transaction_client.get_current_slot().await
        .map_err(|e| StakingError::RpcError(format!("Failed to get current slot: {}", e)))?;
    let timeout_ix = timeout::build_timeout_instruction_from_current(
        current_slot,
        timeout::DEFAULT_SLOT_WINDOW,
    )
        .map_err(|e| StakingError::TransactionFailed(format!("Failed to build timeout instruction: {}", e)))?;
    println!("Added timeout protection: current_slot={}, max_slot={}", 
        current_slot, current_slot + timeout::DEFAULT_SLOT_WINDOW);
    
    // Prepend timeout instruction
    instructions.insert(0, timeout_ix);

    // Apply Jito tips if enabled AND not using hardware wallet
    let jito_settings = get_current_jito_settings();
    let is_hardware_wallet = hardware_wallet.is_some();
    if jito_settings.jito_tx && !is_hardware_wallet {
        println!("Applying Jito modifications");
        staking_client.apply_jito_modifications(&authority_pubkey, &mut instructions)
            .map_err(|e| StakingError::TransactionFailed(format!("Jito error: {}", e)))?;
    } else if is_hardware_wallet {
        println!("Hardware wallet detected - skipping Jito tips");
    }

    // Create and sign transaction (reuse existing pattern)
    let recent_blockhash = staking_client.transaction_client.get_recent_blockhash().await
        .map_err(|e| StakingError::RpcError(format!("Failed to get blockhash: {}", e)))?;

    let mut message = Message::new(&instructions, Some(&authority_pubkey));
    message.recent_blockhash = recent_blockhash;
    
    let transaction = VersionedTransaction {
        signatures: vec![SolanaSignature::default(); message.header.num_required_signatures as usize],
        message: VersionedMessage::Legacy(message),
    };
    
    // Sign transaction
    let message_bytes = transaction.message.serialize();
    let signature_bytes = signer.sign_message(&message_bytes).await
        .map_err(|e| StakingError::WalletError(format!("Failed to sign: {}", e)))?;
    
    if signature_bytes.len() != 64 {
        return Err(StakingError::WalletError("Invalid signature length".to_string()));
    }
    
    let mut sig_array = [0u8; 64];
    sig_array.copy_from_slice(&signature_bytes);
    let solana_signature = SolanaSignature::from(sig_array);
    
    let mut signed_transaction = transaction;
    signed_transaction.signatures[0] = solana_signature;
    
    // Send transaction
    let serialized = bincode::serialize(&signed_transaction)
        .map_err(|e| StakingError::TransactionFailed(format!("Serialization failed: {}", e)))?;
    let encoded = bs58::encode(serialized).into_string();
    
    let signature = staking_client.send_staking_transaction(&encoded).await
        .map_err(|e| StakingError::TransactionFailed(format!("Send failed: {}", e)))?;

    println!("✅ Merge completed: {}", signature);
    Ok(signature)
}

