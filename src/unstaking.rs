// src/unstaking.rs
// Simple standalone instant unstaking implementation

use solana_sdk::{
    pubkey::Pubkey,
    transaction::VersionedTransaction,
    message::{Message, VersionedMessage},
    signature::{Signature as SolanaSignature},
    instruction::{AccountMeta, Instruction},
    sysvar,
};
use solana_system_interface::instruction as system_instruction;
use solana_compute_budget_interface::ComputeBudgetInstruction as ComputeBudgetInstructionInterface;

// Helper to convert ComputeBudgetInstruction from interface to SDK Instruction
// ComputeBudgetInstructionInterface returns solana_instruction::Instruction which uses different types
// System instruction interface already returns solana_sdk::Instruction, no conversion needed
fn convert_compute_budget_instruction(interface_ix: solana_instruction::Instruction) -> Instruction {
    Instruction {
        program_id: Pubkey::new_from_array(interface_ix.program_id.to_bytes()),
        accounts: interface_ix.accounts.iter().map(|meta| AccountMeta {
            pubkey: Pubkey::new_from_array(meta.pubkey.to_bytes()),
            is_signer: meta.is_signer,
            is_writable: meta.is_writable,
        }).collect(),
        data: interface_ix.data,
    }
}
use crate::wallet::{Wallet, WalletInfo};
use crate::hardware::HardwareWallet;
use crate::signing::{TransactionSigner, software::SoftwareSigner, hardware::HardwareSigner};
use crate::storage::get_current_jito_settings;
use crate::transaction::TransactionClient;
use crate::staking::{DetailedStakeAccount, StakeAccountState, StakingError};
use std::sync::Arc;
use std::str::FromStr;
use std::error::Error;
use sha2::{Digest, Sha256};
use bincode;
use bs58;

// Constants for instant unstake program
const PROGRAM_ID_STR: &str = "2rU1oCHtQ7WJUvy15tKtFvxdYNNSc3id7AzUcjeFSddo";
const POOL_PDA_STR: &str = "9nyw5jxhzuSs88HxKJyDCsWBZMhxj2uNXsFcyHF5KBAb";
const SOL_VAULT_STR: &str = "6RLKARrt6oPCyuMCdYdUHmJxd4wUa6ZeyiC8VSMcYxRv";
const SYSTEM_PROGRAM_ID_STR: &str = "11111111111111111111111111111111";

/// Generate Anchor discriminator for the instant unstake instruction
fn anchor_discriminator(name_snake: &str) -> [u8; 8] {
    let mut h = Sha256::new();
    h.update(format!("global:{name_snake}"));
    let d = h.finalize();
    let mut out = [0u8; 8];
    out.copy_from_slice(&d[..8]);
    out
}

