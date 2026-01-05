// src/bonk_staking/client.rs
//! BONK staking client implementation

use borsh::BorshDeserialize;
use chrono::{NaiveDateTime, Utc};
use solana_sdk::{
    pubkey::Pubkey,
    signature::Signature as SolanaSignature,
    transaction::VersionedTransaction,
    message::VersionedMessage,
};
use solana_system_interface::instruction as system_instruction;
use std::error::Error as StdError;
use std::str::FromStr;
use serde_json::{json, Value};
use reqwest::Client as HttpClient;

use crate::signing::TransactionSigner;
use crate::bonk_staking::types::StakeResult;
use crate::storage::get_current_jito_settings;

type Result<T> = std::result::Result<T, Box<dyn StdError>>;

const BONK_DECIMALS: f64 = 100_000.0;

#[derive(BorshDeserialize, Debug, Clone)]
struct StakeDepositReceipt {
    pub payer: Pubkey,
    pub stake_pool: Pubkey,
    pub lock_up_duration: u64,
    pub deposit_timestamp: i64,
    pub stake_mint_claimed: u64,
    pub vault_claimed: u64,
    pub effective_stake: u128,
    pub effective_stake_pda_bump: u8,
}

fn deserialize_receipt(data: &[u8]) -> Option<StakeDepositReceipt> {
    let mut cursor = data;
    StakeDepositReceipt::deserialize(&mut cursor).ok()
}

fn read_u64(data: &[u8], offset: usize) -> Option<u64> {
    if offset + 8 > data.len() {
        return None;
    }
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&data[offset..offset + 8]);
    Some(u64::from_le_bytes(buf))
}

fn read_i64(data: &[u8], offset: usize) -> Option<i64> {
    if offset + 8 > data.len() {
        return None;
    }
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&data[offset..offset + 8]);
    Some(i64::from_le_bytes(buf))
}

fn read_u128(data: &[u8], offset: usize) -> Option<u128> {
    if offset + 16 > data.len() {
        return None;
    }
    let mut buf = [0u8; 16];
    buf.copy_from_slice(&data[offset..offset + 16]);
    Some(u128::from_le_bytes(buf))
}

fn find_pubkey_offset(data: &[u8], target: &Pubkey) -> Option<usize> {
    let needle = target.to_bytes();
    data.windows(needle.len())
        .position(|window| window == needle.as_slice())
}

fn decode_receipt_with_offsets(
    data: &[u8],
    owner: &Pubkey,
    stake_pool: &Pubkey,
) -> Option<StakeDepositReceipt> {
    let owner_offset = find_pubkey_offset(data, owner)?;
    let stake_pool_offset = find_pubkey_offset(data, stake_pool)?;

    let base = if owner_offset + 32 == stake_pool_offset {
        owner_offset + 64
    } else if stake_pool_offset + 32 == owner_offset {
        stake_pool_offset + 64
    } else {
        return None;
    };

    Some(StakeDepositReceipt {
        payer: *owner,
        stake_pool: *stake_pool,
        lock_up_duration: read_u64(data, base)?,
        deposit_timestamp: read_i64(data, base + 8)?,
        stake_mint_claimed: read_u64(data, base + 16)?,
        vault_claimed: read_u64(data, base + 24)?,
        effective_stake: read_u128(data, base + 32)?,
        effective_stake_pda_bump: data.get(base + 48).copied().unwrap_or(0),
    })
}

/// Wrapper around bonk-staking-rewards for app integration
pub struct BonkStakingClient {
    rpc_url: String,
    http_client: HttpClient,
}

impl BonkStakingClient {
    pub fn new(rpc_url: Option<&str>) -> Self {
        let url = rpc_url
            .unwrap_or("https://johna-k3cr1v-fast-mainnet.helius-rpc.com")
            .to_string();
        
        Self {
            rpc_url: url,
            http_client: HttpClient::new(),
        }
    }

