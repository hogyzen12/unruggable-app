use solana_sdk::{
    pubkey::Pubkey, 
    signature::Signature as SolanaSignature,
    transaction::VersionedTransaction,
    message::VersionedMessage,
    instruction::Instruction,
};
use std::error::Error as StdError;
use std::str::FromStr;
use serde_json::{json, Value};
use reqwest::Client as HttpClient;

use crate::signing::TransactionSigner;
use crate::carrot::types::{CarrotBalances, DepositResult, WithdrawResult};

type Result<T> = std::result::Result<T, Box<dyn StdError>>;

// Token-2022 Program ID
const TOKEN_2022_PROGRAM_ID: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";

/// Wrapper around carrot-sdk that works with our TransactionSigner trait
pub struct CarrotClient {
    rpc_url: String,
    http_client: HttpClient,
}

impl CarrotClient {
    /// Create a new CarrotClient with optional RPC URL
    pub fn new(rpc_url: Option<&str>) -> Self {
        let url = rpc_url
            .unwrap_or("https://johna-k3cr1v-fast-mainnet.helius-rpc.com")
            .to_string();
        
        Self { 
            rpc_url: url,
            http_client: HttpClient::new(),
        }
    }

    /// Get token balances for a wallet
    pub async fn get_balances(&self, wallet_pubkey: &Pubkey) -> Result<CarrotBalances> {
        // Create SDK client for balance queries
        let sdk_client = carrot_sdk::CarrotClient::new(self.rpc_url.clone());
        
        let mut balances = CarrotBalances::default();

        // Get USDC balance
        if let Ok(usdc_balance) = sdk_client.get_asset_balance(wallet_pubkey, &carrot_sdk::USDC_MINT) {
            balances.usdc = usdc_balance as f64 / 1_000_000.0;
        }

        // Get USDT balance
        if let Ok(usdt_balance) = sdk_client.get_asset_balance(wallet_pubkey, &carrot_sdk::USDT_MINT) {
            balances.usdt = usdt_balance as f64 / 1_000_000.0;
        }

        // Get pyUSD balance
        if let Ok(pyusd_balance) = sdk_client.get_asset_balance(wallet_pubkey, &carrot_sdk::PYUSD_MINT) {
            balances.pyusd = pyusd_balance as f64 / 1_000_000.0;
        }

        // Get CRT balance
        if let Ok(crt_balance) = sdk_client.get_crt_balance(wallet_pubkey) {
            balances.crt = crt_balance as f64 / 1_000_000_000.0;
        }

        Ok(balances)
    }

    /// Deposit assets and receive CRT tokens
    pub async fn deposit_with_signer(
        &self,
        signer: &dyn TransactionSigner,
        asset_mint: &Pubkey,
        amount: u64,
    ) -> Result<DepositResult> {
        println!("[Carrot] Starting deposit: amount={}, asset_mint={}", amount, asset_mint);
        
        // Get signer's public key
        let member_pubkey_str = signer.get_public_key().await?;
        let member_pubkey = Pubkey::from_str(&member_pubkey_str)
            .map_err(|e| format!("Invalid public key: {}", e))?;
        println!("[Carrot] Member pubkey: {}", member_pubkey);
        
        // Fetch vault account to get remaining accounts
        println!("[Carrot] Fetching vault account...");
        let vault_data = self.get_account(&carrot_sdk::VAULT_ADDRESS).await?;
        
        // Skip 8-byte Anchor discriminator before deserializing
        if vault_data.len() < 8 {
            return Err("Vault data too short".into());
        }
        let vault: carrot_sdk::Vault = borsh::BorshDeserialize::try_from_slice(&vault_data[8..])
            .map_err(|e| format!("Failed to deserialize vault: {}", e))?;
        let remaining_accounts = vault.get_remaining_accounts();
        println!("[Carrot] Got {} remaining accounts from vault", remaining_accounts.len());
        
        // Build instructions
        let mut instructions = Vec::new();
        
        // Create CRT ATA if needed (using Token-2022)
        let token_2022_program = Pubkey::from_str(TOKEN_2022_PROGRAM_ID)
            .expect("Valid Token-2022 program ID");
        let create_crt_ata_ix = spl_associated_token_account::instruction::create_associated_token_account_idempotent(
            &member_pubkey,
            &member_pubkey,
            &carrot_sdk::CRT_MINT,
            &token_2022_program,
        );
        instructions.push(create_crt_ata_ix);
        
        // Build issue (deposit) instruction
        let issue_ix = carrot_sdk::instructions::build_issue_instruction(
            &member_pubkey,
            asset_mint,
            amount,
            remaining_accounts,
        )?;
        instructions.push(issue_ix);
        
        // Get recent blockhash
        println!("[Carrot] Getting recent blockhash...");
        let recent_blockhash = self.get_recent_blockhash().await?;
        
        // Create transaction message
        let message = solana_sdk::message::Message::new(
            &instructions,
            Some(&member_pubkey),
        );
        
        let mut message_with_blockhash = message;
        message_with_blockhash.recent_blockhash = recent_blockhash;
        
        let mut transaction = VersionedTransaction {
            signatures: vec![SolanaSignature::default()],
            message: VersionedMessage::Legacy(message_with_blockhash),
        };
        
        // Sign transaction
        println!("[Carrot] Signing transaction...");
        let message_bytes = transaction.message.serialize();
        let signature_bytes = signer.sign_message(&message_bytes).await?;
        
        if signature_bytes.len() != 64 {
            return Err(format!("Invalid signature length: {}", signature_bytes.len()).into());
        }
        
        let mut sig_array = [0u8; 64];
        sig_array.copy_from_slice(&signature_bytes);
        transaction.signatures[0] = SolanaSignature::from(sig_array);
        
        // Send transaction
        println!("[Carrot] Sending transaction...");
        let serialized = bincode::serialize(&transaction)?;
        let encoded = bs58::encode(serialized).into_string();
        let signature = self.send_transaction(&encoded).await?;
        println!("[Carrot] Deposit successful! Signature: {}", signature);
        
        // Calculate CRT received (approximate based on current rate)
        let crt_received = amount as f64 / 112.0 / 1_000_000_000.0;
        
        Ok(DepositResult {
            signature,
            crt_received,
        })
    }