/// Build the instant unstake instruction - EXACT copy from working CLI
fn build_instant_unstake_instruction(
    user: &Pubkey,
    stake_account: &Pubkey,
) -> Result<Instruction, StakingError> {
    let program_id = Pubkey::from_str(PROGRAM_ID_STR)
        .map_err(|_| StakingError::RpcError("Invalid program ID".to_string()))?;
    let pool_pda = Pubkey::from_str(POOL_PDA_STR)
        .map_err(|_| StakingError::RpcError("Invalid pool PDA".to_string()))?;
    let sol_vault = Pubkey::from_str(SOL_VAULT_STR)
        .map_err(|_| StakingError::RpcError("Invalid SOL vault".to_string()))?;
    let system_program_id = Pubkey::from_str(SYSTEM_PROGRAM_ID_STR)
        .map_err(|_| StakingError::RpcError("Invalid system program ID".to_string()))?;

    // Pattern 4: "stake_account_info" + stake_account (from working CLI)
    let stake_account_info = {
        let seeds = vec![b"stake_account_info".as_ref(), stake_account.as_ref()];
        let (derived, _) = Pubkey::find_program_address(&seeds, &program_id);
        println!("Using stake account info PDA: {}", derived);
        derived
    };

    // The error shows we need F3yy3FVpwq9MV321AzALFcDWZp9XBBHbMas3t4AtEtCW as manager fee account
    // This might be derived differently - let's try a few patterns
    let manager_fee_account = {
        // Try different PDA patterns for manager fee account
        let test_patterns = vec![
            // Pattern 1: "manager_fee" seed
            vec![b"manager_fee".as_ref()],
            // Pattern 2: "fee" seed  
            vec![b"fee".as_ref()],
            // Pattern 3: "manager" seed
            vec![b"manager".as_ref()],
            // Pattern 4: pool + "fee"
            vec![pool_pda.as_ref(), b"fee".as_ref()],
            // Pattern 5: "manager_fee_account" seed
            vec![b"manager_fee_account".as_ref()],
        ];

        let target_manager_fee = "F3yy3FVpwq9MV321AzALFcDWZp9XBBHbMas3t4AtEtCW";
        let mut found_manager_fee = None;

        for (i, seeds) in test_patterns.iter().enumerate() {
            let (derived, _) = Pubkey::find_program_address(seeds, &program_id);
            println!("Manager fee pattern {}: {} -> {}", i + 1, 
                match i {
                    0 => "\"manager_fee\"",
                    1 => "\"fee\"",
                    2 => "\"manager\"", 
                    3 => "pool + \"fee\"",
                    4 => "\"manager_fee_account\"",
                    _ => "unknown",
                }, derived);
            
            if derived.to_string() == target_manager_fee {
                println!("  Found matching manager fee pattern {}", i + 1);
                found_manager_fee = Some(derived);
                break;
            }
        }

        // If no pattern matches, use the expected address directly
        found_manager_fee.unwrap_or_else(|| {
            println!("No manager fee pattern matched, using expected address directly");
            Pubkey::from_str(target_manager_fee).unwrap()
        })
    };

    println!("Building instant unstake instruction for stake account: {}", stake_account);

    // CRITICAL: Use the EXACT discriminator from working CLI
    let disc = anchor_discriminator("liquid_unstake_stake_account");
    let mut data = Vec::with_capacity(24);
    data.extend_from_slice(&disc);
    
    // CRITICAL: Add the instruction data exactly like the CLI
    // Serialize Option<u64> for minimum_lamports_out
    data.push(1); // 1 = Some, 0 = None
    
    // Use a reasonable amount (equivalent to 0.1 VLP for testing)
    let amount_base_units = 100_000_000u64; // 0.1 VLP in base units
    data.extend_from_slice(&amount_base_units.to_le_bytes());
    
    // Account structure from successful transaction (EXACT from CLI)
    let accounts = vec![
        AccountMeta::new(pool_pda, false),         // #1 - Pool PDA
        AccountMeta::new(*user, true),             // #2 - User (signer)
        AccountMeta::new(*stake_account, false),   // #3 - Stake account
        AccountMeta::new(stake_account_info, false), // #4 - Stake account info
        AccountMeta::new(sol_vault, false),        // #5 - SOL vault
        AccountMeta::new(*user, true),             // #6 - User sol account
        AccountMeta::new(manager_fee_account, false), // #7 - Manager fee account (FIXED)
        AccountMeta::new_readonly(Pubkey::from_str("Stake11111111111111111111111111111111111111").unwrap(), false), // #8 - Stake program
        AccountMeta::new_readonly(spl_token::id(), false), // #9 - Token program
        AccountMeta::new_readonly(system_program_id, false), // #10 - System program
        AccountMeta::new_readonly(Pubkey::from_str("SysvarC1ock11111111111111111111111111111111").unwrap(), false), // #11 - Clock
    ];

    println!("Account structure (with corrected manager fee account):");
    for (i, account) in accounts.iter().enumerate() {
        let name = match i {
            0 => "pool",
            1 => "user",
            2 => "stake_account", 
            3 => "stake_account_info",
            4 => "sol_vault",
            5 => "user_sol_account",
            6 => "manager_fee_account",
            7 => "stake_program",
            8 => "token_program",
            9 => "system_program",
            10 => "clock",
            _ => "unknown",
        };
        println!("  {}: {} ({})", name, account.pubkey, 
                 if account.is_writable { "writable" } else { "readonly" });
    }

    Ok(Instruction { program_id, accounts, data })
}

/// Add Jito tip instructions
fn add_jito_tips(
    from_pubkey: &Pubkey,
    instructions: &mut Vec<Instruction>,
) -> Result<(), Box<dyn Error>> {
    let jito_address1 = Pubkey::from_str("juLesoSmdTcRtzjCzYzRoHrnF8GhVu6KCV7uxq7nJGp")?;
    let jito_address2 = Pubkey::from_str("DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL")?;

    // system_instruction already returns solana_sdk::Instruction, use directly
    instructions.push(system_instruction::transfer(from_pubkey, &jito_address1, 100_000));
    instructions.push(system_instruction::transfer(from_pubkey, &jito_address2, 100_000));

    println!("Added Jito tip instructions");
    Ok(())
}

