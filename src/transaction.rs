// src/transaction.rs
use crate::wallet::Wallet;
use crate::signing::{TransactionSigner, SignerType};
use crate::storage::get_current_jito_settings;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Signature as SolanaSignature},
    transaction::VersionedTransaction,
    message::{Message, VersionedMessage},
    system_instruction,
    hash::Hash,
};
use bs58;
use reqwest::Client;
use std::error::Error;
use std::str::FromStr;
use serde_json::{Value, json};

/// Transaction client for sending transactions
pub struct TransactionClient {
    client: Client,
    rpc_url: String,
}

impl TransactionClient {
    /// Create a new transaction client
    pub fn new(rpc_url: Option<&str>) -> Self {
        let url = rpc_url.unwrap_or("https://serene-stylish-mound.solana-mainnet.quiknode.pro/5489821bcd1547d9cd7b2d81f90c086e36e0e9f7/").to_string();
        Self {
            client: Client::new(),
            rpc_url: url,
        }
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

    /// Send a signed transaction
    pub async fn send_transaction(&self, signed_tx: &str) -> Result<String, Box<dyn Error>> {
        // Check Jito settings
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
        
        // Get recent blockhash
        let recent_blockhash = self.get_recent_blockhash().await?;
        println!("Using blockhash: {}", recent_blockhash);
        
        // Create the transfer instruction using Solana SDK
        let transfer_instruction = system_instruction::transfer(
            &from_pubkey,
            &to_pubkey,
            amount_lamports,
        );
        
        // Start with the basic transfer instruction
        let mut instructions = vec![transfer_instruction];
        
        // Apply Jito modifications if JitoTx is enabled
        if jito_settings.jito_tx {
            println!("JitoTx is enabled, applying Jito modifications");
            self.apply_jito_modifications(&from_pubkey, &mut instructions)?;
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
    ) -> Result<(), Box<dyn Error>> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_transaction_client() {
        let client = TransactionClient::new(None);
        let blockhash = client.get_recent_blockhash().await;
        assert!(blockhash.is_ok());
    }
}