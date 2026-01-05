use solana_sdk::{
    pubkey::Pubkey, 
    signature::Signature as SolanaSignature,
    transaction::VersionedTransaction,
    message::VersionedMessage,
    instruction::Instruction,
};
use solana_system_interface::instruction as system_instruction;
use std::error::Error as StdError;
use std::str::FromStr;
use serde_json::{json, Value};
use reqwest::Client as HttpClient;

use crate::signing::TransactionSigner;
use crate::carrot::types::{CarrotBalances, DepositResult, WithdrawResult};
use crate::storage::get_current_jito_settings;

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

    /// Get token account balance via RPC (works for both Token and Token-2022)
    async fn get_token_account_balance(&self, wallet_pubkey: &Pubkey, mint_pubkey: &Pubkey) -> Result<u64> {
        // Get associated token account address
        let token_program_id = self.get_mint_program_id(mint_pubkey).await?;
        let ata = spl_associated_token_account::get_associated_token_address_with_program_id(
            wallet_pubkey,
            mint_pubkey,
            &token_program_id,
        );

        // Get account info
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getTokenAccountBalance",
            "params": [
                ata.to_string()
            ]
        });

        let response = self.http_client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await?;

        let json: Value = response.json().await?;

        if let Some(amount_str) = json["result"]["value"]["amount"].as_str() {
            let amount = amount_str.parse::<u64>()
                .map_err(|e| format!("Failed to parse balance: {}", e))?;
            Ok(amount)
        } else {
            // Account doesn't exist or has no balance
            Ok(0)
        }
    }

    /// Get token balances for a wallet
    pub async fn get_balances(&self, wallet_pubkey: &Pubkey) -> Result<CarrotBalances> {
        let mut balances = CarrotBalances::default();

        // Get USDC balance (Token program)
        if let Ok(usdc_balance) = self.get_token_account_balance(wallet_pubkey, &carrot_sdk::USDC_MINT).await {
            balances.usdc = usdc_balance as f64 / 1_000_000.0;
            println!("[Carrot] USDC balance: {}", balances.usdc);
        }

        // Get USDT balance (Token program)
        if let Ok(usdt_balance) = self.get_token_account_balance(wallet_pubkey, &carrot_sdk::USDT_MINT).await {
            balances.usdt = usdt_balance as f64 / 1_000_000.0;
            println!("[Carrot] USDT balance: {}", balances.usdt);
        }

        // Get pyUSD balance (Token-2022 program)
        if let Ok(pyusd_balance) = self.get_token_account_balance(wallet_pubkey, &carrot_sdk::PYUSD_MINT).await {
            balances.pyusd = pyusd_balance as f64 / 1_000_000.0;
            println!("[Carrot] pyUSD balance: {}", balances.pyusd);
        }

        // Get CRT balance (Token-2022 program)
        if let Ok(crt_balance) = self.get_token_account_balance(wallet_pubkey, &carrot_sdk::CRT_MINT).await {
            balances.crt = crt_balance as f64 / 1_000_000_000.0;
            println!("[Carrot] CRT balance: {}", balances.crt);
        }

        Ok(balances)
    }

    /// Deposit assets and receive CRT tokens
    pub async fn deposit_with_signer(
        &self,
        signer: &dyn TransactionSigner,
        asset_mint: &Pubkey,
        amount: u64,
        is_hardware_wallet: bool,
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
        
        // Check Jito settings and add tip if enabled AND not using hardware wallet
        let jito_settings = get_current_jito_settings();
        if jito_settings.jito_tx && !is_hardware_wallet {
            let jito_tip_address = Pubkey::from_str("juLesoSmdTcRtzjCzYzRoHrnF8GhVu6KCV7uxq7nJGp")?;
            let tip_ix = system_instruction::transfer(&member_pubkey, &jito_tip_address, 100_000);
            instructions.push(tip_ix);
            println!("[Carrot] Added Jito tip to deposit transaction");
        } else if is_hardware_wallet {
            println!("[Carrot] Hardware wallet detected - skipping Jito tips");
        }
        
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
        is_hardware_wallet: bool,
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
        
        // Detect which token program this mint uses
        println!("[Carrot] Detecting token program for asset mint...");
        let token_program_id = self.get_mint_program_id(asset_mint).await
            .unwrap_or_else(|_| {
                println!("[Carrot] Failed to detect program, using standard Token program");
                spl_token::id()
            });
        
        // Create asset ATA if needed (using detected program ID)
        let create_asset_ata_ix = spl_associated_token_account::instruction::create_associated_token_account_idempotent(
            &member_pubkey,
            &member_pubkey,
            asset_mint,
            &token_program_id, // Use detected program ID (Token or Token-2022)
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
        
        // Check Jito settings and add tip if enabled AND not using hardware wallet
        let jito_settings = get_current_jito_settings();
        if jito_settings.jito_tx && !is_hardware_wallet {
            let jito_tip_address = Pubkey::from_str("juLesoSmdTcRtzjCzYzRoHrnF8GhVu6KCV7uxq7nJGp")?;
            let tip_ix = system_instruction::transfer(&member_pubkey, &jito_tip_address, 100_000);
            instructions.push(tip_ix);
            println!("[Carrot] Added Jito tip to withdraw transaction");
        } else if is_hardware_wallet {
            println!("[Carrot] Hardware wallet detected - skipping Jito tips");
        }
        
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

    /// Detect which token program owns a mint account (Token or Token-2022)
    async fn get_mint_program_id(&self, mint_pubkey: &Pubkey) -> Result<Pubkey> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getAccountInfo",
            "params": [
                mint_pubkey.to_string(),
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
        
        if let Some(owner_str) = json["result"]["value"]["owner"].as_str() {
            let owner = Pubkey::from_str(owner_str)
                .map_err(|e| format!("Invalid owner pubkey: {}", e))?;
            
            // Check if it's Token-2022 program
            let token_2022_id = Pubkey::from_str(TOKEN_2022_PROGRAM_ID)
                .map_err(|e| format!("Invalid Token-2022 program ID: {}", e))?;
            
            if owner == token_2022_id {
                println!("[Carrot] Mint {} uses Token-2022 program", mint_pubkey);
                Ok(token_2022_id)
            } else {
                // Default to standard Token program
                println!("[Carrot] Mint {} uses standard Token program", mint_pubkey);
                Ok(spl_token::id())
            }
        } else {
            // Default to standard Token program if we can't determine
            println!("[Carrot] Could not determine program for mint {}, defaulting to Token program", mint_pubkey);
            Ok(spl_token::id())
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