    /// Withdraw CRT tokens and receive assets
    pub async fn withdraw_with_signer(
        &self,
        signer: &dyn TransactionSigner,
        asset_mint: &Pubkey,
        crt_amount: u64,
    ) -> Result<WithdrawResult> {
        println!("[Carrot] Starting withdraw: crt_amount={}, asset_mint={}", crt_amount, asset_mint);
        
        // Get signer's public key
        let member_pubkey_str = signer.get_public_key().await?;
        let member_pubkey = Pubkey::from_str(&member_pubkey_str)
            .map_err(|e| format!("Invalid public key: {}", e))?;
        println!("[Carrot] Member pubkey: {}", member_pubkey);
        
        // Fetch vault account to get remaining accounts
        println!("[Carrot] Fetching vault account...");
        let vault_data = self.get_account(&carrot_sdk::VAULT_ADDRESS).await?;
        
        // Skip 8-byte Anchor discriminator before deserializing
        if vault_data.len() < 8 {
            return Err("Vault data too short".into());
        }
        let vault: carrot_sdk::Vault = borsh::BorshDeserialize::try_from_slice(&vault_data[8..])
            .map_err(|e| format!("Failed to deserialize vault: {}", e))?;
        let remaining_accounts = vault.get_remaining_accounts();
        println!("[Carrot] Got {} remaining accounts from vault", remaining_accounts.len());
        
        // Build instructions
        let mut instructions = Vec::new();
        
        // Create asset ATA if needed
        let create_asset_ata_ix = spl_associated_token_account::instruction::create_associated_token_account_idempotent(
            &member_pubkey,
            &member_pubkey,
            asset_mint,
            &spl_token::id(),
        );
        instructions.push(create_asset_ata_ix);
        
        // Build redeem (withdraw) instruction
        let redeem_ix = carrot_sdk::instructions::build_redeem_instruction(
            &member_pubkey,
            asset_mint,
            crt_amount,
            remaining_accounts,
        )?;
        instructions.push(redeem_ix);
        
        // Get recent blockhash
        println!("[Carrot] Getting recent blockhash...");
        let recent_blockhash = self.get_recent_blockhash().await?;
        
        // Create transaction message
        let message = solana_sdk::message::Message::new(
            &instructions,
            Some(&member_pubkey),
        );
        
        let mut message_with_blockhash = message;
        message_with_blockhash.recent_blockhash = recent_blockhash;
        
        let mut transaction = VersionedTransaction {
            signatures: vec![SolanaSignature::default()],
            message: VersionedMessage::Legacy(message_with_blockhash),
        };
        
        // Sign transaction
        println!("[Carrot] Signing transaction...");
        let message_bytes = transaction.message.serialize();
        let signature_bytes = signer.sign_message(&message_bytes).await?;
        
        if signature_bytes.len() != 64 {
            return Err(format!("Invalid signature length: {}", signature_bytes.len()).into());
        }
        
        let mut sig_array = [0u8; 64];
        sig_array.copy_from_slice(&signature_bytes);
        transaction.signatures[0] = SolanaSignature::from(sig_array);
        
        // Send transaction
        println!("[Carrot] Sending transaction...");
        let serialized = bincode::serialize(&transaction)?;
        let encoded = bs58::encode(serialized).into_string();
        let signature = self.send_transaction(&encoded).await?;
        println!("[Carrot] Withdraw successful! Signature: {}", signature);
        
        // Calculate asset received (approximate based on current rate)
        let asset_received = crt_amount as f64 * 112.0 / 1_000_000_000.0 / 1_000_000.0;
        
        Ok(WithdrawResult {
            signature,
            asset_received,
        })
    }

    /// Get account data from RPC
    async fn get_account(&self, pubkey: &Pubkey) -> Result<Vec<u8>> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getAccountInfo",
            "params": [
                pubkey.to_string(),
                {
                    "encoding": "base64"
                }
            ]
        });

        let response = self.http_client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await?;

        let json: Value = response.json().await?;

        if let Some(data) = json["result"]["value"]["data"][0].as_str() {
            let decoded = base64::decode(data)
                .map_err(|e| format!("Failed to decode account data: {}", e))?;
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
            "params": [
                {
                    "commitment": "finalized"
                }
            ]
        });

        let response = self.http_client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await?;

        let json: Value = response.json().await?;

        if let Some(blockhash_str) = json["result"]["value"]["blockhash"].as_str() {
            let blockhash = solana_sdk::hash::Hash::from_str(blockhash_str)
                .map_err(|e| format!("Invalid blockhash: {}", e))?;
            Ok(blockhash)
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
}