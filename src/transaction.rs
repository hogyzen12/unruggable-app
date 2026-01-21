// src/transaction.rs
use crate::wallet::Wallet;
use crate::signing::{TransactionSigner, SignerType};
use crate::storage::get_current_jito_settings;
use crate::components::modals::bulk_send_modal::SelectedTokenForBulkSend;
use crate::config::TpuConfig;
use crate::timeout;
use solana_sdk::{
    pubkey::Pubkey,
    hash::Hash,
    signature::{Signature as SolanaSignature, Keypair},
    message::{Message, VersionedMessage},
    transaction::VersionedTransaction,
};
use solana_system_interface::instruction as system_instruction;
use bs58;
use reqwest::Client;
use std::error::Error;
use std::str::FromStr;
use std::sync::Arc;
use serde_json::{Value, json};
use spl_token::instruction as token_instruction;
use spl_associated_token_account::get_associated_token_address_with_program_id;
use spl_associated_token_account::instruction::create_associated_token_account;
use std::collections::HashMap;
use yellowstone_jet_tpu_client::yellowstone_grpc::sender::YellowstoneTpuSender;
use tokio::sync::{Mutex, OnceCell};
use std::sync::atomic::{AtomicBool, Ordering};

// Token program IDs
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const TOKEN_2022_PROGRAM_ID: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";

// Add these constants for transaction size management
const MAX_TRANSACTION_SIZE: usize = 1200; // Conservative limit (actual is ~1232)
const ESTIMATED_INSTRUCTION_SIZE: usize = 150; // Estimated bytes per instruction
const HEADER_OVERHEAD: usize = 200; // Transaction header and signature overhead

/// Transaction client for sending transactions
pub struct TransactionClient {
    client: Client,
    rpc_url: String,
    /// Lazy-initialized TPU sender for parallel transaction delivery
    tpu_sender: Arc<OnceCell<Arc<Mutex<YellowstoneTpuSender>>>>,
    /// TPU configuration
    tpu_config: TpuConfig,
    /// Flag to track if TPU initialization has been attempted and failed
    tpu_init_failed: AtomicBool,
}

/// Bulk transaction builder for atomic multi-token sends
pub struct BulkTransactionBuilder {
    /// The sender's public key
    from_pubkey: Pubkey,
    /// The recipient's public key  
    to_pubkey: Pubkey,
    /// List of SOL transfer amounts (in SOL, not lamports)
    sol_transfers: Vec<f64>,
    /// List of SPL token transfers (mint, amount, decimals)
    spl_transfers: Vec<(String, f64, u8)>,
    /// Instructions to be included in the transaction
    instructions: Vec<solana_sdk::instruction::Instruction>,
    /// Track required account creations
    required_ata_creations: Vec<Pubkey>,
}

impl BulkTransactionBuilder {
    /// Create a new bulk transaction builder
    pub fn new(from_pubkey: Pubkey, to_pubkey: Pubkey) -> Self {
        Self {
            from_pubkey,
            to_pubkey,
            sol_transfers: Vec::new(),
            spl_transfers: Vec::new(),
            instructions: Vec::new(),
            required_ata_creations: Vec::new(),
        }
    }

    /// Add a SOL transfer to the bulk transaction
    pub fn add_sol_transfer(&mut self, amount_sol: f64) -> Result<(), Box<dyn Error>> {
        if amount_sol <= 0.0 {
            return Err("SOL amount must be positive".into());
        }
        self.sol_transfers.push(amount_sol);
        Ok(())
    }

    /// Add an SPL token transfer to the bulk transaction
    pub fn add_spl_transfer(
        &mut self, 
        mint: &str, 
        amount: f64
    ) -> Result<(), Box<dyn Error>> {
        if amount <= 0.0 {
            return Err("Token amount must be positive".into());
        }
        // Store mint and amount, decimals will be fetched during build
        self.spl_transfers.push((mint.to_string(), amount, 0)); // 0 as placeholder
        Ok(())
    }

    /// Check if the current instruction set would fit in a single transaction
    fn estimate_transaction_size(&self) -> usize {
        let instruction_count = self.sol_transfers.len() + 
                              self.spl_transfers.len() + 
                              self.required_ata_creations.len();
        
        HEADER_OVERHEAD + (instruction_count * ESTIMATED_INSTRUCTION_SIZE)
    }