/// Main instant unstake function - simple and standalone
pub async fn instant_unstake_stake_account(
    stake_account: &DetailedStakeAccount,
    wallet_info: Option<&WalletInfo>,
    hardware_wallet: Option<Arc<HardwareWallet>>,
    rpc_url: Option<&str>,
) -> Result<String, StakingError> {
    println!("INSTANT UNSTAKE: Starting for stake account: {}", stake_account.pubkey);
    
    // Validate active stake account
    if stake_account.state != StakeAccountState::Delegated {
        return Err(StakingError::InvalidAmount("Can only instant unstake active stake accounts".to_string()));
    }

    let stake_balance_sol = (stake_account.balance.saturating_sub(stake_account.rent_exempt_reserve)) as f64 / 1_000_000_000.0;
    println!("Stake account balance: {:.6} SOL", stake_balance_sol);

    // Create transaction client
    let transaction_client = TransactionClient::new(rpc_url);
    
    // Create signer
    let signer: Box<dyn TransactionSigner> = if let Some(ref hw) = hardware_wallet {
        Box::new(HardwareSigner::from_wallet(hw.clone()))
    } else if let Some(w) = wallet_info {
        let wallet = Wallet::from_wallet_info(w)
            .map_err(|e| StakingError::WalletError(format!("Failed to create wallet: {}", e)))?;
        Box::new(SoftwareSigner::new(wallet))
    } else {
        return Err(StakingError::WalletError("No wallet provided".to_string()));
    };

    // Get user pubkey
    let user_pubkey_str = signer.get_public_key().await
        .map_err(|e| StakingError::WalletError(format!("Failed to get public key: {}", e)))?;
    let user_pubkey = Pubkey::from_str(&user_pubkey_str)
        .map_err(|_| StakingError::WalletError("Invalid wallet address".to_string()))?;

    // Build instant unstake instruction
    let instant_unstake_ix = build_instant_unstake_instruction(&user_pubkey, &stake_account.pubkey)?;

    // Build transaction
    let mut instructions = Vec::new();
    
    // Convert compute budget instructions from interface to SDK type
    instructions.push(convert_compute_budget_instruction(
        ComputeBudgetInstructionInterface::set_compute_unit_limit(200_000)
    ));
    instructions.push(convert_compute_budget_instruction(
        ComputeBudgetInstructionInterface::set_compute_unit_price(20_000)
    ));
    instructions.push(instant_unstake_ix);

    // Add Jito tips if enabled AND not using hardware wallet
    let jito_settings = get_current_jito_settings();
    if jito_settings.jito_tx && hardware_wallet.is_none() {
        println!("Adding Jito tips");
        if let Err(e) = add_jito_tips(&user_pubkey, &mut instructions) {
            println!("Jito tips failed: {}, continuing", e);
        }
    } else if hardware_wallet.is_some() {
        println!("Hardware wallet detected - skipping Jito tips");
    }

    // Get recent blockhash
    let recent_blockhash = transaction_client.get_recent_blockhash().await
        .map_err(|e| StakingError::RpcError(format!("Failed to get blockhash: {}", e)))?;

    // Create transaction message
    let mut message = Message::new(&instructions, Some(&user_pubkey));
    message.recent_blockhash = recent_blockhash;
    
    let transaction = VersionedTransaction {
        signatures: vec![SolanaSignature::default(); message.header.num_required_signatures as usize],
        message: VersionedMessage::Legacy(message),
    };
    
    // Sign transaction
    let message_bytes = transaction.message.serialize();
    let signature_bytes = signer.sign_message(&message_bytes).await
        .map_err(|e| StakingError::WalletError(format!("Failed to sign: {}", e)))?;

    let signature = SolanaSignature::from(
        <[u8; 64]>::try_from(signature_bytes.as_slice())
            .map_err(|_| StakingError::WalletError("Invalid signature length".to_string()))?
    );

    let mut signed_transaction = transaction;
    signed_transaction.signatures[0] = signature;

    // Serialize transaction to string (as TransactionClient expects)
    let serialized = bincode::serialize(&signed_transaction)
        .map_err(|e| StakingError::TransactionFailed(format!("Serialization failed: {}", e)))?;
    let encoded = bs58::encode(serialized).into_string();

    println!("Sending instant unstake transaction ({} bytes)", encoded.len());

    // Send transaction
    match transaction_client.send_transaction(&encoded).await {
        Ok(sig) => {
            println!("Instant unstake successful!");
            println!("Transaction: {}", sig);
            println!("Explorer: https://explorer.solana.com/tx/{}?cluster=mainnet", sig);
            Ok(sig)
        }
        Err(e) => {
            println!("Transaction failed: {}", e);
            Err(StakingError::TransactionFailed(format!("Transaction failed: {}", e)))
        }
    }
}

/// Check if a stake account can be instantly unstaked
pub fn can_instant_unstake(stake_account: &DetailedStakeAccount) -> bool {
    stake_account.state == StakeAccountState::Delegated
}

