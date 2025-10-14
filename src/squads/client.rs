// src/squads/client.rs
//! Squads client implementation following the TransactionClient pattern

use crate::signing::TransactionSigner;
use crate::squads::types::{MultisigInfo, PendingTransaction, ApprovalResult, Member, Permissions};
use solana_sdk::{
    pubkey::Pubkey,
    signature::Signature as SolanaSignature,
    transaction::VersionedTransaction,
    message::VersionedMessage,
};
use squads_v4_client::{
    accounts::{Multisig, Proposal},
    instructions::{self, ProposalVoteArgs},
    pda,
    types::ProposalStatus,
};
use reqwest::Client;
use serde_json::{json, Value};
use std::error::Error;
use std::str::FromStr;

const SQUADS_PROGRAM_ID: &str = "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf";

/// Client for interacting with Squads v4 multisigs
/// Follows the same pattern as TransactionClient in the app
pub struct SquadsClient {
    client: Client,
    rpc_url: String,
    program_id: Pubkey,
}

impl SquadsClient {
    /// Create a new Squads client
    pub fn new(rpc_url: Option<&str>) -> Self {
        let url = rpc_url
            .unwrap_or("https://johna-k3cr1v-fast-mainnet.helius-rpc.com")
            .to_string();
        let program_id = Pubkey::from_str(SQUADS_PROGRAM_ID)
            .expect("Valid Squads program ID");

        Self {
            client: Client::new(),
            rpc_url: url,
            program_id,
        }
    }

    /// Find all multisigs where the given wallet is a member using Squads V4 API
    pub async fn find_user_multisigs(
        &self,
        wallet_pubkey: &Pubkey,
    ) -> Result<Vec<MultisigInfo>, Box<dyn Error>> {
        println!("[SquadsClient] Fetching multisigs from Squads API for wallet: {}", wallet_pubkey);
        
        // Call Squads V4 API endpoint
        let api_url = format!(
            "https://v4-api.squads.so/multisigs/{}?useProd=true",
            wallet_pubkey
        );
        
        println!("[SquadsClient] API URL: {}", api_url);
        
        let response = self.client
            .get(&api_url)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(format!("API request failed with status {}: {}", status, error_text).into());
        }
        
        let api_responses: Vec<crate::squads::types::SquadsApiResponse> = response.json().await?;
        println!("[SquadsClient] Received {} multisigs from API", api_responses.len());
        
        let mut multisigs = Vec::new();
        
        for api_response in api_responses {
            println!("[SquadsClient] Processing multisig: {}", api_response.address);
            println!("  Threshold: {}/{}", api_response.account.threshold, api_response.account.members.len());
            println!("  Transaction index: {}", api_response.account.transaction_index);
            println!("  Default vault: {}", api_response.default_vault);
            
            // Convert API response to MultisigInfo
            let address = Pubkey::from_str(&api_response.address)?;
            let transaction_index = api_response.account.transaction_index.parse::<u64>()
                .unwrap_or(0);
            
            // Convert API members to squads-v4-client Member type
            let members: Vec<Member> = api_response.account.members.iter()
                .map(|m| {
                    let key = Pubkey::from_str(&m.key).unwrap_or_default();
                    let permissions = Permissions {
                        mask: m.permissions.mask,
                    };
                    Member {
                        key,
                        permissions,
                    }
                })
                .collect();
            
            // Check if wallet is a member
            let is_member = members.iter().any(|m| &m.key == wallet_pubkey);
            
            // Parse vault address
            let vault_address = Pubkey::from_str(&api_response.default_vault)?;
            
            // Fetch vault balance
            let vault_balance = self.get_sol_balance(&vault_address).await.unwrap_or(0.0);
            println!("  Vault balance: {} SOL", vault_balance);
            
            // Extract multisig name from metadata
            let name = api_response.metadata
                .as_ref()
                .and_then(|m| m.name.clone())
                .unwrap_or_else(|| format!("Multisig {}", &api_response.address[..8]));
            
            multisigs.push(MultisigInfo {
                address,
                threshold: api_response.account.threshold,
                members,
                transaction_index,
                is_member,
                vault_address,
                vault_balance,
                name,
            });
            
            println!("[SquadsClient] Added multisig (member: {})", is_member);
        }
        