    /// Build the transaction instructions (async to check account existence)
    pub async fn build_instructions(
        &mut self,
        client: &TransactionClient,
    ) -> Result<Vec<solana_sdk::instruction::Instruction>, Box<dyn Error>> {
        let mut instructions = Vec::new();

        // First, check which ATA accounts need to be created
        for (mint_str, _, _) in &self.spl_transfers {
            let mint_pubkey = Pubkey::from_str(mint_str)?;
            
            // Detect which token program this mint uses
            let token_program_id = client.get_mint_program_id(&mint_pubkey).await
                .unwrap_or_else(|_| spl_token::id()); // Fallback to standard Token program
            
            let to_token_account = get_associated_token_address_with_program_id(
                &self.to_pubkey,
                &mint_pubkey,
                &token_program_id,
            );
            
            if !client.account_exists(&to_token_account).await? {
                println!("Will create ATA for mint {} -> {}", mint_str, to_token_account);
                self.required_ata_creations.push(mint_pubkey);
                
                let create_ata_instruction = create_associated_token_account(
                    &self.from_pubkey, // Payer
                    &self.to_pubkey,   // Owner
                    &mint_pubkey,      // Token mint
                    &token_program_id, // Token program ID (Token or Token-2022)
                );
                instructions.push(create_ata_instruction);
            }
        }

        // Add SOL transfer instructions
        for &amount_sol in &self.sol_transfers {
            let amount_lamports = (amount_sol * 1_000_000_000.0) as u64;
            let transfer_instruction = system_instruction::transfer(
                &self.from_pubkey,
                &self.to_pubkey,
                amount_lamports,
            );
            instructions.push(transfer_instruction);
        }

        // Add SPL token transfer instructions
        for (mint_str, amount, _) in &self.spl_transfers {
            let mint_pubkey = Pubkey::from_str(mint_str)?;
            
            // Fetch decimals dynamically like single token send does
            let decimals = client.get_token_decimals(&mint_pubkey).await.unwrap_or(6);
            let amount_units = (*amount * 10_f64.powi(decimals as i32)) as u64;
            
            // Detect token program for this mint
            let token_program_id = client.get_mint_program_id(&mint_pubkey).await
                .unwrap_or_else(|_| spl_token::id());
            
            let from_token_account = get_associated_token_address_with_program_id(
                &self.from_pubkey,
                &mint_pubkey,
                &token_program_id,
            );
            let to_token_account = get_associated_token_address_with_program_id(
                &self.to_pubkey,
                &mint_pubkey,
                &token_program_id,
            );
            
            let transfer_instruction = token_instruction::transfer(
                &spl_token::id(),
                &from_token_account,
                &to_token_account,
                &self.from_pubkey,
                &[&self.from_pubkey],
                amount_units,
            )?;
            instructions.push(transfer_instruction);
        }

        self.instructions = instructions.clone();
        Ok(instructions)
    }

    /// Get the estimated number of transactions needed
    pub fn get_estimated_transaction_count(&self) -> usize {
        let estimated_size = self.estimate_transaction_size();
        if estimated_size <= MAX_TRANSACTION_SIZE {
            1
        } else {
            // Rough estimate - could be more sophisticated
            (estimated_size + MAX_TRANSACTION_SIZE - 1) / MAX_TRANSACTION_SIZE
        }
    }

    /// Split instructions into multiple transactions if needed
    pub fn split_for_transaction_limits(&self) -> Vec<Vec<solana_sdk::instruction::Instruction>> {
        if self.estimate_transaction_size() <= MAX_TRANSACTION_SIZE {
            return vec![self.instructions.clone()];
        }

        // For now, implement a simple split strategy
        // In a more sophisticated implementation, you'd optimize the splits
        let mut transactions = Vec::new();
        let mut current_transaction = Vec::new();
        let mut current_size = HEADER_OVERHEAD;

        for instruction in &self.instructions {
            if current_size + ESTIMATED_INSTRUCTION_SIZE > MAX_TRANSACTION_SIZE {
                // Start a new transaction
                if !current_transaction.is_empty() {
                    transactions.push(current_transaction);
                    current_transaction = Vec::new();
                    current_size = HEADER_OVERHEAD;
                }
            }
            
            current_transaction.push(instruction.clone());
            current_size += ESTIMATED_INSTRUCTION_SIZE;
        }

        if !current_transaction.is_empty() {
            transactions.push(current_transaction);
        }

        transactions
    }
}

impl TransactionClient {
    /// Create a new transaction client
    pub fn new(rpc_url: Option<&str>) -> Self {
        let url = rpc_url.unwrap_or("https://johna-k3cr1v-fast-mainnet.helius-rpc.com").to_string();
        let tpu_config = TpuConfig::from_env();
        
        // TPU sender will be initialized lazily on first transaction send
        Self {
            client: Client::new(),
            rpc_url: url,
            tpu_sender: Arc::new(OnceCell::new()),
            tpu_config,
            tpu_init_failed: AtomicBool::new(false),
        }
    }
    