// Build a normal deactivate stake instruction for regular unstaking
fn build_deactivate_stake_instruction(
    stake_account: &Pubkey,
    stake_authority: &Pubkey,
) -> Result<Instruction, StakingError> {
    println!("Building deactivate instruction for stake account: {}", stake_account);
    println!("Stake authority: {}", stake_authority);
    
    // Build the deactivate instruction manually
    // The stake program ID
    let stake_program_id = Pubkey::from_str("Stake11111111111111111111111111111111111111")
        .map_err(|_| StakingError::RpcError("Invalid stake program ID".to_string()))?;
    
    // Clock sysvar
    let clock_sysvar = Pubkey::from_str("SysvarC1ock11111111111111111111111111111111")
        .map_err(|_| StakingError::RpcError("Invalid clock sysvar".to_string()))?;
    
    // Build account metas
    let accounts = vec![
        AccountMeta::new(*stake_account, false),      // Stake account (writable)
        AccountMeta::new_readonly(clock_sysvar, false), // Clock sysvar (readonly)
        AccountMeta::new(*stake_authority, true), // Stake authority (writable, signer)
    ];
    
    // Deactivate instruction discriminator (instruction index 5 for Deactivate as LE u32)
    let instruction_data = 5u32.to_le_bytes().to_vec(); // [5, 0, 0, 0]
    
    let instruction = Instruction {
        program_id: stake_program_id,
        accounts,
        data: instruction_data,
    };
    
    Ok(instruction)
}

/// Main normal unstake function - deactivates a stake account for regular unstaking
pub async fn normal_unstake_stake_account(
    stake_account: &DetailedStakeAccount,
    wallet_info: Option<&WalletInfo>,
    hardware_wallet: Option<Arc<HardwareWallet>>,
    rpc_url: Option<&str>,
) -> Result<String, StakingError> {
    println!("NORMAL UNSTAKE: Starting for stake account: {}", stake_account.pubkey);
    
    // Validate that this is an active stake account that can be deactivated
    if stake_account.state != StakeAccountState::Delegated {
        return Err(StakingError::InvalidAmount("Can only deactivate delegated stake accounts".to_string()));
    }

    let stake_balance_sol = (stake_account.balance.saturating_sub(stake_account.rent_exempt_reserve)) as f64 / 1_000_000_000.0;
    println!("Stake account balance: {:.6} SOL", stake_balance_sol);
    
    // Create transaction client
    let transaction_client = TransactionClient::new(rpc_url);
    
    // Create signer
    let signer: Box<dyn TransactionSigner> = if let Some(ref hw) = hardware_wallet {
        Box::new(HardwareSigner::from_wallet(hw.clone()))
    } else if let Some(w) = wallet_info {
        let wallet = Wallet::from_wallet_info(w)
            .map_err(|e| StakingError::WalletError(format!("Failed to create wallet: {}", e)))?;
        Box::new(SoftwareSigner::new(wallet))
    } else {
        return Err(StakingError::WalletError("No wallet provided".to_string()));
    };

    // Get user pubkey (this will be the stake authority)
    let user_pubkey_str = signer.get_public_key().await
        .map_err(|e| StakingError::WalletError(format!("Failed to get public key: {}", e)))?;
    let user_pubkey = Pubkey::from_str(&user_pubkey_str)
        .map_err(|_| StakingError::WalletError("Invalid wallet address".to_string()))?;

    // Build deactivate instruction 
    let deactivate_ix = build_deactivate_stake_instruction(&stake_account.pubkey, &user_pubkey)?;

    // Build transaction with compute budget and deactivate instruction
    let mut instructions = Vec::new();
    
    // Add compute budget instructions (matching the transaction you provided)
    instructions.push(convert_compute_budget_instruction(
        ComputeBudgetInstructionInterface::set_compute_unit_price(375_000)
    ));
    instructions.push(convert_compute_budget_instruction(
        ComputeBudgetInstructionInterface::set_compute_unit_limit(200_000)
    ));
    
    // Add the main deactivate instruction
    instructions.push(deactivate_ix);

    // Add Jito tips if enabled AND not using hardware wallet
    let jito_settings = get_current_jito_settings();
    if jito_settings.jito_tx && hardware_wallet.is_none() {
        println!("Adding Jito tips");
        if let Err(e) = add_jito_tips(&user_pubkey, &mut instructions) {
            println!("Jito tips failed: {}, continuing", e);
        }
    } else if hardware_wallet.is_some() {
        println!("Hardware wallet detected - skipping Jito tips");
    }

    // Get recent blockhash
    let recent_blockhash = transaction_client.get_recent_blockhash().await
        .map_err(|e| StakingError::RpcError(format!("Failed to get blockhash: {}", e)))?;

    // Create transaction message
    let mut message = Message::new(&instructions, Some(&user_pubkey));
    message.recent_blockhash = recent_blockhash;
    
    let transaction = VersionedTransaction {
        signatures: vec![SolanaSignature::default(); message.header.num_required_signatures as usize],
        message: VersionedMessage::Legacy(message),
    };
    
    // Sign transaction
    let message_bytes = transaction.message.serialize();
    let signature_bytes = signer.sign_message(&message_bytes).await
        .map_err(|e| StakingError::WalletError(format!("Failed to sign: {}", e)))?;

    let signature = SolanaSignature::from(
        <[u8; 64]>::try_from(signature_bytes.as_slice())
            .map_err(|_| StakingError::WalletError("Invalid signature length".to_string()))?
    );

    let mut signed_transaction = transaction;
    signed_transaction.signatures[0] = signature;

    // Serialize transaction to string (as TransactionClient expects)
    let serialized = bincode::serialize(&signed_transaction)
        .map_err(|e| StakingError::TransactionFailed(format!("Serialization failed: {}", e)))?;
    let encoded = bs58::encode(serialized).into_string();

    println!("Sending normal unstake (deactivate) transaction ({} bytes)", encoded.len());

    // Send transaction
    match transaction_client.send_transaction(&encoded).await {
        Ok(sig) => {
            println!("Normal unstake successful!");
            println!("Transaction: {}", sig);
            println!("Explorer: https://explorer.solana.com/tx/{}?cluster=mainnet", sig);
            Ok(sig)
        }
        Err(e) => {
            println!("Transaction failed: {}", e);
            Err(StakingError::TransactionFailed(format!("Transaction failed: {}", e)))
        }
    }
}

