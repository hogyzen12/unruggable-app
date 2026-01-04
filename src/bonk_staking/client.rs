// src/bonk_staking/client.rs
//! BONK staking client implementation

use solana_sdk::{
    pubkey::Pubkey,
    signature::Signature as SolanaSignature,
    transaction::VersionedTransaction,
    message::VersionedMessage,
    system_instruction,
};
use std::error::Error as StdError;
use std::str::FromStr;
use serde_json::{json, Value};
use reqwest::Client as HttpClient;

use crate::signing::TransactionSigner;
use crate::bonk_staking::types::StakeResult;
use crate::storage::get_current_jito_settings;

type Result<T> = std::result::Result<T, Box<dyn StdError>>;

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
        let bonk_mint = bonk_staking_rewards::BONK_MINT;
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
            Ok(lamports as f64 / 1_000_000_000.0) // Convert to BONK
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
        let amount_bonk = amount as f64 / 1_000_000_000.0; // Convert lamports to BONK
        if bonk_balance < amount_bonk {
            return Err(format!("Insufficient BONK balance: have {:.2}, need {:.2}", bonk_balance, amount_bonk).into());
        }

        // Build instructions
        let mut instructions = Vec::new();

        // Add compute budget
        let compute_budget_ix = bonk_staking_rewards::instructions::build_compute_budget_price_instruction(5045);
        instructions.push(compute_budget_ix);

        // Create stake token ATA if needed
        let create_stake_ata_ix = spl_associated_token_account::instruction::create_associated_token_account_idempotent(
            &user_pubkey,
            &user_pubkey,
            &bonk_staking_rewards::BONK_STAKE_MINT,
            &spl_token::id(),
        );
        instructions.push(create_stake_ata_ix);

        // Build stake instruction
        let stake_ix = bonk_staking_rewards::instructions::build_stake_instruction(
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
            let (receipt_pda, _) = bonk_staking_rewards::pda::derive_stake_deposit_receipt(
                user,
                &bonk_staking_rewards::BONK_STAKE_POOL,
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
    pub async fn get_user_stakes(&self, _wallet_address: &str) -> Result<Vec<crate::bonk_staking::types::StakePosition>> {
        // TODO: Implement actual stake fetching from blockchain
        // For now, return empty vec - user will need to stake to see positions
        Ok(Vec::new())
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