    /// Initialize TPU in the background (non-blocking)
    /// Call this at app startup to avoid lag on first transaction
    /// DISABLED ON iOS: iOS does not support TPU background spawning
    pub fn init_tpu_background(self: &Arc<Self>) {
        #[cfg(target_os = "ios")]
        {
            println!("[TPU] TPU background initialization disabled on iOS");
            return;
        }

        #[cfg(not(target_os = "ios"))]
        {
            if !self.tpu_config.is_valid() {
                println!("[TPU] TPU not configured, skipping background initialization");
                return;
            }

            let client = Arc::clone(self);

            // Use try_spawn to handle runtime issues gracefully
            match tokio::runtime::Handle::try_current() {
                Ok(handle) => {
                    handle.spawn(async move {
                        println!("[TPU] Starting background TPU initialization...");
                        match client.get_tpu_sender().await {
                            Some(_) => println!("[TPU] Background TPU initialization complete"),
                            None => println!("[TPU] Background TPU initialization failed (continuing without TPU)"),
                        }
                    });
                }
                Err(e) => {
                    println!("[TPU] Cannot spawn background task: {:?}", e);
                    println!("[TPU] TPU will initialize on first transaction instead");
                }
            }
        }
    }
    
    /// Get or initialize TPU sender (lazy async initialization)
    /// DISABLED ON iOS: Always returns None on iOS platform
    async fn get_tpu_sender(&self) -> Option<Arc<Mutex<YellowstoneTpuSender>>> {
        // TPU not supported on iOS
        #[cfg(target_os = "ios")]
        {
            return None;
        }

        #[cfg(not(target_os = "ios"))]
        {
            // Return None if TPU is not configured
            if !self.tpu_config.is_valid() {
                return None;
            }
        
        // Check if TPU initialization previously failed - skip retry
        if self.tpu_init_failed.load(Ordering::Relaxed) {
            return None;
        }
        
        // Check if already initialized successfully
        if let Some(sender) = self.tpu_sender.get() {
            return Some(Arc::clone(sender));
        }
        
        // Try to initialize TPU sender
        use yellowstone_jet_tpu_client::yellowstone_grpc::sender::{
            create_yellowstone_tpu_sender, Endpoints,
        };
        
        // Generate ephemeral identity keypair for this TPU sender instance
        let tpu_identity = Keypair::new();
        
        println!("[TPU] Creating TPU sender with:");
        println!("[TPU]   RPC: {}", self.rpc_url);
        println!("[TPU]   gRPC: {}", self.tpu_config.grpc_endpoint);
        println!("[TPU]   Token: {}", if self.tpu_config.grpc_token.is_some() { "***" } else { "none" });
        
        let endpoints = Endpoints {
            rpc: self.rpc_url.clone(),
            grpc: self.tpu_config.grpc_endpoint.clone(),
            grpc_x_token: self.tpu_config.grpc_token.clone(),
        };
        
        // Create the TPU sender asynchronously
        match create_yellowstone_tpu_sender(
            Default::default(),
            tpu_identity,
            endpoints,
        ).await {
            Ok(result) => {
                println!("[TPU] Initialized TPU sender successfully");
                let sender = Arc::new(Mutex::new(result.sender));
                let _ = self.tpu_sender.set(Arc::clone(&sender));
                Some(sender)
            }
            Err(e) => {
                println!("[TPU] Failed to initialize TPU sender: {}. Continuing with RPC only.", e);
                // Mark TPU as failed so we don't retry on subsequent transactions
                self.tpu_init_failed.store(true, Ordering::Relaxed);
                println!("[TPU] TPU initialization disabled for this session");
                None
            }
        }
        }
    }

    /// Send bulk transaction with multiple tokens/SOL
    pub async fn send_bulk_tokens_with_signer(
        &self,
        signer: &dyn TransactionSigner,
        to_address: &str,
        selected_tokens: Vec<SelectedTokenForBulkSend>,
    ) -> Result<String, Box<dyn Error>> {
        // Validate recipient address early
        let to_pubkey = Pubkey::from_str(to_address)?;
        let from_pubkey_str = signer.get_public_key().await?;
        let from_pubkey = Pubkey::from_str(&from_pubkey_str)?;

        if selected_tokens.is_empty() {
            return Err("No tokens selected for bulk send".into());
        }

        println!("Bulk sending {} tokens to {}", selected_tokens.len(), to_address);

        // Create bulk transaction builder
        let mut builder = BulkTransactionBuilder::new(from_pubkey, to_pubkey);

        // Add all transfers to the builder
        for selected_token in &selected_tokens {
            let token = &selected_token.token;
            
            // Check if this is SOL (special case)
            if token.mint == "So11111111111111111111111111111111111111112" || 
               token.symbol.to_uppercase() == "SOL" {
                builder.add_sol_transfer(selected_token.amount)?;
                println!("Added SOL transfer: {} SOL", selected_token.amount);
            } else {
                // Use existing pattern - let transaction client fetch decimals
                builder.add_spl_transfer(&token.mint, selected_token.amount)?;
                println!("Added SPL transfer: {} {} (mint: {})", 
                    selected_token.amount, token.symbol, token.mint);
            }
        }

        // Build the instructions (this will check for ATA creation needs)
        let instructions = builder.build_instructions(self).await?;
        
        println!("Built {} instructions for bulk transaction", instructions.len());

        // Check if we need to split into multiple transactions
        let transaction_batches = builder.split_for_transaction_limits();
        
        if transaction_batches.len() > 1 {
            println!("Transaction too large, splitting into {} batches", transaction_batches.len());
            // For now, return an error - you could implement batch sending
            return Err("Transaction too large for single batch. Multi-batch sending not yet implemented.".into());
        }

        // Send as single transaction
        self.send_bulk_transaction_single(signer, instructions).await
    }