/// Check if a stake account can be normally unstaked (deactivated)
pub fn can_normal_unstake(stake_account: &DetailedStakeAccount) -> bool {
    // Can only deactivate delegated (active) stake accounts
    stake_account.state == StakeAccountState::Delegated
}

/// Build a split stake instruction
fn build_split_instruction(
    stake_account: &Pubkey,
    new_stake_account: &Pubkey,
    stake_authority: &Pubkey,
    lamports: u64,
) -> Result<Instruction, StakingError> {
    println!("Building split instruction:");
    println!("  Source stake: {}", stake_account);
    println!("  New stake: {}", new_stake_account);
    println!("  Amount: {} lamports ({:.6} SOL)", lamports, lamports as f64 / 1_000_000_000.0);
    
    let stake_program_id = Pubkey::from_str("Stake11111111111111111111111111111111111111")
        .map_err(|_| StakingError::RpcError("Invalid stake program ID".to_string()))?;
    
    // Split instruction accounts (from Solana SDK)
    let accounts = vec![
        AccountMeta::new(*stake_account, false),       // Source stake account (writable)
        AccountMeta::new(*new_stake_account, false),   // New stake account (writable)
        AccountMeta::new_readonly(*stake_authority, true), // Stake authority (signer)
    ];
    
    // Split instruction discriminator (instruction index 3 for Split as LE u32)
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&3u32.to_le_bytes()); // Split = 3
    instruction_data.extend_from_slice(&lamports.to_le_bytes()); // Amount to split
    
    let instruction = Instruction {
        program_id: stake_program_id,
        accounts,
        data: instruction_data,
    };
    
    Ok(instruction)
}