    /// Get BONK balance for a wallet (returns as f64 for UI display)
    pub async fn get_bonk_balance(&self, wallet_pubkey: &Pubkey) -> Result<f64> {
        let bonk_mint = bonk_staking_rewards_v3::BONK_MINT;
        let ata = spl_associated_token_account::get_associated_token_address(
            wallet_pubkey,
            &bonk_mint,
        );

        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getTokenAccountBalance",
            "params": [ata.to_string()]
        });

        let response = self.http_client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await?;

        let json: Value = response.json().await?;

        if let Some(amount_str) = json["result"]["value"]["amount"].as_str() {
            let lamports = amount_str.parse::<u64>()?;
            Ok(lamports as f64 / BONK_DECIMALS) // Convert to BONK (5 decimals)
        } else {
            Ok(0.0)
        }
    }

    /// Stake BONK tokens using the app's signing infrastructure
    pub async fn stake_with_signer(
        &self,
        signer: &dyn TransactionSigner,
        amount: u64,
        duration_days: u64,
        nonce: Option<u32>,
        is_hardware_wallet: bool,
    ) -> Result<StakeResult> {
        let user_pubkey_str = signer.get_public_key().await?;
        let user_pubkey = Pubkey::from_str(&user_pubkey_str)?;

        // Validate and convert duration
        let lock_duration_seconds = match duration_days {
            30 => 30 * 24 * 60 * 60,
            90 => 90 * 24 * 60 * 60,
            180 => 180 * 24 * 60 * 60,
            365 => 365 * 24 * 60 * 60,
            _ => return Err("Duration must be 30, 90, 180, or 365 days".into()),
        };

        // Get or auto-select nonce
        let stake_nonce = match nonce {
            Some(n) => n,
            None => self.find_available_nonce(&user_pubkey).await?,
        };

        // Check BONK balance
        let bonk_balance = self.get_bonk_balance(&user_pubkey).await?;
        let amount_bonk = amount as f64 / BONK_DECIMALS; // Convert lamports to BONK (5 decimals)
        if bonk_balance < amount_bonk {
            return Err(format!("Insufficient BONK balance: have {:.2}, need {:.2}", bonk_balance, amount_bonk).into());
        }

        // Build instructions
        let mut instructions = Vec::new();

        // Add compute budget
        let compute_budget_ix = bonk_staking_rewards_v3::instructions::build_compute_budget_price_instruction(5045);
        instructions.push(compute_budget_ix);

        // Create stake token ATA if needed
        let create_stake_ata_ix = spl_associated_token_account::instruction::create_associated_token_account_idempotent(
            &user_pubkey,
            &user_pubkey,
            &bonk_staking_rewards_v3::BONK_STAKE_MINT,
            &spl_token::id(),
        );
        instructions.push(create_stake_ata_ix);

        // Build stake instruction
        let stake_ix = bonk_staking_rewards_v3::instructions::build_stake_instruction(
            &user_pubkey,
            amount,
            lock_duration_seconds,
            stake_nonce,
        );
        instructions.push(stake_ix);

        // Check Jito settings and add tip if enabled AND not using hardware wallet
        let jito_settings = get_current_jito_settings();
        if jito_settings.jito_tx && !is_hardware_wallet {
            let jito_tip_address = Pubkey::from_str("juLesoSmdTcRtzjCzYzRoHrnF8GhVu6KCV7uxq7nJGp")?;
            let tip_ix = system_instruction::transfer(&user_pubkey, &jito_tip_address, 100_000);
            instructions.push(tip_ix);
        }

        // Get recent blockhash
        let recent_blockhash = self.get_recent_blockhash().await?;

        // Create transaction message
        let message = solana_sdk::message::Message::new(&instructions, Some(&user_pubkey));
        let mut message_with_blockhash = message;
        message_with_blockhash.recent_blockhash = recent_blockhash;

        let mut transaction = VersionedTransaction {
            signatures: vec![SolanaSignature::default()],
            message: VersionedMessage::Legacy(message_with_blockhash),
        };

        // Sign transaction
        let message_bytes = transaction.message.serialize();
        let signature_bytes = signer.sign_message(&message_bytes).await?;

        if signature_bytes.len() != 64 {
            return Err(format!("Invalid signature length: {}", signature_bytes.len()).into());
        }

        let mut sig_array = [0u8; 64];
        sig_array.copy_from_slice(&signature_bytes);
        transaction.signatures[0] = SolanaSignature::from(sig_array);

        // Send transaction
        let serialized = bincode::serialize(&transaction)?;
        let encoded = bs58::encode(serialized).into_string();
        let signature = self.send_transaction(&encoded).await?;

        Ok(StakeResult {
            signature,
            amount,
            duration_days,
        })
    }

    /// Find the next available nonce for a user
    async fn find_available_nonce(&self, user: &Pubkey) -> Result<u32> {
        for nonce in 0..100 {
            let (receipt_pda, _) = bonk_staking_rewards_v3::pda::derive_stake_deposit_receipt(
                user,
                &bonk_staking_rewards_v3::BONK_STAKE_POOL,
                nonce,
            );

            if self.get_account(&receipt_pda).await.is_err() {
                return Ok(nonce);
            }
        }

        Err("No available nonce found (0-99 all in use)".into())
    }

    /// Get account data from RPC
    async fn get_account(&self, pubkey: &Pubkey) -> Result<Vec<u8>> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getAccountInfo",
            "params": [
                pubkey.to_string(),
                { "encoding": "base64" }
            ]
        });

        let response = self.http_client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await?;

        let json: Value = response.json().await?;

        if let Some(data) = json["result"]["value"]["data"][0].as_str() {
            let decoded = base64::decode(data)?;
            Ok(decoded)
        } else {
            Err("Account not found".into())
        }
    }

    /// Get recent blockhash from RPC
    async fn get_recent_blockhash(&self) -> Result<solana_sdk::hash::Hash> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getLatestBlockhash",
            "params": [{ "commitment": "finalized" }]
        });

        let response = self.http_client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await?;

        let json: Value = response.json().await?;

        if let Some(blockhash_str) = json["result"]["value"]["blockhash"].as_str() {
            Ok(solana_sdk::hash::Hash::from_str(blockhash_str)?)
        } else {
            Err("Failed to get blockhash".into())
        }
    }

    /// Send transaction to RPC
    async fn send_transaction(&self, signed_tx: &str) -> Result<String> {
        let request = json!({
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
        });

        let response = self.http_client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await?;

        let json: Value = response.json().await?;

        if let Some(error) = json.get("error") {
            Err(format!("Transaction error: {:?}", error).into())
        } else if let Some(result) = json["result"].as_str() {
            Ok(result.to_string())
        } else {
            Err(format!("Unknown error: {:?}", json).into())
        }
    }

    /// Get duration options with their labels and weight multipliers
    pub fn get_duration_options() -> Vec<(u64, &'static str, f64)> {
        vec![
            (30, "30 days (1 month)", 1.0),
            (90, "90 days (3 months)", 1.5),
            (180, "180 days (6 months)", 2.0),
            (365, "365 days (12 months)", 3.2),
        ]
    }

    /// Get user's active stakes (placeholder - real implementation would query blockchain)
    pub async fn get_user_stakes(&self, wallet_address: &str) -> Result<Vec<crate::bonk_staking::types::StakePosition>> {
        let owner = Pubkey::from_str(wallet_address)?;
        let mut stakes = Vec::new();
        let mut scanned = 0;
        let mut found_accounts = 0;
        let mut decoded = 0;

        for nonce in 0..100 {
            let (receipt_pda, _) = bonk_staking_rewards_v3::pda::derive_stake_deposit_receipt(
                &owner,
                &bonk_staking_rewards_v3::BONK_STAKE_POOL,
                nonce,
            );
            scanned += 1;

            let data = match self.get_account(&receipt_pda).await {
                Ok(data) => data,
                Err(_) => continue,
            };
            found_accounts += 1;
            println!(
                "[BONK] Found receipt account: {} (nonce {}, {} bytes)",
                receipt_pda,
                nonce,
                data.len()
            );

            if data.len() <= 8 {
                println!("[BONK] Receipt data too short for decoding: {}", receipt_pda);
                continue;
            }

            let owner_offset = find_pubkey_offset(&data, &owner);
            let pool_offset = find_pubkey_offset(&data, &bonk_staking_rewards_v3::BONK_STAKE_POOL);
            println!(
                "[BONK] Receipt offsets: pda={} owner_offset={:?} stake_pool_offset={:?}",
                receipt_pda, owner_offset, pool_offset
            );

            let receipt = match decode_receipt_with_offsets(&data, &owner, &bonk_staking_rewards_v3::BONK_STAKE_POOL) {
                Some(receipt) => receipt,
                None => match deserialize_receipt(&data[8..]) {
                    Some(receipt) => receipt,
                    None => {
                        println!(
                            "[BONK] Failed to decode receipt for {}",
                            receipt_pda
                        );
                        continue;
                    }
                },
            };
            decoded += 1;

            let amount = receipt.vault_claimed as f64 / BONK_DECIMALS;
            let duration_seconds = if receipt.lock_up_duration < 10_000 {
                receipt.lock_up_duration * 86_400
            } else {
                receipt.lock_up_duration
            };
            let duration_days = duration_seconds / 86_400;
            let deposit_ts = if receipt.deposit_timestamp > 1_000_000_000_000 {
                receipt.deposit_timestamp / 1000
            } else {
                receipt.deposit_timestamp
            };
            let unlock_ts = deposit_ts + duration_seconds as i64;
            let unlock_time = NaiveDateTime::from_timestamp_opt(unlock_ts, 0)
                .map(|dt| dt.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let is_unlocked = Utc::now().timestamp() >= unlock_ts;
            let multiplier = match duration_seconds {
                d if d <= 2_592_000 => 1.0,
                d if d <= 7_776_000 => 1.5,
                d if d <= 15_552_000 => 2.0,
                _ => 3.2,
            };

            println!(
                "[BONK] Receipt decoded: pda={} amount={} lock_up_duration={} duration_seconds={} deposit_ts={} unlock_ts={} unlocked={}",
                receipt_pda,
                amount,
                receipt.lock_up_duration,
                duration_seconds,
                deposit_ts,
                unlock_ts,
                is_unlocked
            );

            stakes.push(crate::bonk_staking::types::StakePosition {
                receipt_address: receipt_pda.to_string(),
                amount,
                duration_days,
                unlock_time,
                multiplier,
                is_unlocked,
            });
        }

        println!(
            "[BONK] Stake scan summary: scanned={}, found_accounts={}, decoded={}, positions={}",
            scanned,
            found_accounts,
            decoded,
            stakes.len()
        );

        Ok(stakes)
    }

    /// Alias for stake_with_signer to match modal's expected method name
    pub async fn stake_bonk_with_signer(
        &self,
        signer: &dyn TransactionSigner,
        amount: u64,
        duration_days: u64,
        is_hardware_wallet: bool,
    ) -> Result<StakeResult> {
        self.stake_with_signer(signer, amount, duration_days, None, is_hardware_wallet).await
    }
}