    /// Send a single bulk transaction with all instructions
    async fn send_bulk_transaction_single(
        &self,
        signer: &dyn TransactionSigner,
        mut instructions: Vec<solana_sdk::instruction::Instruction>,
    ) -> Result<String, Box<dyn Error>> {
        // Get current slot and build timeout instruction (FIRST)
        let current_slot = self.get_current_slot().await?;
        let timeout_ix = timeout::build_timeout_instruction_from_current(
            current_slot,
            timeout::DEFAULT_SLOT_WINDOW,
        )?;
        println!("Added timeout protection: current_slot={}, max_slot={}", 
            current_slot, current_slot + timeout::DEFAULT_SLOT_WINDOW);
        
        // Prepend timeout instruction
        instructions.insert(0, timeout_ix);
        
        // Check Jito settings and apply modifications if needed
        let jito_settings = get_current_jito_settings();
        let from_pubkey_str = signer.get_public_key().await?;
        let from_pubkey = Pubkey::from_str(&from_pubkey_str)?;

        if jito_settings.jito_tx {
            println!("JitoTx is enabled, applying Jito modifications to bulk transaction");
            // Note: bulk transactions currently don't support hardware wallets, defaulting to false
            self.apply_jito_modifications(&from_pubkey, &mut instructions, false)?;
        }

        // Get recent blockhash
        let recent_blockhash = self.get_recent_blockhash().await?;
        println!("Using blockhash: {}", recent_blockhash);

        // Create a message with all instructions
        let mut message = Message::new(&instructions, Some(&from_pubkey));
        message.recent_blockhash = recent_blockhash;

        // Create a VersionedTransaction with empty signatures
        let mut transaction = VersionedTransaction {
            signatures: vec![SolanaSignature::default(); message.header.num_required_signatures as usize],
            message: VersionedMessage::Legacy(message),
        };

        println!("Number of signatures expected: {}", transaction.message.header().num_required_signatures);

        // Serialize the transaction message for signing
        let message_bytes = transaction.message.serialize();

        // Sign the message with our signer
        let signature_bytes = signer.sign_message(&message_bytes).await?;

        // Convert to solana signature (expect exactly 64 bytes)
        if signature_bytes.len() != 64 {
            return Err(format!("Invalid signature length: expected 64, got {}", signature_bytes.len()).into());
        }

        let mut sig_array = [0u8; 64];
        sig_array.copy_from_slice(&signature_bytes);
        let solana_signature = SolanaSignature::from(sig_array);

        // Assign the signature to the transaction
        if transaction.signatures.len() != 1 {
            return Err(format!("Expected 1 signature slot, found {}", transaction.signatures.len()).into());
        }
        transaction.signatures[0] = solana_signature;

        // Serialize the entire transaction with signature
        let serialized_transaction = bincode::serialize(&transaction)?;
        let encoded_transaction = bs58::encode(serialized_transaction).into_string();

        println!("Serialized bulk transaction: {} bytes", encoded_transaction.len());

        // Send the transaction
        self.send_transaction(&encoded_transaction).await
    }

    /// Get token decimals for multiple mints (batch operation)
    pub async fn get_token_decimals_batch(&self, mints: &[String]) -> HashMap<String, u8> {
        let mut decimals_map = HashMap::new();
        
        // For now, fetch individually - could be optimized with batch requests
        for mint_str in mints {
            if let Ok(mint_pubkey) = Pubkey::from_str(mint_str) {
                if let Ok(decimals) = self.get_token_decimals(&mint_pubkey).await {
                    decimals_map.insert(mint_str.clone(), decimals);
                } else {
                    // Default to 6 decimals if we can't fetch
                    decimals_map.insert(mint_str.clone(), 6);
                }
            }
        }
        
        decimals_map
    }
    