/// Partial unstake - split stake account and unstake only a portion
/// 
/// This function:
/// 1. Creates a new stake account
/// 2. Splits the specified amount from the original stake to the new account
/// 3. Deactivates the new stake account
/// 
/// The original stake remains active with the remaining balance
pub async fn partial_unstake_stake_account(
    stake_account: &DetailedStakeAccount,
    amount_to_unstake_sol: f64,
    wallet_info: Option<&WalletInfo>,
    hardware_wallet: Option<Arc<HardwareWallet>>,
    rpc_url: Option<&str>,
) -> Result<String, StakingError> {
    println!("PARTIAL UNSTAKE: Starting for stake account: {}", stake_account.pubkey);
    println!("  Amount to unstake: {:.6} SOL", amount_to_unstake_sol);
    
    // Validate that this is an active stake account
    if stake_account.state != StakeAccountState::Delegated {
        return Err(StakingError::InvalidAmount("Can only partially unstake delegated stake accounts".to_string()));
    }

    // Convert amount to lamports
    let amount_to_unstake_lamports = (amount_to_unstake_sol * 1_000_000_000.0) as u64;
    
    // Get current staked amount (excluding rent reserve)
    let current_staked = stake_account.balance.saturating_sub(stake_account.rent_exempt_reserve);
    
    // Validate amount
    if amount_to_unstake_lamports == 0 {
        return Err(StakingError::InvalidAmount("Amount must be greater than 0".to_string()));
    }
    
    if amount_to_unstake_lamports > current_staked {
        return Err(StakingError::InvalidAmount(
            format!("Cannot unstake {} SOL - only {} SOL available (excluding rent)", 
                amount_to_unstake_sol,
                current_staked as f64 / 1_000_000_000.0
            )
        ));
    }
    
    // Require minimum balance remaining (0.01 SOL)
    let remaining_balance = current_staked.saturating_sub(amount_to_unstake_lamports);
    if remaining_balance > 0 && remaining_balance < 10_000_000 {
        return Err(StakingError::InvalidAmount(
            "Remaining stake must be at least 0.01 SOL or 0 (full unstake)".to_string()
        ));
    }
    
    println!("  Current staked: {:.6} SOL", current_staked as f64 / 1_000_000_000.0);
    println!("  Will remain: {:.6} SOL", remaining_balance as f64 / 1_000_000_000.0);
    
    // Create transaction client
    let transaction_client = TransactionClient::new(rpc_url);
    
    // Create signer
    let signer: Box<dyn TransactionSigner> = if let Some(ref hw) = hardware_wallet {
        Box::new(HardwareSigner::from_wallet(hw.clone()))
    } else if let Some(w) = wallet_info {
        let wallet = Wallet::from_wallet_info(w)
            .map_err(|e| StakingError::WalletError(format!("Failed to create wallet: {}", e)))?;
        Box::new(SoftwareSigner::new(wallet))
    } else {
        return Err(StakingError::WalletError("No wallet provided".to_string()));
    };

    // Get user pubkey (stake authority)
    let user_pubkey_str = signer.get_public_key().await
        .map_err(|e| StakingError::WalletError(format!("Failed to get public key: {}", e)))?;
    let user_pubkey = Pubkey::from_str(&user_pubkey_str)
        .map_err(|_| StakingError::WalletError("Invalid wallet address".to_string()))?;

    // Generate new stake account keypair for the split portion
    use solana_sdk::signature::{Keypair, Signer as SdkSigner};
    let new_stake_keypair = Keypair::new();
    let new_stake_pubkey = new_stake_keypair.pubkey();
    
    println!("  New stake account: {}", new_stake_pubkey);
    
    // Get rent exemption for stake account (200 bytes)
    let rent_exemption = crate::rpc::get_minimum_balance_for_rent_exemption(200, rpc_url)
        .await
        .map_err(|e| StakingError::RpcError(format!("Failed to get rent exemption: {}", e)))?;
    
    // Build instructions
    let mut instructions = Vec::new();
    
    // Add compute budget
    instructions.push(convert_compute_budget_instruction(
        ComputeBudgetInstructionInterface::set_compute_unit_limit(300_000)
    ));
    instructions.push(convert_compute_budget_instruction(
        ComputeBudgetInstructionInterface::set_compute_unit_price(50_000)
    ));
    
    // 1. Create the new stake account (system_instruction already returns solana_sdk::Instruction)
    instructions.push(system_instruction::create_account(
        &user_pubkey,
        &new_stake_pubkey,
        rent_exemption,
        200, // stake account size
        &Pubkey::from_str("Stake11111111111111111111111111111111111111").unwrap(),
    ));
    
    // 2. Split stake from original to new account
    let split_ix = build_split_instruction(
        &stake_account.pubkey,
        &new_stake_pubkey,
        &user_pubkey,
        amount_to_unstake_lamports,
    )?;
    instructions.push(split_ix);
    
    // 3. Deactivate the new stake account
    let deactivate_ix = build_deactivate_stake_instruction(&new_stake_pubkey, &user_pubkey)?;
    instructions.push(deactivate_ix);

    // Add Jito tips if enabled AND not using hardware wallet
    let jito_settings = get_current_jito_settings();
    if jito_settings.jito_tx && hardware_wallet.is_none() {
        println!("Adding Jito tips");
        if let Err(e) = add_jito_tips(&user_pubkey, &mut instructions) {
            println!("Jito tips failed: {}, continuing", e);
        }
    } else if hardware_wallet.is_some() {
        println!("Hardware wallet detected - skipping Jito tips");
    }

    // Get recent blockhash
    let recent_blockhash = transaction_client.get_recent_blockhash().await
        .map_err(|e| StakingError::RpcError(format!("Failed to get blockhash: {}", e)))?;

    // Create transaction message
    let mut message = Message::new(&instructions, Some(&user_pubkey));
    message.recent_blockhash = recent_blockhash;
    
    let mut transaction = VersionedTransaction {
        signatures: vec![SolanaSignature::default(); message.header.num_required_signatures as usize],
        message: VersionedMessage::Legacy(message),
    };
    
    // Sign with wallet
    let message_bytes = transaction.message.serialize();
    let signature_bytes = signer.sign_message(&message_bytes).await
        .map_err(|e| StakingError::WalletError(format!("Failed to sign: {}", e)))?;

    let signature = SolanaSignature::from(
        <[u8; 64]>::try_from(signature_bytes.as_slice())
            .map_err(|_| StakingError::WalletError("Invalid signature length".to_string()))?
    );

    transaction.signatures[0] = signature;
    
    // Also sign with the new stake account keypair
    let legacy_message = match &transaction.message {
        VersionedMessage::Legacy(msg) => msg.clone(),
        _ => return Err(StakingError::TransactionFailed("Expected legacy message".to_string())),
    };
    
    let mut legacy_transaction = solana_sdk::transaction::Transaction {
        signatures: vec![SolanaSignature::default(); legacy_message.header.num_required_signatures as usize],
        message: legacy_message,
    };
    
    // Sign with the new stake keypair
    legacy_transaction.partial_sign(&[&new_stake_keypair], recent_blockhash);
    
    // Add wallet signature
    legacy_transaction.signatures[0] = signature;

    // Serialize and send
    let serialized = bincode::serialize(&legacy_transaction)
        .map_err(|e| StakingError::TransactionFailed(format!("Serialization failed: {}", e)))?;
    let encoded = bs58::encode(serialized).into_string();

    println!("Sending partial unstake transaction ({} bytes)", encoded.len());

    // Send transaction
    match transaction_client.send_transaction(&encoded).await {
        Ok(sig) => {
            println!("Partial unstake successful!");
            println!("Transaction: {}", sig);
            println!("Explorer: https://explorer.solana.com/tx/{}?cluster=mainnet", sig);
            Ok(sig)
        }
        Err(e) => {
            println!("Transaction failed: {}", e);
            Err(StakingError::TransactionFailed(format!("Transaction failed: {}", e)))
        }
    }
}