        println!("[SquadsClient] Found {} multisigs total", multisigs.len());
        Ok(multisigs)
    }
    
    /// Get SOL balance for an address
    async fn get_sol_balance(&self, address: &Pubkey) -> Result<f64, Box<dyn Error>> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getBalance",
            "params": [address.to_string()]
        });

        let response = self.client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await?;

        let json: Value = response.json().await?;

        if let Some(lamports) = json["result"]["value"].as_u64() {
            Ok(lamports as f64 / 1_000_000_000.0)
        } else {
            Err("Failed to get balance".into())
        }
    }

    /// Get details about a specific multisig
    pub async fn get_multisig_info(
        &self,
        multisig_address: &Pubkey,
        wallet_pubkey: &Pubkey,
    ) -> Result<MultisigInfo, Box<dyn Error>> {
        // Fetch the multisig account
        let account_data = self.get_account(multisig_address).await?;
        
        // Deserialize the multisig account using custom deserialization
        let multisig = Multisig::try_from_slice(&account_data)?;

        // Check if wallet is a member
        let is_member = multisig.members.iter().any(|m| &m.key == wallet_pubkey);

        // Derive default vault PDA
        let (vault_address, _) = pda::get_vault_pda(multisig_address, 0, Some(&self.program_id));
        
        // Fetch vault balance
        let vault_balance = self.get_sol_balance(&vault_address).await.unwrap_or(0.0);

        Ok(MultisigInfo {
            address: *multisig_address,
            threshold: multisig.threshold,
            members: multisig.members.clone(),
            transaction_index: multisig.transaction_index,
            is_member,
            vault_address,
            vault_balance,
            name: format!("Multisig {}", &multisig_address.to_string()[..8]),
        })
    }

    /// Find pending transactions for a multisig that need the wallet's approval or execution
    pub async fn find_pending_transactions(
        &self,
        multisig_address: &Pubkey,
        wallet_pubkey: &Pubkey,
    ) -> Result<Vec<PendingTransaction>, Box<dyn Error>> {
        println!("Finding pending transactions for multisig: {}", multisig_address);

        let multisig_info = self.get_multisig_info(multisig_address, wallet_pubkey).await?;
        let mut pending = Vec::new();

        // Check recent transactions (last 10 transactions)
        let start_index = multisig_info.transaction_index.saturating_sub(10);
        
        for tx_index in start_index..=multisig_info.transaction_index {
            // Derive transaction PDA
            let (transaction_pda, _) = pda::get_transaction_pda(
                multisig_address,
                tx_index,
                Some(&self.program_id),
            );

            // Derive proposal PDA
            let (proposal_pda, _) = pda::get_proposal_pda(
                multisig_address,
                tx_index,
                Some(&self.program_id),
            );

            // Try to fetch the proposal
            if let Ok(proposal_data) = self.get_account(&proposal_pda).await {
                let proposal = Proposal::try_from_slice(&proposal_data)?;

                // Check if this proposal needs action (Active = needs approval, Approved = needs execution)
                if matches!(proposal.status, ProposalStatus::Active { .. } | ProposalStatus::Approved { .. }) {
                    // Check if wallet has already approved
                    let has_approved = proposal.approved.contains(wallet_pubkey);

                    pending.push(PendingTransaction {
                        multisig: *multisig_address,
                        transaction_index: tx_index,
                        proposal: proposal_pda,
                        transaction: transaction_pda,
                        status: proposal.status,
                        approved_count: proposal.approved.len() as u16,
                        has_approved,
                        description: format!("Transaction #{}", tx_index),
                    });
                }
            }
        }

        Ok(pending)
    }

    /// Approve a pending transaction with the given signer
    /// This is the main method that integrates with the existing TransactionSigner infrastructure
    pub async fn approve_transaction_with_signer(
        &self,
        signer: &dyn TransactionSigner,
        multisig: &Pubkey,
        transaction_index: u64,
    ) -> Result<ApprovalResult, Box<dyn Error>> {
        println!("Approving transaction {} for multisig {}", transaction_index, multisig);

        // Get signer's public key
        let member_pubkey_str = signer.get_public_key().await?;
        let member_pubkey = Pubkey::from_str(&member_pubkey_str)?;

        // Derive PDAs
        let (transaction_pda, _) = pda::get_transaction_pda(
            multisig,
            transaction_index,
            Some(&self.program_id),
        );

        let (proposal_pda, _) = pda::get_proposal_pda(
            multisig,
            transaction_index,
            Some(&self.program_id),
        );

        // Create approval instruction
        let vote_args = ProposalVoteArgs { memo: None };
        
        let approval_ix = instructions::proposal_approve(
            *multisig,
            proposal_pda,
            member_pubkey,
            vote_args,
            Some(self.program_id),
        );

        // Get recent blockhash
        let recent_blockhash = self.get_recent_blockhash().await?;

        // Create transaction
        let message = solana_sdk::message::Message::new(
            &[approval_ix],
            Some(&member_pubkey),
        );

        let mut message_with_blockhash = message;
        message_with_blockhash.recent_blockhash = recent_blockhash;

        let mut transaction = VersionedTransaction {
            signatures: vec![SolanaSignature::default()],
            message: VersionedMessage::Legacy(message_with_blockhash),
        };

        // Sign the transaction
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

        // Get updated proposal to check if threshold was met
        let proposal_data = self.get_account(&proposal_pda).await?;
        let proposal = Proposal::try_from_slice(&proposal_data)?;
        
        let multisig_data = self.get_account(multisig).await?;
        let multisig_account = Multisig::try_from_slice(&multisig_data)?;

        let threshold_met = proposal.approved.len() as u16 >= multisig_account.threshold;

        Ok(ApprovalResult {
            signature,
            threshold_met,
            approval_count: proposal.approved.len() as u16,
        })
    }

    /// Execute an approved transaction with the given signer
    /// This executes a transaction that has met the approval threshold
    pub async fn execute_transaction_with_signer(
        &self,
        signer: &dyn TransactionSigner,
        multisig: &Pubkey,
        transaction_index: u64,
    ) -> Result<String, Box<dyn Error>> {
        println!("Executing transaction {} for multisig {}", transaction_index, multisig);

        // Get signer's public key
        println!("[Execute] Getting signer public key...");
        let member_pubkey_str = signer.get_public_key().await?;
        let member_pubkey = Pubkey::from_str(&member_pubkey_str)?;
        println!("[Execute] Signer pubkey: {}", member_pubkey);

        // Derive PDAs
        println!("[Execute] Deriving PDAs...");
        let (transaction_pda, _) = pda::get_transaction_pda(
            multisig,
            transaction_index,
            Some(&self.program_id),
        );
        println!("[Execute] Transaction PDA: {}", transaction_pda);

        let (proposal_pda, _) = pda::get_proposal_pda(
            multisig,
            transaction_index,
            Some(&self.program_id),
        );
        println!("[Execute] Proposal PDA: {}", proposal_pda);

        // Fetch the vault transaction to get the accounts needed
        println!("[Execute] Fetching transaction account data...");
        let transaction_data = self.get_account(&transaction_pda).await?;
        println!("[Execute] Transaction data size: {} bytes", transaction_data.len());
        
        println!("[Execute] Deserializing VaultTransaction...");
        let vault_tx = squads_v4_client::accounts::VaultTransaction::try_from_slice(&transaction_data)?;
        println!("[Execute] VaultTransaction deserialized successfully");
        println!("[Execute] Vault index: {}", vault_tx.vault_index);
        println!("[Execute] Account keys count: {}", vault_tx.message.account_keys.len());
        println!("[Execute] Instructions count: {}", vault_tx.message.instructions.len());

        // Build the remaining accounts from the transaction message
        println!("[Execute] Building remaining accounts...");
        let mut remaining_accounts = Vec::new();
        
        // The vault PDA is needed for CPI but it's already included in the transaction message's account_keys
        // We just need to pass through the accounts from the transaction message
        
        // Add all account keys from the transaction message
        // NOTE: The transaction message already includes the vault PDA and all other required accounts
        for (i, account_key) in vault_tx.message.account_keys.iter().enumerate() {
            let is_writable = vault_tx.message.is_static_writable_index(i);
            
            println!("[Execute] Account {}: {} (writable: {})", i, account_key, is_writable);
            
            // Always set is_signer to false for remaining accounts
            // The vault PDA and other accounts will sign within the program, not in this transaction
            if is_writable {
                remaining_accounts.push(solana_sdk::instruction::AccountMeta::new(*account_key, false));
            } else {
                remaining_accounts.push(solana_sdk::instruction::AccountMeta::new_readonly(*account_key, false));
            }
        }
        println!("[Execute] Total remaining accounts: {}", remaining_accounts.len());

        // Create execute instruction
        println!("[Execute] Creating execute instruction...");
        let execute_ix = instructions::vault_transaction_execute(
            *multisig,
            proposal_pda,
            transaction_pda,
            member_pubkey,
            remaining_accounts,
            Some(self.program_id),
        );
        println!("[Execute] Execute instruction created with {} accounts", execute_ix.accounts.len());

        // Get recent blockhash
        println!("[Execute] Getting recent blockhash...");
        let recent_blockhash = self.get_recent_blockhash().await?;
        println!("[Execute] Recent blockhash: {}", recent_blockhash);

        // Create transaction
        println!("[Execute] Creating transaction message...");
        let message = solana_sdk::message::Message::new(
            &[execute_ix],
            Some(&member_pubkey),
        );

        let mut message_with_blockhash = message;
        message_with_blockhash.recent_blockhash = recent_blockhash;

        let mut transaction = VersionedTransaction {
            signatures: vec![SolanaSignature::default()],
            message: VersionedMessage::Legacy(message_with_blockhash),
        };

        // Sign the transaction
        println!("[Execute] Signing transaction...");
        let message_bytes = transaction.message.serialize();
        println!("[Execute] Message bytes size: {}", message_bytes.len());
        let signature_bytes = signer.sign_message(&message_bytes).await?;
        println!("[Execute] Signature received, size: {}", signature_bytes.len());

        if signature_bytes.len() != 64 {
            return Err(format!("Invalid signature length: {}", signature_bytes.len()).into());
        }

        let mut sig_array = [0u8; 64];
        sig_array.copy_from_slice(&signature_bytes);
        transaction.signatures[0] = SolanaSignature::from(sig_array);
        println!("[Execute] Transaction signed successfully");

        // Send transaction
        println!("[Execute] Serializing transaction...");
        let serialized = bincode::serialize(&transaction)?;
        println!("[Execute] Serialized size: {} bytes", serialized.len());
        let encoded = bs58::encode(serialized).into_string();
        println!("[Execute] Encoded transaction, sending to RPC...");
        
        let signature = self.send_transaction(&encoded).await?;
        println!("[Execute] Transaction sent successfully! Signature: {}", signature);

        Ok(signature)
    }

    /// Get account data from the network
    async fn get_account(&self, pubkey: &Pubkey) -> Result<Vec<u8>, Box<dyn Error>> {
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

        let response = self.client
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

    /// Get recent blockhash
    async fn get_recent_blockhash(&self) -> Result<solana_sdk::hash::Hash, Box<dyn Error>> {
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

        if let Some(blockhash_str) = json["result"]["value"]["blockhash"].as_str() {
            let hash = solana_sdk::hash::Hash::from_str(blockhash_str)?;
            Ok(hash)
        } else {
            Err("Failed to get blockhash".into())
        }
    }

    /// Send transaction
    async fn send_transaction(&self, signed_tx: &str) -> Result<String, Box<dyn Error>> {
        println!("[RPC] Preparing sendTransaction request...");
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

        println!("[RPC] Sending transaction to RPC: {}", self.rpc_url);
        let response = self.client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await?;

        println!("[RPC] Received response with status: {}", response.status());
        let json: Value = response.json().await?;
        println!("[RPC] Response JSON: {}", serde_json::to_string_pretty(&json).unwrap_or_else(|_| format!("{:?}", json)));

        if let Some(error) = json.get("error") {
            println!("[RPC] ERROR in response: {:?}", error);
            Err(format!("Transaction error: {:?}", error).into())
        } else if let Some(result) = json["result"].as_str() {
            println!("[RPC] SUCCESS - Transaction signature: {}", result);
            Ok(result.to_string())
        } else {
            println!("[RPC] UNKNOWN RESPONSE FORMAT: {:?}", json);
            Err(format!("Unknown error: {:?}", json).into())
        }
    }
}