    /// Get recent blockhash from the network
    pub async fn get_recent_blockhash(&self) -> Result<Hash, Box<dyn Error>> {
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

        let response = self.client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await?;

        let json: Value = response.json().await?;
        
        println!("Blockhash response: {:?}", json);
        
        if let Some(error) = json.get("error") {
            return Err(format!("RPC error: {:?}", error).into());
        }
        
        if let Some(blockhash_str) = json["result"]["value"]["blockhash"].as_str() {
            let hash = Hash::from_str(blockhash_str)?;
            Ok(hash)
        } else {
            Err(format!("Failed to get blockhash from response: {:?}", json).into())
        }
    }

    /// Get current slot number from the network
    pub async fn get_current_slot(&self) -> Result<u64, Box<dyn Error>> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getSlot",
            "params": [
                {
                    "commitment": "confirmed"
                }
            ]
        });

        let response = self.client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await?;

        let json: Value = response.json().await?;
        
        if let Some(error) = json.get("error") {
            return Err(format!("RPC error getting slot: {:?}", error).into());
        }
        
        if let Some(slot) = json["result"].as_u64() {
            Ok(slot)
        } else {
            Err(format!("Failed to get slot from response: {:?}", json).into())
        }
    }

    /// Send a signed transaction with parallel RPC + TPU delivery
    pub async fn send_transaction(&self, signed_tx: &str) -> Result<String, Box<dyn Error>> {
        // Decode the transaction to get signature and serialized bytes
        let tx_bytes = bs58::decode(signed_tx).into_vec()?;
        let transaction: VersionedTransaction = bincode::deserialize(&tx_bytes)?;
        let signature = transaction.signatures[0];
        
        // Parallel TPU send (fire-and-forget if TPU is enabled)
        // DISABLED ON iOS: iOS restricts tokio::spawn in background, causing crashes
        #[cfg(not(target_os = "ios"))]
        {
            if let Some(tpu_sender) = self.get_tpu_sender().await {
                let tpu_sender_clone = Arc::clone(&tpu_sender);
                let tx_bytes_clone = tx_bytes.clone();
                let sig_clone = signature;
                let fanout = self.tpu_config.fanout_count;

                tokio::spawn(async move {
                    let mut sender = tpu_sender_clone.lock().await;
                    match sender.send_txn(sig_clone, tx_bytes_clone).await {
                        Ok(_) => {
                            println!("[TPU] Transaction {} sent via TPU", sig_clone);
                        }
                        Err(e) => {
                            println!("[TPU] Failed to send transaction via TPU: {:?}", e);
                            // Don't fail the whole transaction - RPC might still work
                        }
                    }
                });
            }
        }

        #[cfg(target_os = "ios")]
        {
            println!("[TPU] TPU disabled on iOS - using RPC-only submission");
        }
        
        // RPC send (unchanged - this is the source of truth)
        let jito_settings = get_current_jito_settings();
        
        // Prepare the request, potentially with Jito-specific parameters
        let request = if jito_settings.jito_tx {
            // If JitoTx is enabled, use base64 encoding as recommended by Jito
            // and skip preflight as required by Jito
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "sendTransaction",
                "params": [
                    signed_tx,
                    {
                        "encoding": "base58", // We're still using base58 as that's what our code produces
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

        let response = self.client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await?;

        let json: Value = response.json().await?;
        
        println!("Send transaction response: {:?}", json);
        
        if let Some(error) = json.get("error") {
            Err(format!("Transaction error: {:?}", error).into())
        } else if let Some(result) = json["result"].as_str() {
            Ok(result.to_string())
        } else {
            Err(format!("Unknown error sending transaction: {:?}", json).into())
        }
    }

    /// Send SOL from one wallet to another (original method for backward compatibility)
    pub async fn send_sol(
        &self,
        from_wallet: &Wallet,
        to_address: &str,
        amount_sol: f64,
    ) -> Result<String, Box<dyn Error>> {
        let signer = SignerType::from_wallet(from_wallet.clone());
        self.send_sol_with_signer(&signer, to_address, amount_sol).await
    }

    /// Send SOL without timeout instruction (used for delayed execution flow)
    pub async fn send_sol_no_timeout(
        &self,
        from_wallet: &Wallet,
        to_address: &str,
        amount_sol: f64,
    ) -> Result<String, Box<dyn Error>> {
        let signer = SignerType::from_wallet(from_wallet.clone());
        self.send_sol_with_signer_no_timeout(&signer, to_address, amount_sol).await
    }
    
    /// Send SOL using any signer type
    pub async fn send_sol_with_signer(
        &self,
        signer: &dyn TransactionSigner,
        to_address: &str,
        amount_sol: f64,
    ) -> Result<String, Box<dyn Error>> {
        // Check Jito settings
        let jito_settings = get_current_jito_settings();
        
        // Get the public key from the signer
        let from_pubkey_str = signer.get_public_key().await?;
        let from_pubkey = Pubkey::from_str(&from_pubkey_str)?;
        let to_pubkey = Pubkey::from_str(to_address)?;
        
        // Convert SOL to lamports
        let amount_lamports = (amount_sol * 1_000_000_000.0) as u64;
        
        println!("Sending {} lamports ({} SOL) from {} to {}", 
            amount_lamports, amount_sol, from_pubkey, to_pubkey);
        
        // Get current slot and build timeout instruction (FIRST)
        let current_slot = self.get_current_slot().await?;
        let timeout_ix = timeout::build_timeout_instruction_from_current(
            current_slot,
            timeout::DEFAULT_SLOT_WINDOW,
        )?;
        println!("Added timeout protection: current_slot={}, max_slot={}", 
            current_slot, current_slot + timeout::DEFAULT_SLOT_WINDOW);
        
        // Get recent blockhash
        let recent_blockhash = self.get_recent_blockhash().await?;
        println!("Using blockhash: {}", recent_blockhash);
        
        // Create the transfer instruction using Solana SDK
        let transfer_instruction = system_instruction::transfer(
            &from_pubkey,
            &to_pubkey,
            amount_lamports,
        );
        
        // Build instructions with timeout FIRST
        let mut instructions = vec![timeout_ix, transfer_instruction];
        
        // Apply Jito modifications if JitoTx is enabled
        if jito_settings.jito_tx {
            println!("JitoTx is enabled, applying Jito modifications");
            self.apply_jito_modifications(&from_pubkey, &mut instructions, signer.is_hardware())?;
        }
        
        // Create a message with all instructions
        let mut message = Message::new(&instructions, Some(&from_pubkey));
        message.recent_blockhash = recent_blockhash;
        
        // Create a VersionedTransaction with empty signatures
        let mut transaction = VersionedTransaction {
            signatures: vec![SolanaSignature::default(); message.header.num_required_signatures as usize],
            message: VersionedMessage::Legacy(message),
        };
        
        println!("Number of signatures expected: {}", transaction.message.header().num_required_signatures);
        
        // Serialize the transaction message for signing
        let message_bytes = transaction.message.serialize();
        
        // Sign the message with our signer
        let signature_bytes = signer.sign_message(&message_bytes).await?;
        
        // Convert to solana signature (expect exactly 64 bytes)
        if signature_bytes.len() != 64 {
            return Err(format!("Invalid signature length: expected 64, got {}", signature_bytes.len()).into());
        }
        
        let mut sig_array = [0u8; 64];
        sig_array.copy_from_slice(&signature_bytes);
        let solana_signature = SolanaSignature::from(sig_array);
        
        // Assign the signature to the transaction
        if transaction.signatures.len() != 1 {
            return Err(format!("Expected 1 signature slot, found {}", transaction.signatures.len()).into());
        }
        transaction.signatures[0] = solana_signature;
        
        // Serialize the entire transaction with signature
        let serialized_transaction = bincode::serialize(&transaction)?;
        let encoded_transaction = bs58::encode(serialized_transaction).into_string();
        
        println!("Serialized transaction: {} bytes", encoded_transaction.len());
        
        // Send the transaction
        self.send_transaction(&encoded_transaction).await
    }

    /// Send SOL using any signer type without timeout instruction
    pub async fn send_sol_with_signer_no_timeout(
        &self,
        signer: &dyn TransactionSigner,
        to_address: &str,
        amount_sol: f64,
    ) -> Result<String, Box<dyn Error>> {
        let jito_settings = get_current_jito_settings();
        let from_pubkey_str = signer.get_public_key().await?;
        let from_pubkey = Pubkey::from_str(&from_pubkey_str)?;
        let to_pubkey = Pubkey::from_str(to_address)?;
        let amount_lamports = (amount_sol * 1_000_000_000.0) as u64;

        let recent_blockhash = self.get_recent_blockhash().await?;
        let transfer_instruction = system_instruction::transfer(
            &from_pubkey,
            &to_pubkey,
            amount_lamports,
        );
        let mut instructions = vec![transfer_instruction];

        if jito_settings.jito_tx {
            self.apply_jito_modifications(&from_pubkey, &mut instructions, signer.is_hardware())?;
        }

        let mut message = Message::new(&instructions, Some(&from_pubkey));
        message.recent_blockhash = recent_blockhash;

        let mut transaction = VersionedTransaction {
            signatures: vec![SolanaSignature::default(); message.header.num_required_signatures as usize],
            message: VersionedMessage::Legacy(message),
        };

        let message_bytes = transaction.message.serialize();
        let signature_bytes = signer.sign_message(&message_bytes).await?;
        if signature_bytes.len() != 64 {
            return Err(format!("Invalid signature length: expected 64, got {}", signature_bytes.len()).into());
        }
        let mut sig_array = [0u8; 64];
        sig_array.copy_from_slice(&signature_bytes);
        transaction.signatures[0] = SolanaSignature::from(sig_array);

        let serialized_transaction = bincode::serialize(&transaction)?;
        let encoded_transaction = bs58::encode(serialized_transaction).into_string();
        self.send_transaction(&encoded_transaction).await
    }

    // Send SPL token transaction using wallet
    pub async fn send_spl_token(
        &self,
        from_wallet: &Wallet,
        to_address: &str,
        amount: f64,
        token_mint: &str,
    ) -> Result<String, Box<dyn Error>> {
        let signer = SignerType::from_wallet(from_wallet.clone());
        self.send_spl_token_with_signer(&signer, to_address, amount, token_mint).await
    }

    /// Send SPL token transaction using any signer type
    pub async fn send_spl_token_with_signer(
        &self,
        signer: &dyn TransactionSigner,
        to_address: &str,
        amount: f64,
        token_mint: &str,
    ) -> Result<String, Box<dyn Error>> {
        // Check Jito settings
        let jito_settings = get_current_jito_settings();
        
        let from_pubkey_str = signer.get_public_key().await?;
        let from_pubkey = Pubkey::from_str(&from_pubkey_str)?;
        let to_pubkey = Pubkey::from_str(to_address)?;
        let mint_pubkey = Pubkey::from_str(token_mint)?;
        
        println!("Sending {} tokens from {} to {} (mint: {})", 
            amount, from_pubkey, to_pubkey, mint_pubkey);
        
        // Get current slot and build timeout instruction (FIRST)
        let current_slot = self.get_current_slot().await?;
        let timeout_ix = timeout::build_timeout_instruction_from_current(
            current_slot,
            timeout::DEFAULT_SLOT_WINDOW,
        )?;
        println!("Added timeout protection: current_slot={}, max_slot={}", 
            current_slot, current_slot + timeout::DEFAULT_SLOT_WINDOW);
        
        // Get token info to determine decimals
        let token_decimals = self.get_token_decimals(&mint_pubkey).await
            .unwrap_or(6); // Default to 6 decimals if we can't fetch
            
        // Convert amount to token units (accounting for decimals)
        let amount_units = (amount * 10_f64.powi(token_decimals as i32)) as u64;
        
        println!("Token amount in units: {} (decimals: {})", amount_units, token_decimals);
        
        // Detect which token program this mint uses
        let token_program_id = self.get_mint_program_id(&mint_pubkey).await
            .unwrap_or_else(|_| spl_token::id()); // Fallback to standard Token program
        
        // Get associated token accounts
        let from_token_account = get_associated_token_address_with_program_id(
            &from_pubkey,
            &mint_pubkey,
            &token_program_id,
        );
        let to_token_account = get_associated_token_address_with_program_id(
            &to_pubkey,
            &mint_pubkey,
            &token_program_id,
        );
        
        println!("From token account: {}", from_token_account);
        println!("To token account: {}", to_token_account);
        
        // Get recent blockhash
        let recent_blockhash = self.get_recent_blockhash().await?;
        println!("Using blockhash: {}", recent_blockhash);
        
        // Build instructions starting with timeout
        let mut instructions = vec![timeout_ix];
        
        if !self.account_exists(&to_token_account).await? {
            println!("Creating destination token account: {}", to_token_account);
            
            // Detect which token program this mint uses
            let token_program_id = self.get_mint_program_id(&mint_pubkey).await
                .unwrap_or_else(|_| spl_token::id()); // Fallback to standard Token program
            
            // Create associated token account for recipient
            let create_ata_instruction = create_associated_token_account(
                &from_pubkey, // Payer (sender pays for account creation)
                &to_pubkey,   // Owner of the new account
                &mint_pubkey, // Token mint
                &token_program_id, // Token program ID (Token or Token-2022)
            );
            
            instructions.push(create_ata_instruction);
        }
        
        // Create the token transfer instruction
        let transfer_instruction = token_instruction::transfer(
            &spl_token::id(),                    // Token program ID
            &from_token_account,                 // Source token account
            &to_token_account,                   // Destination token account  
            &from_pubkey,                        // Authority (owner of source account)
            &[&from_pubkey],                     // Signers
            amount_units,                        // Amount in token units
        )?;
        
        instructions.push(transfer_instruction);
        
        // Apply Jito modifications if JitoTx is enabled
        if jito_settings.jito_tx {
            println!("JitoTx is enabled, applying Jito modifications");
            self.apply_jito_modifications(&from_pubkey, &mut instructions, signer.is_hardware())?;
        }
        
        // Create a message with all instructions
        let mut message = Message::new(&instructions, Some(&from_pubkey));
        message.recent_blockhash = recent_blockhash;
        
        // Create a VersionedTransaction with empty signatures
        let mut transaction = VersionedTransaction {
            signatures: vec![SolanaSignature::default(); message.header.num_required_signatures as usize],
            message: VersionedMessage::Legacy(message),
        };
        
        println!("Number of signatures expected: {}", transaction.message.header().num_required_signatures);
        
        // Serialize the transaction message for signing
        let message_bytes = transaction.message.serialize();
        
        // Sign the message with our signer
        let signature_bytes = signer.sign_message(&message_bytes).await?;
        
        // Convert to solana signature (expect exactly 64 bytes)
        if signature_bytes.len() != 64 {
            return Err(format!("Invalid signature length: expected 64, got {}", signature_bytes.len()).into());
        }
        
        let mut sig_array = [0u8; 64];
        sig_array.copy_from_slice(&signature_bytes);
        let solana_signature = SolanaSignature::from(sig_array);
        
        // Assign the signature to the transaction
        if transaction.signatures.len() != 1 {
            return Err(format!("Expected 1 signature slot, found {}", transaction.signatures.len()).into());
        }
        transaction.signatures[0] = solana_signature;
        
        // Serialize the entire transaction with signature
        let serialized_transaction = bincode::serialize(&transaction)?;
        let encoded_transaction = bs58::encode(serialized_transaction).into_string();
        
        println!("Serialized SPL token transaction: {} bytes", encoded_transaction.len());
        
        // Send the transaction
        self.send_transaction(&encoded_transaction).await
    }

    /// Detect which token program owns a mint account (Token or Token-2022)
    async fn get_mint_program_id(&self, mint_pubkey: &Pubkey) -> Result<Pubkey, Box<dyn Error>> {
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

        let response = self.client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await?;

        let json: Value = response.json().await?;
        
        if let Some(owner_str) = json["result"]["value"]["owner"].as_str() {
            let owner = Pubkey::from_str(owner_str)?;
            
            // Check if it's Token-2022 program
            let token_2022_id = Pubkey::from_str(TOKEN_2022_PROGRAM_ID)?;
            if owner == token_2022_id {
                println!("Mint {} uses Token-2022 program", mint_pubkey);
                Ok(token_2022_id)
            } else {
                // Default to standard Token program
                println!("Mint {} uses standard Token program", mint_pubkey);
                Ok(spl_token::id())
            }
        } else {
            // Default to standard Token program if we can't determine
            Ok(spl_token::id())
        }
    }

    /// Get token decimals for a given mint
    async fn get_token_decimals(&self, mint_pubkey: &Pubkey) -> Result<u8, Box<dyn Error>> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getAccountInfo",
            "params": [
                mint_pubkey.to_string(),
                {
                    "encoding": "jsonParsed"
                }
            ]
        });

        let response = self.client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await?;

        let json: Value = response.json().await?;
        
        if let Some(account_data) = json["result"]["value"]["data"]["parsed"]["info"]["decimals"].as_u64() {
            Ok(account_data as u8)
        } else {
            Err("Failed to get token decimals".into())
        }
    }

    /// Check if an account exists
    async fn account_exists(&self, account_pubkey: &Pubkey) -> Result<bool, Box<dyn Error>> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getAccountInfo",
            "params": [
                account_pubkey.to_string(),
                {
                    "encoding": "base64"
                }
            ]
        });

        let response = self.client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await?;

        let json: Value = response.json().await?;
        
        // Account exists if the result value is not null
        Ok(!json["result"]["value"].is_null())
    }

    /// Confirm transaction status
    pub async fn confirm_transaction(&self, signature: &str) -> Result<bool, Box<dyn Error>> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getSignatureStatuses",
            "params": [[signature]]
        });

        let response = self.client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await?;

        let json: Value = response.json().await?;
        
        if let Some(result) = json["result"]["value"][0]["confirmationStatus"].as_str() {
            Ok(result == "finalized" || result == "confirmed")
        } else {
            Ok(false)
        }
    }

    //Jito tx options
    fn apply_jito_modifications(
        &self,
        from_pubkey: &Pubkey,
        instructions: &mut Vec<solana_sdk::instruction::Instruction>,
        is_hardware_wallet: bool,
    ) -> Result<(), Box<dyn Error>> {
        // Skip Jito tips for hardware wallet transactions
        if is_hardware_wallet {
            println!("Hardware wallet detected - skipping Jito tips");
            return Ok(());
        }
        
        // First Jito address (as per JS example)
        let jito_address1 = Pubkey::from_str("juLesoSmdTcRtzjCzYzRoHrnF8GhVu6KCV7uxq7nJGp")?;
        
        // Second Jito address (as per JS example)
        let jito_address2 = Pubkey::from_str("DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL")?;

        // Add two transfer instructions as tips to Jito
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

        println!("Added Jito tip instructions to transaction");
        Ok(())
    }
}