/// Check if a stake account can be partially unstaked
pub fn can_partial_unstake(stake_account: &DetailedStakeAccount) -> bool {
    // Can only partially unstake delegated (active) stake accounts
    // And must have more than minimum (0.01 SOL) to make splitting worthwhile
    let available = stake_account.balance.saturating_sub(stake_account.rent_exempt_reserve);
    stake_account.state == StakeAccountState::Delegated && available > 20_000_000 // > 0.02 SOL
}

/// Build a withdraw stake instruction
fn build_withdraw_instruction(
    stake_account: &Pubkey,
    destination: &Pubkey,
    withdraw_authority: &Pubkey,
    lamports: u64,
) -> Result<Instruction, StakingError> {
    println!("Building withdraw instruction:");
    println!("  Stake account: {}", stake_account);
    println!("  Destination: {}", destination);
    println!("  Amount: {} lamports ({:.6} SOL)", lamports, lamports as f64 / 1_000_000_000.0);
    
    let stake_program_id = Pubkey::from_str("Stake11111111111111111111111111111111111111")
        .map_err(|_| StakingError::RpcError("Invalid stake program ID".to_string()))?;
    
    // Clock sysvar
    let clock_sysvar = Pubkey::from_str("SysvarC1ock11111111111111111111111111111111")
        .map_err(|_| StakingError::RpcError("Invalid clock sysvar".to_string()))?;
    
    // Stake history sysvar
    let stake_history_sysvar = Pubkey::from_str("SysvarStakeHistory1111111111111111111111111")
        .map_err(|_| StakingError::RpcError("Invalid stake history sysvar".to_string()))?;
    
    // Build account metas for withdraw instruction
    let accounts = vec![
        AccountMeta::new(*stake_account, false),           // Stake account (writable)
        AccountMeta::new(*destination, false),             // Destination account (writable)
        AccountMeta::new_readonly(clock_sysvar, false),    // Clock sysvar (readonly)
        AccountMeta::new_readonly(stake_history_sysvar, false), // Stake history sysvar (readonly)
        AccountMeta::new_readonly(*withdraw_authority, true),   // Withdraw authority (signer)
    ];
    
    // Withdraw instruction discriminator (instruction index 4 for Withdraw as LE u32)
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&4u32.to_le_bytes()); // Withdraw = 4
    instruction_data.extend_from_slice(&lamports.to_le_bytes()); // Amount to withdraw
    
    let instruction = Instruction {
        program_id: stake_program_id,
        accounts,
        data: instruction_data,
    };
    
    Ok(instruction)
}

/// Withdraw SOL from an inactive stake account back to the user's wallet
/// 
/// This function:
/// 1. Withdraws all SOL (including rent reserve) from an inactive stake account
/// 2. Transfers the SOL to the user's wallet
/// 3. Destroys the stake account (balance becomes 0)
pub async fn withdraw_stake_account(
    stake_account: &DetailedStakeAccount,
    wallet_info: Option<&WalletInfo>,
    hardware_wallet: Option<Arc<HardwareWallet>>,
    rpc_url: Option<&str>,
) -> Result<String, StakingError> {
    println!("WITHDRAW: Starting for stake account: {}", stake_account.pubkey);
    
    // Validate that this is an inactive stake account
    if stake_account.state != StakeAccountState::Uninitialized {
        return Err(StakingError::InvalidAmount(
            "Can only withdraw from inactive stake accounts. Account must be fully deactivated first.".to_string()
        ));
    }

    // Calculate total withdrawable amount (full balance including rent reserve)
    let withdraw_amount = stake_account.balance;
    let withdraw_amount_sol = withdraw_amount as f64 / 1_000_000_000.0;
    
    println!("Total withdrawable: {:.6} SOL (includes rent reserve)", withdraw_amount_sol);
    
    if withdraw_amount == 0 {
        return Err(StakingError::InvalidAmount("Stake account has zero balance".to_string()));
    }
    
    // Create transaction client
    let transaction_client = TransactionClient::new(rpc_url);
    
    // Create signer
    let signer: Box<dyn TransactionSigner> = if let Some(ref hw) = hardware_wallet {
        Box::new(HardwareSigner::from_wallet(hw.clone()))
    } else if let Some(w) = wallet_info {
        let wallet = Wallet::from_wallet_info(w)
            .map_err(|e| StakingError::WalletError(format!("Failed to create wallet: {}", e)))?;
        Box::new(SoftwareSigner::new(wallet))
    } else {
        return Err(StakingError::WalletError("No wallet provided".to_string()));
    };

    // Get user pubkey (this will be both the destination and withdraw authority)
    let user_pubkey_str = signer.get_public_key().await
        .map_err(|e| StakingError::WalletError(format!("Failed to get public key: {}", e)))?;
    let user_pubkey = Pubkey::from_str(&user_pubkey_str)
        .map_err(|_| StakingError::WalletError("Invalid wallet address".to_string()))?;

    // Build withdraw instruction
    let withdraw_ix = build_withdraw_instruction(
        &stake_account.pubkey,
        &user_pubkey,  // Destination = user's wallet
        &user_pubkey,  // Withdraw authority = user
        withdraw_amount, // Withdraw full balance
    )?;

    // Build transaction with compute budget and withdraw instruction
    let mut instructions = Vec::new();
    
    // Add compute budget instructions
    instructions.push(convert_compute_budget_instruction(
        ComputeBudgetInstructionInterface::set_compute_unit_price(50_000)
    ));
    instructions.push(convert_compute_budget_instruction(
        ComputeBudgetInstructionInterface::set_compute_unit_limit(200_000)
    ));
    
    // Add the main withdraw instruction
    instructions.push(withdraw_ix);

    // Add Jito tips if enabled AND not using hardware wallet
    let jito_settings = get_current_jito_settings();
    if jito_settings.jito_tx && hardware_wallet.is_none() {
        println!("Adding Jito tips");
        if let Err(e) = add_jito_tips(&user_pubkey, &mut instructions) {
            println!("Jito tips failed: {}, continuing", e);
        }
    } else if hardware_wallet.is_some() {
        println!("Hardware wallet detected - skipping Jito tips");
    }

    // Get recent blockhash
    let recent_blockhash = transaction_client.get_recent_blockhash().await
        .map_err(|e| StakingError::RpcError(format!("Failed to get blockhash: {}", e)))?;

    // Create transaction message
    let mut message = Message::new(&instructions, Some(&user_pubkey));
    message.recent_blockhash = recent_blockhash;
    
    let transaction = VersionedTransaction {
        signatures: vec![SolanaSignature::default(); message.header.num_required_signatures as usize],
        message: VersionedMessage::Legacy(message),
    };
    
    // Sign transaction
    let message_bytes = transaction.message.serialize();
    let signature_bytes = signer.sign_message(&message_bytes).await
        .map_err(|e| StakingError::WalletError(format!("Failed to sign: {}", e)))?;

    let signature = SolanaSignature::from(
        <[u8; 64]>::try_from(signature_bytes.as_slice())
            .map_err(|_| StakingError::WalletError("Invalid signature length".to_string()))?
    );

    let mut signed_transaction = transaction;
    signed_transaction.signatures[0] = signature;

    // Serialize transaction to string
    let serialized = bincode::serialize(&signed_transaction)
        .map_err(|e| StakingError::TransactionFailed(format!("Serialization failed: {}", e)))?;
    let encoded = bs58::encode(serialized).into_string();

    println!("Sending withdraw transaction ({} bytes)", encoded.len());

    // Send transaction
    match transaction_client.send_transaction(&encoded).await {
        Ok(sig) => {
            println!("Withdraw successful!");
            println!("Transaction: {}", sig);
            println!("Explorer: https://explorer.solana.com/tx/{}?cluster=mainnet", sig);
            Ok(sig)
        }
        Err(e) => {
            println!("Transaction failed: {}", e);
            Err(StakingError::TransactionFailed(format!("Transaction failed: {}", e)))
        }
    }
}

/// Check if a stake account can be withdrawn
pub fn can_withdraw(stake_account: &DetailedStakeAccount) -> bool {
    // Can only withdraw from inactive (uninitialized) stake accounts with a balance
    stake_account.state == StakeAccountState::Uninitialized && stake_account.balance > 0
}