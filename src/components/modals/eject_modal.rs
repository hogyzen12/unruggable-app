// src/components/modals/eject_modal.rs

use dioxus::prelude::*;
use crate::components::common::Token;
use crate::wallet::{Wallet, WalletInfo};
use crate::hardware::HardwareWallet;
use crate::transaction::TransactionClient;
use crate::signing::{SignerType, hardware::HardwareSigner};
use crate::signing::TransactionSigner;
use crate::components::address_input::AddressInput;
use solana_sdk::{
    pubkey::Pubkey,
    transaction::{Transaction, VersionedTransaction},
    message::{Message, VersionedMessage},
    system_instruction,
    signature::Signature,
};
use spl_token::instruction as token_instruction;
use spl_associated_token_account;
use serde::Deserialize;
use reqwest;
use std::sync::Arc;
use std::collections::HashSet;
use std::str::FromStr;
use base64;
use bs58;

const SOL_MINT: &str = "So11111111111111111111111111111111111111112";

#[derive(Debug, Clone, PartialEq)]
pub enum EjectTokenStatus {
    Pending,
    FetchingQuote,
    SwappingToSol,
    SwapSuccess { sol_received: f64 },
    SwapFailed { reason: String },
    ClosingAccount,
    AccountClosed { rent_reclaimed: f64 },
    CloseFailed { reason: String },
    Complete { sol_received: f64, rent_reclaimed: f64 },
    Failed { reason: String },
}

impl EjectTokenStatus {
    pub fn is_complete(&self) -> bool {
        matches!(self, EjectTokenStatus::Complete { .. } | EjectTokenStatus::Failed { .. })
    }

    pub fn status_text(&self) -> String {
        match self {
            EjectTokenStatus::Pending => "Waiting...".to_string(),
            EjectTokenStatus::FetchingQuote => "Fetching swap quote...".to_string(),
            EjectTokenStatus::SwappingToSol => "Swapping to SOL...".to_string(),
            EjectTokenStatus::SwapSuccess { sol_received } => format!("Swapped → {:.4} SOL", sol_received),
            EjectTokenStatus::SwapFailed { reason } => format!("Swap failed: {}", reason),
            EjectTokenStatus::ClosingAccount => "Closing token account...".to_string(),
            EjectTokenStatus::AccountClosed { rent_reclaimed } => format!("Closed → {:.6} SOL rent", rent_reclaimed),
            EjectTokenStatus::CloseFailed { reason } => format!("Close failed: {}", reason),
            EjectTokenStatus::Complete { sol_received, rent_reclaimed } => {
                format!("✅ Complete: {:.4} SOL + {:.6} rent", sol_received, rent_reclaimed)
            }
            EjectTokenStatus::Failed { reason } => format!("❌ Failed: {}", reason),
        }
    }

    pub fn status_color(&self) -> &str {
        match self {
            EjectTokenStatus::Pending => "#94a3b8",
            EjectTokenStatus::FetchingQuote | EjectTokenStatus::SwappingToSol | EjectTokenStatus::ClosingAccount => "#3b82f6",
            EjectTokenStatus::SwapSuccess { .. } | EjectTokenStatus::AccountClosed { .. } => "#10b981",
            EjectTokenStatus::Complete { .. } => "#10b981",
            EjectTokenStatus::SwapFailed { .. } | EjectTokenStatus::CloseFailed { .. } | EjectTokenStatus::Failed { .. } => "#ef4444",
        }
    }
}

// Jupiter Ultra API response
#[derive(Debug, Deserialize)]
struct JupiterOrderResponse {
    #[serde(rename = "inAmount")]
    in_amount: String,
    #[serde(rename = "outAmount")]
    out_amount: String,
    transaction: Option<String>, // base64 encoded unsigned transaction
    #[serde(rename = "requestId")]
    request_id: String,
}

/// Try to swap a token to SOL using Jupiter
async fn try_swap_to_sol(
    token_mint: &str,
    amount_lamports: u64,
    user_pubkey: &str,
    _decimals: u8,
) -> Result<(String, f64), String> {
    let client = reqwest::Client::new();

    let url = format!(
        "https://lite-api.jup.ag/ultra/v1/order?inputMint={}&outputMint={}&amount={}&taker={}",
        token_mint, SOL_MINT, amount_lamports, user_pubkey
    );

    println!("🔄 Attempting swap: {} lamports of {}", amount_lamports, token_mint);

    match client.get(&url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<JupiterOrderResponse>().await {
                    Ok(order) => {
                        if let Some(tx) = order.transaction {
                            // Parse the output amount to get SOL received
                            let sol_out: f64 = order.out_amount.parse().unwrap_or(0.0) / 1_000_000_000.0;
                            println!("✅ Swap quote received: {} SOL", sol_out);
                            Ok((tx, sol_out))
                        } else {
                            Err("No transaction in response".to_string())
                        }
                    }
                    Err(e) => Err(format!("Failed to parse Jupiter response: {}", e)),
                }
            } else {
                Err(format!("Jupiter API error: {}", response.status()))
            }
        }
        Err(e) => Err(format!("Network error: {}", e)),
    }
}

/// Build and execute EJECT transaction: swap -> close account -> optional send
async fn execute_eject<F>(
    tokens: Vec<Token>,
    wallet: Option<WalletInfo>,
    hardware_wallet: Option<Arc<HardwareWallet>>,
    custom_rpc: Option<String>,
    recipient: Option<Pubkey>,
    has_send: bool,
    mut status_callback: F,
) -> Result<(String, f64, Option<String>), String>
where
    F: FnMut(usize, EjectTokenStatus),
{
    println!("🚀 Starting EJECT execution for {} tokens", tokens.len());

    // Get signer - following the same pattern as bulk_send_modal.rs
    let signer: Box<dyn TransactionSigner> = if let Some(hw) = hardware_wallet {
        // Use hardware wallet signer
        Box::new(HardwareSigner::from_wallet(hw))
    } else if let Some(wallet_info) = wallet {
        // Use software wallet signer
        match Wallet::from_wallet_info(&wallet_info) {
            Ok(wallet) => {
                let signer = SignerType::from_wallet(wallet);
                Box::new(signer)
            }
            Err(e) => {
                return Err(format!("Failed to load wallet: {}", e));
            }
        }
    } else {
        return Err("No wallet available".to_string());
    };

    let user_pubkey_str = match signer.get_public_key().await {
        Ok(pk) => pk,
        Err(e) => return Err(format!("Failed to get public key: {}", e)),
    };

    let user_pubkey = match Pubkey::from_str(&user_pubkey_str) {
        Ok(pk) => pk,
        Err(e) => return Err(format!("Invalid public key: {}", e)),
    };

    println!("📍 User address: {}", user_pubkey_str);

    // Create transaction client
    let tx_client = TransactionClient::new(custom_rpc.as_deref());

    // Process each token
    let mut total_sol_reclaimed = 0.0;
    let mut last_signature = String::new();

    for (index, token) in tokens.iter().enumerate() {
        println!("\n🔄 Processing token: {} ({})", token.symbol, token.mint);

        // Calculate amount in lamports
        let amount_lamports = (token.balance * 10_f64.powi(token.decimals as i32)) as u64;

        if amount_lamports == 0 {
            println!("⚠️ Token {} has 0 balance, skipping", token.symbol);
            status_callback(index, EjectTokenStatus::Failed { reason: "Zero balance".to_string() });
            continue;
        }

        // Update status: Fetching quote
        status_callback(index, EjectTokenStatus::FetchingQuote);

        // Try to swap via Jupiter
        match try_swap_to_sol(&token.mint, amount_lamports, &user_pubkey_str, token.decimals).await {
            Ok((unsigned_tx_b64, sol_out)) => {
                println!("✅ Swap quote obtained: {} SOL expected", sol_out);

                // Update status: Swapping
                status_callback(index, EjectTokenStatus::SwappingToSol);

                // Sign and execute the swap transaction
                match sign_and_execute_transaction(&unsigned_tx_b64, &*signer, &tx_client).await {
                    Ok(signature) => {
                        println!("✅ Swap successful: {}", signature);
                        last_signature = signature;

                        // Update status: Swap success
                        status_callback(index, EjectTokenStatus::SwapSuccess { sol_received: sol_out });

                        // Now close the token account
                        status_callback(index, EjectTokenStatus::ClosingAccount);

                        match close_token_account(&token, &user_pubkey, &*signer, &tx_client).await {
                            Ok((close_sig, rent)) => {
                                println!("✅ Closed token account: {}", close_sig);
                                last_signature = close_sig;
                                total_sol_reclaimed += sol_out + rent;
                                status_callback(index, EjectTokenStatus::Complete {
                                    sol_received: sol_out,
                                    rent_reclaimed: rent
                                });
                            }
                            Err(e) => {
                                println!("⚠️ Failed to close account: {}", e);
                                // Still count the swap as success
                                total_sol_reclaimed += sol_out;
                                status_callback(index, EjectTokenStatus::Complete {
                                    sol_received: sol_out,
                                    rent_reclaimed: 0.0
                                });
                            }
                        }
                    }
                    Err(e) => {
                        println!("❌ Swap execution failed: {}", e);
                        status_callback(index, EjectTokenStatus::SwapFailed { reason: e.clone() });

                        // Fallback: try to close the account without swap
                        status_callback(index, EjectTokenStatus::ClosingAccount);
                        match close_token_account(&token, &user_pubkey, &*signer, &tx_client).await {
                            Ok((sig, rent)) => {
                                println!("✅ Closed account: {}", sig);
                                last_signature = sig;
                                total_sol_reclaimed += rent;
                                status_callback(index, EjectTokenStatus::Complete {
                                    sol_received: 0.0,
                                    rent_reclaimed: rent
                                });
                            }
                            Err(e) => {
                                println!("❌ Close failed: {}", e);
                                status_callback(index, EjectTokenStatus::Failed { reason: e });
                            }
                        }
                    }
                }
            }
            Err(e) => {
                println!("⚠️ Swap not available: {}", e);
                status_callback(index, EjectTokenStatus::SwapFailed { reason: e });

                // Try to close the account anyway
                status_callback(index, EjectTokenStatus::ClosingAccount);
                match close_token_account(&token, &user_pubkey, &*signer, &tx_client).await {
                    Ok((sig, rent)) => {
                        println!("✅ Closed account: {}", sig);
                        last_signature = sig;
                        total_sol_reclaimed += rent;
                        status_callback(index, EjectTokenStatus::Complete {
                            sol_received: 0.0,
                            rent_reclaimed: rent
                        });
                    }
                    Err(e) => {
                        println!("❌ Close failed: {}", e);
                        status_callback(index, EjectTokenStatus::Failed { reason: e });
                    }
                }
            }
        }
    }

    println!("\n💰 Total SOL reclaimed: {} SOL", total_sol_reclaimed);

    // If recipient is specified, send the SOL
    let mut send_signature: Option<String> = None;
    if let Some(recipient_pubkey) = recipient {
        println!("📤 Sending {} SOL to recipient: {}", total_sol_reclaimed, recipient_pubkey);
        
        // Use the send index (after all token operations)
        let send_index = tokens.len();
        
        // Update status callback for send operation
        if has_send {
            status_callback(send_index, EjectTokenStatus::Pending);
            status_callback(send_index, EjectTokenStatus::SwappingToSol); // Reuse "processing" status
        }

        match send_sol_to_recipient(
            &user_pubkey,
            &recipient_pubkey,
            total_sol_reclaimed,
            &*signer,
            &tx_client
        ).await {
            Ok(sig) => {
                println!("✅ SOL sent successfully: {}", sig);
                send_signature = Some(sig.clone());
                last_signature = sig.clone();
                
                if has_send {
                    status_callback(send_index, EjectTokenStatus::Complete {
                        sol_received: total_sol_reclaimed,
                        rent_reclaimed: 0.0
                    });
                }
            }
            Err(e) => {
                println!("❌ Failed to send SOL: {}", e);
                if has_send {
                    status_callback(send_index, EjectTokenStatus::Failed { reason: e.clone() });
                }
            }
        }
    }

    // Return the final signature, total SOL, and optional send signature
    Ok((last_signature, total_sol_reclaimed, send_signature))
}

/// Sign and execute a transaction
async fn sign_and_execute_transaction(
    unsigned_tx_b64: &str,
    signer: &dyn TransactionSigner,
    tx_client: &TransactionClient,
) -> Result<String, String> {
    // Decode the base64 transaction
    let unsigned_tx_bytes = base64::decode(unsigned_tx_b64)
        .map_err(|e| format!("Failed to decode transaction: {}", e))?;

    // Deserialize transaction
    let mut transaction: VersionedTransaction = bincode::deserialize(&unsigned_tx_bytes)
        .map_err(|e| format!("Failed to deserialize transaction: {}", e))?;

    // Sign the message
    let message_bytes = transaction.message.serialize();
    let signature_bytes = signer.sign_message(&message_bytes).await
        .map_err(|e| format!("Failed to sign: {}", e))?;

    // Apply signature
    let mut sig_array = [0u8; 64];
    sig_array.copy_from_slice(&signature_bytes);
    transaction.signatures[0] = Signature::from(sig_array);

    // Serialize and send
    let signed_tx_bytes = bincode::serialize(&transaction)
        .map_err(|e| format!("Failed to serialize signed transaction: {}", e))?;

    let signed_tx_b58 = bs58::encode(&signed_tx_bytes).into_string();

    // Send the transaction
    tx_client.send_transaction(&signed_tx_b58).await
        .map_err(|e| format!("{:?}", e))
}

/// Close a token account to reclaim rent
async fn close_token_account(
    token: &Token,
    owner: &Pubkey,
    signer: &dyn TransactionSigner,
    tx_client: &TransactionClient,
) -> Result<(String, f64), String> {
    println!("🔒 Closing token account: {}", token.symbol);
    
    let token_mint = Pubkey::from_str(&token.mint)
        .map_err(|e| format!("Invalid mint: {}", e))?;
    
    // Derive the Associated Token Account address
    let token_account = spl_associated_token_account::get_associated_token_address(
        owner,
        &token_mint
    );
    
    println!("📍 Token account to close: {}", token_account);
    
    // Create close account instruction
    let close_instruction = token_instruction::close_account(
        &spl_token::id(),
        &token_account,
        owner,  // destination for rent
        owner,  // account owner
        &[],    // signers (will be added during signing)
    )
    .map_err(|e| format!("Failed to create close instruction: {}", e))?;
    
    // Add revenue tip instruction (0.0001 SOL)
    let revenue_address = Pubkey::from_str("juLesoSmdTcRtzjCzYzRoHrnF8GhVu6KCV7uxq7nJGp")
        .map_err(|e| format!("Invalid revenue address: {}", e))?;
    let tip_instruction = system_instruction::transfer(
        owner,
        &revenue_address,
        100_000, // 0.0001 SOL in lamports
    );
    
    // Get recent blockhash
    let recent_blockhash = tx_client.get_recent_blockhash().await
        .map_err(|e| format!("Failed to get blockhash: {:?}", e))?;
    
    // Create transaction with both instructions
    let message = Message::new(&[close_instruction, tip_instruction], Some(owner));
    let mut transaction = Transaction::new_unsigned(message);
    transaction.message.recent_blockhash = recent_blockhash;

    // Sign the transaction
    let message_bytes = bincode::serialize(&transaction.message)
        .map_err(|e| format!("Failed to serialize message: {}", e))?;

    let signature_bytes = signer.sign_message(&message_bytes).await
        .map_err(|e| format!("Failed to sign: {}", e))?;

    let mut sig_array = [0u8; 64];
    sig_array.copy_from_slice(&signature_bytes);
    transaction.signatures = vec![Signature::from(sig_array)];

    // Serialize and send
    let signed_tx_bytes = bincode::serialize(&transaction)
        .map_err(|e| format!("Failed to serialize transaction: {}", e))?;

    let signed_tx_b58 = bs58::encode(&signed_tx_bytes).into_string();

    // Send the transaction
    let signature = tx_client.send_transaction(&signed_tx_b58).await
        .map_err(|e| format!("{:?}", e))?;

    // Standard rent-exempt amount for a token account
    let rent_reclaimed = 0.00203928;

    Ok((signature, rent_reclaimed))
}

/// Send SOL to a recipient address
async fn send_sol_to_recipient(
    sender: &Pubkey,
    recipient: &Pubkey,
    amount_sol: f64,
    signer: &dyn TransactionSigner,
    tx_client: &TransactionClient,
) -> Result<String, String> {
    println!("💸 Sending {} SOL from {} to {}", amount_sol, sender, recipient);
    
    // Convert SOL to lamports, leave some for fee and tip
    let fee_buffer = 0.0011; // Keep 0.0011 SOL for fees + tip (0.001 fee + 0.0001 tip)
    let amount_to_send = amount_sol - fee_buffer;
    
    if amount_to_send <= 0.0 {
        return Err("Insufficient SOL to send after accounting for fees".to_string());
    }
    
    let lamports = (amount_to_send * 1_000_000_000.0) as u64;
    
    // Create transfer instruction
    let transfer_instruction = system_instruction::transfer(
        sender,
        recipient,
        lamports,
    );
    
    // Add revenue tip instruction (0.0001 SOL)
    let revenue_address = Pubkey::from_str("juLesoSmdTcRtzjCzYzRoHrnF8GhVu6KCV7uxq7nJGp")
        .map_err(|e| format!("Invalid revenue address: {}", e))?;
    let tip_instruction = system_instruction::transfer(
        sender,
        &revenue_address,
        100_000, // 0.0001 SOL in lamports
    );
    
    // Get recent blockhash
    let recent_blockhash = tx_client.get_recent_blockhash().await
        .map_err(|e| format!("Failed to get blockhash: {:?}", e))?;
    
    // Create transaction with both transfer and tip
    let message = Message::new(&[transfer_instruction, tip_instruction], Some(sender));
    let mut transaction = Transaction::new_unsigned(message);
    transaction.message.recent_blockhash = recent_blockhash;

    // Sign the transaction
    let message_bytes = bincode::serialize(&transaction.message)
        .map_err(|e| format!("Failed to serialize message: {}", e))?;

    let signature_bytes = signer.sign_message(&message_bytes).await
        .map_err(|e| format!("Failed to sign: {}", e))?;

    let mut sig_array = [0u8; 64];
    sig_array.copy_from_slice(&signature_bytes);
    transaction.signatures = vec![Signature::from(sig_array)];

    // Serialize and send
    let signed_tx_bytes = bincode::serialize(&transaction)
        .map_err(|e| format!("Failed to serialize transaction: {}", e))?;

    let signed_tx_b58 = bs58::encode(&signed_tx_bytes).into_string();

    // Send the transaction
    tx_client.send_transaction(&signed_tx_b58).await
        .map_err(|e| format!("{:?}", e))
}


#[derive(Debug, Clone, PartialEq)]
pub struct EjectTokenItem {
    pub token: Token,
    pub status: EjectTokenStatus,
}

/// Processing modal showing real-time EJECT progress
#[component]
fn EjectProcessingModal(
    tokens: Vec<EjectTokenItem>,
    current_step: String,
    is_complete: bool,
    total_sol_received: f64,
    final_signature: String,
    send_signature: Option<String>,
    oncancel: EventHandler<()>,
    onclose: EventHandler<()>,
) -> Element {
    let all_complete = tokens.iter().all(|item| item.status.is_complete());
    let show_success = is_complete && all_complete;

    rsx! {
        div {
            class: "modal-backdrop",
            style: "z-index: 1001;",

            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                style: "
                    background: #2C2C2C;
                    border-radius: 20px;
                    padding: 0;
                    width: min(600px, calc(100vw - 32px));
                    max-width: 600px;
                    max-height: calc(100vh - 64px);
                    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.8);
                    border: 1px solid rgba(255, 255, 255, 0.1);
                    overflow: hidden;
                    margin: 16px auto;
                    display: flex;
                    flex-direction: column;
                ",

                // Header
                div {
                    style: "
                        padding: 24px;
                        border-bottom: 1px solid rgba(255, 255, 255, 0.1);
                    ",
                    h2 {
                        style: "
                            color: #f8fafc;
                            font-size: 20px;
                            font-weight: 700;
                            margin: 0 0 8px 0;
                        ",
                        if show_success {
                            "EJECT Complete! 🚀"
                        } else {
                            "Processing EJECT..."
                        }
                    }
                    div {
                        style: "
                            color: #94a3b8;
                            font-size: 14px;
                        ",
                        if show_success {
                            "Successfully ejected {tokens.len()} tokens"
                        } else {
                            "{current_step}"
                        }
                    }
                }

                // Progress list
                div {
                    style: "
                        padding: 20px 24px;
                        overflow-y: auto;
                        flex: 1;
                        max-height: 400px;
                    ",
                    for item in tokens.iter() {
                        div {
                            key: "{item.token.mint}",
                            style: "
                                background: #1a1a1a;
                                border: 1.5px solid #4a4a4a;
                                border-radius: 10px;
                                padding: 12px 16px;
                                margin-bottom: 12px;
                                display: flex;
                                align-items: center;
                                gap: 12px;
                            ",

                            // Token icon
                            img {
                                src: "{item.token.icon_type}",
                                alt: "{item.token.symbol}",
                                style: "width: 32px; height: 32px; border-radius: 50%;"
                            }

                            // Token info and status
                            div {
                                style: "flex: 1;",
                                div {
                                    style: "
                                        color: #f8fafc;
                                        font-size: 14px;
                                        font-weight: 600;
                                        margin-bottom: 4px;
                                    ",
                                    "{item.token.symbol}"
                                }
                                div {
                                    style: "
                                        color: {item.status.status_color()};
                                        font-size: 13px;
                                    ",
                                    "{item.status.status_text()}"
                                }
                            }

                            // Status indicator
                            if !item.status.is_complete() {
                                div {
                                    style: "
                                        width: 20px;
                                        height: 20px;
                                        border: 2px solid #3b82f6;
                                        border-top-color: transparent;
                                        border-radius: 50%;
                                        animation: spin 1s linear infinite;
                                    ",
                                }
                            } else {
                                div {
                                    style: "
                                        font-size: 20px;
                                    ",
                                    if matches!(item.status, EjectTokenStatus::Complete { .. }) {
                                        "✅"
                                    } else {
                                        "❌"
                                    }
                                }
                            }
                        }
                    }
                }

                // Success summary (shown when complete)
                if show_success {
                    div {
                        style: "
                            padding: 20px 24px;
                            border-top: 1px solid rgba(255, 255, 255, 0.1);
                            background: #1a1a1a;
                        ",

                        // Success icon
                        div {
                            style: "
                                width: 60px;
                                height: 60px;
                                background: rgba(16, 185, 129, 0.1);
                                border: 2px solid #10b981;
                                border-radius: 50%;
                                margin: 0 auto 16px;
                                display: flex;
                                align-items: center;
                                justify-content: center;
                                font-size: 32px;
                            ",
                            "✓"
                        }

                        // Total SOL reclaimed
                        div {
                            style: "
                                background: rgba(16, 185, 129, 0.1);
                                border: 1px solid rgba(16, 185, 129, 0.3);
                                border-radius: 12px;
                                padding: 16px;
                                margin-bottom: 16px;
                                text-align: center;
                            ",
                            div {
                                style: "
                                    color: #94a3b8;
                                    font-size: 13px;
                                    margin-bottom: 6px;
                                ",
                                "Total SOL Reclaimed"
                            }
                            div {
                                style: "
                                    color: #10b981;
                                    font-size: 24px;
                                    font-weight: 700;
                                ",
                                "{total_sol_received:.6} SOL"
                            }
                        }

                        // Transaction signature
                        if !final_signature.is_empty() && final_signature != "EJECT_COMPLETED" {
                            div {
                                style: "margin-bottom: 16px;",
                                div {
                                    style: "
                                        color: #94a3b8;
                                        font-size: 12px;
                                        margin-bottom: 6px;
                                    ",
                                    "Transaction Signature:"
                                }
                                div {
                                    style: "
                                        background: #2a2a2a;
                                        border: 1px solid #4a4a4a;
                                        border-radius: 8px;
                                        padding: 10px 12px;
                                        color: #cbd5e1;
                                        font-size: 11px;
                                        word-break: break-all;
                                        font-family: monospace;
                                    ",
                                    "{final_signature}"
                                }
                            }

                // Explorer links
                div {
                    style: "display: flex; gap: 8px;",
                    a {
                        href: format!("https://solscan.io/tx/{}", final_signature),
                        target: "_blank",
                        style: "
                            flex: 1;
                            background: #3a3a3a;
                            color: #ffffff;
                            border: 1px solid #5a5a5a;
                            border-radius: 8px;
                            padding: 8px 12px;
                            text-align: center;
                            text-decoration: none;
                            font-size: 12px;
                            font-weight: 600;
                        ",
                        "Solscan"
                    }
                    a {
                        href: format!("https://orb.helius.dev/tx/{}?cluster=mainnet-beta&tab=summary", final_signature),
                        target: "_blank",
                        style: "
                            flex: 1;
                            background: #3a3a3a;
                            color: #ffffff;
                            border: 1px solid #5a5a5a;
                            border-radius: 8px;
                            padding: 8px 12px;
                            text-align: center;
                            text-decoration: none;
                            font-size: 12px;
                            font-weight: 600;
                        ",
                        "Orb"
                    }
                }
                        }
                    }
                }

                // Action buttons
                div {
                    style: "
                        padding: 20px 24px;
                        border-top: 1px solid rgba(255, 255, 255, 0.1);
                    ",
                    if show_success {
                        button {
                            style: "
                                width: 100%;
                                background: white;
                                color: #1a1a1a;
                                border: none;
                                border-radius: 12px;
                                padding: 14px 24px;
                                font-size: 15px;
                                font-weight: 700;
                                cursor: pointer;
                                box-shadow: 0 4px 12px rgba(255, 255, 255, 0.2);
                            ",
                            onclick: move |_| onclose.call(()),
                            "Close"
                        }
                    } else {
                        button {
                            style: "
                                width: 100%;
                                background: #3a3a3a;
                                color: #ffffff;
                                border: 1px solid #5a5a5a;
                                border-radius: 12px;
                                padding: 12px 24px;
                                font-size: 14px;
                                font-weight: 600;
                                cursor: pointer;
                            ",
                            onclick: move |_| oncancel.call(()),
                            "Cancel"
                        }
                    }
                }

                // Add CSS animation
                style {
                    "
                    @keyframes spin {{
                        0% {{ transform: rotate(0deg); }}
                        100% {{ transform: rotate(360deg); }}
                    }}
                    "
                }
            }
        }
    }
}

/// Hardware wallet approval overlay for eject operation
#[component]
fn EjectHardwareApprovalOverlay(selected_count: usize, oncancel: EventHandler<()>) -> Element {
    rsx! {
        div {
            class: "hardware-approval-overlay",

            div {
                class: "hardware-approval-content",

                h3 {
                    class: "hardware-approval-title",
                    "Confirm EJECT ({selected_count} tokens)"
                }

                div {
                    class: "hardware-icon-container",
                    div {
                        class: "hardware-icon",
                        div {
                            class: "blink-indicator",
                        }
                    }
                    div {
                        class: "button-indicator",
                        div {
                            class: "button-press",
                        }
                    }
                }

                p {
                    class: "hardware-approval-text",
                    "Please check your hardware wallet and confirm the EJECT transaction."
                }

                div {
                    class: "hardware-steps",
                    div {
                        class: "hardware-step",
                        div { class: "step-number", "1" }
                        span { "Review the transaction on your device" }
                    }
                    div {
                        class: "hardware-step",
                        div { class: "step-number", "2" }
                        span { "Press the button on your Unruggable to confirm" }
                    }
                }

                button {
                    class: "hardware-cancel-button",
                    onclick: move |_| oncancel.call(()),
                    "Cancel EJECT"
                }
            }
        }
    }
}

/// Success modal for EJECT operation
#[component]
pub fn EjectSuccessModal(
    signature: String,
    tokens_ejected: usize,
    total_sol_reclaimed: f64,
    was_hardware_wallet: bool,
    onclose: EventHandler<()>,
) -> Element {
    let solscan_url = format!("https://solscan.io/tx/{}", signature);
    let orb_url = format!("https://orb.helius.dev/tx/{}?cluster=mainnet-beta&tab=summary", signature);

    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| onclose.call(()),

            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                style: "
                    background: #2C2C2C;
                    border-radius: 20px;
                    padding: 0;
                    width: min(420px, calc(100vw - 32px));
                    max-width: 420px;
                    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.8);
                    border: 1px solid rgba(255, 255, 255, 0.1);
                    overflow: hidden;
                    margin: 16px auto;
                ",

                h2 {
                    style: "
                        color: #f8fafc;
                        font-size: 22px;
                        font-weight: 700;
                        margin: 0;
                        padding: 24px 24px 16px;
                        text-align: center;
                    ",
                    "EJECT Completed Successfully! 🚀"
                }

                div {
                    style: "padding: 0 24px 20px; text-align: center;",
                    div {
                        style: "
                            width: 80px;
                            height: 80px;
                            background: rgba(16, 185, 129, 0.1);
                            border: 2px solid #10b981;
                            border-radius: 50%;
                            margin: 0 auto 16px;
                            display: flex;
                            align-items: center;
                            justify-content: center;
                            font-size: 40px;
                        ",
                        "✓"
                    }

                    div {
                        style: "
                            background: #1a1a1a;
                            border: 1.5px solid #4a4a4a;
                            border-radius: 12px;
                            padding: 16px;
                            margin-bottom: 16px;
                        ",
                        div {
                            style: "
                                display: flex;
                                justify-content: space-between;
                                margin-bottom: 10px;
                            ",
                            span {
                                style: "color: #94a3b8; font-size: 14px;",
                                "Tokens Ejected:"
                            }
                            span {
                                style: "color: #cbd5e1; font-size: 14px; font-weight: 600;",
                                "{tokens_ejected}"
                            }
                        }
                        div {
                            style: "display: flex; justify-content: space-between;",
                            span {
                                style: "color: #94a3b8; font-size: 14px;",
                                "SOL Reclaimed:"
                            }
                            span {
                                style: "color: #10b981; font-size: 14px; font-weight: 600;",
                                "{total_sol_reclaimed:.6} SOL"
                            }
                        }
                    }

                    if was_hardware_wallet {
                        p {
                            style: "
                                color: #94a3b8;
                                font-size: 13px;
                                margin: 8px 0 0 0;
                                padding: 8px 12px;
                                background: rgba(255, 255, 255, 0.05);
                                border-radius: 8px;
                            ",
                            "Signed with hardware wallet"
                        }
                    }
                }

                div {
                    style: "padding: 0 24px 24px;",
                    div {
                        style: "margin-bottom: 20px;",
                        label {
                            style: "
                                color: #94a3b8;
                                font-size: 13px;
                                display: block;
                                margin-bottom: 8px;
                            ",
                            "Transaction Signature:"
                        }
                        div {
                            title: "Click to copy",
                            onclick: move |_| {
                                log::info!("Signature copied to clipboard: {}", signature);
                            },
                            style: "
                                background: #1a1a1a;
                                border: 1px solid #4a4a4a;
                                border-radius: 8px;
                                padding: 12px;
                                color: #cbd5e1;
                                font-size: 13px;
                                word-break: break-all;
                                cursor: pointer;
                                transition: all 0.2s ease;
                            ",
                            "{signature}"
                        }
                        div {
                            style: "
                                color: #94a3b8;
                                font-size: 12px;
                                margin-top: 6px;
                                text-align: center;
                            ",
                            "Click to copy"
                        }
                    }

                    div {
                        style: "margin-bottom: 20px;",
                        p {
                            style: "
                                color: #94a3b8;
                                font-size: 13px;
                                margin: 0 0 12px 0;
                            ",
                            "View transaction in explorer:"
                        }

                        div {
                            style: "display: flex; flex-direction: column; gap: 8px;",
                            a {
                                href: "{solscan_url}",
                                target: "_blank",
                                rel: "noopener noreferrer",
                                style: "
                                    background: #3a3a3a;
                                    color: #ffffff;
                                    border: 1px solid #5a5a5a;
                                    border-radius: 8px;
                                    padding: 10px 16px;
                                    text-align: center;
                                    text-decoration: none;
                                    font-size: 14px;
                                    font-weight: 600;
                                    transition: all 0.2s ease;
                                ",
                                "Solscan"
                            }
                            a {
                                href: "{orb_url}",
                                target: "_blank",
                                rel: "noopener noreferrer",
                                style: "
                                    background: #3a3a3a;
                                    color: #ffffff;
                                    border: 1px solid #5a5a5a;
                                    border-radius: 8px;
                                    padding: 10px 16px;
                                    text-align: center;
                                    text-decoration: none;
                                    font-size: 14px;
                                    font-weight: 600;
                                    transition: all 0.2s ease;
                                ",
                                "Orb"
                            }
                        }
                    }
                }

                div {
                    style: "padding: 0 24px 24px;",
                    button {
                        style: "
                            width: 100%;
                            background: white;
                            color: #1a1a1a;
                            border: none;
                            border-radius: 12px;
                            padding: 14px 24px;
                            font-size: 15px;
                            font-weight: 700;
                            cursor: pointer;
                            transition: all 0.2s ease;
                            box-shadow: 0 4px 12px rgba(255, 255, 255, 0.2);
                        ",
                        onclick: move |_| onclose.call(()),
                        "Close"
                    }
                }
            }
        }
    }
}

#[component]
pub fn EjectModal(
    selected_token_mints: HashSet<String>,
    all_tokens: Vec<Token>,
    wallet: Option<WalletInfo>,
    hardware_wallet: Option<Arc<HardwareWallet>>,
    current_balance: f64, // SOL balance for fees
    custom_rpc: Option<String>,
    onclose: EventHandler<()>,
    onsuccess: EventHandler<String>,
) -> Element {
    println!("🚀 EjectModal component rendered!");

    // State management
    let mut ejecting = use_signal(|| false);
    let mut error_message = use_signal(|| None as Option<String>);
    let mut eject_items = use_signal(|| Vec::<EjectTokenItem>::new());
    let mut current_step_text = use_signal(|| "Preparing...".to_string());
    let mut send_sol_enabled = use_signal(|| false);
    let mut recipient = use_signal(|| "".to_string());
    let mut resolved_recipient = use_signal(|| Option::<Pubkey>::None);
    let mut show_processing_modal = use_signal(|| false);
    let mut processing_complete = use_signal(|| false);

    // Transaction state
    let mut transaction_signature = use_signal(|| "".to_string());
    let mut show_hardware_approval = use_signal(|| false);
    let mut total_sol_received = use_signal(|| 0.0);
    let mut estimated_swap_value = use_signal(|| 0.0);

    // Filter tokens to only selected ones
    let selected_tokens = use_memo(move || {
        all_tokens.iter()
            .filter(|token| selected_token_mints.contains(&token.mint))
            .cloned()
            .collect::<Vec<Token>>()
    });

    // Initialize eject items on first render
    use_effect(move || {
        if eject_items().is_empty() && !selected_tokens().is_empty() {
            let items: Vec<EjectTokenItem> = selected_tokens()
                .iter()
                .map(|token| EjectTokenItem {
                    token: token.clone(),
                    status: EjectTokenStatus::Pending,
                })
                .collect();
            eject_items.set(items);
        }
    });

    // Estimate total rent to be reclaimed (0.00203928 SOL per token account)
    let estimated_rent_reclaim = use_memo(move || {
        selected_tokens().len() as f64 * 0.00203928
    });

    // Estimate fees
    let estimated_fees = use_memo(move || {
        // Base fee per transaction * number of tokens
        0.000005 * selected_tokens().len() as f64
    });

    // Check if we have sufficient SOL for fees
    let sufficient_sol_for_fees = use_memo(move || {
        current_balance >= estimated_fees()
    });

    // Return processing/success modal if EJECT is running or complete
    if show_processing_modal() {
        return rsx! {
            EjectProcessingModal {
                tokens: eject_items(),
                current_step: current_step_text(),
                is_complete: processing_complete(),
                total_sol_received: total_sol_received(),
                final_signature: transaction_signature(),
                send_signature: None,
                oncancel: move |_| {
                    show_processing_modal.set(false);
                    ejecting.set(false);
                },
                onclose: move |_| {
                    show_processing_modal.set(false);
                    processing_complete.set(false);
                    onsuccess.call(transaction_signature());
                }
            }
        };
    }

    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| onclose.call(()),

            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                style: "
                    background: #2C2C2C;
                    border-radius: 20px;
                    padding: 0;
                    width: min(520px, calc(100vw - 32px));
                    max-width: 520px;
                    max-height: calc(100vh - 64px);
                    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.8);
                    border: 1px solid rgba(255, 255, 255, 0.1);
                    overflow: hidden;
                    margin: 16px auto;
                    display: flex;
                    flex-direction: column;
                    position: relative;
                ",

                // Hardware approval overlay
                if show_hardware_approval() {
                    EjectHardwareApprovalOverlay {
                        selected_count: selected_tokens().len(),
                        oncancel: move |_| {
                            show_hardware_approval.set(false);
                            ejecting.set(false);
                        }
                    }
                }

                // Modal header
                div {
                    style: "
                        display: flex;
                        justify-content: space-between;
                        align-items: center;
                        padding: 24px;
                        border-bottom: 1px solid rgba(255, 255, 255, 0.1);
                    ",
                    h2 {
                        style: "
                            color: #f8fafc;
                            font-size: 22px;
                            font-weight: 700;
                            margin: 0;
                            letter-spacing: -0.025em;
                        ",
                        "EJECT Tokens 🚀"
                    }
                    button {
                        style: "
                            background: none;
                            border: none;
                            color: white;
                            font-size: 28px;
                            cursor: pointer;
                            padding: 0;
                            min-width: 32px;
                            min-height: 32px;
                            display: flex;
                            align-items: center;
                            justify-content: center;
                        ",
                        onclick: move |_| onclose.call(()),
                        "×"
                    }
                }

                // Show error if any
                if let Some(error) = error_message() {
                    div {
                        style: "
                            padding: 12px 16px;
                            background-color: rgba(220, 38, 38, 0.1);
                            border: 1px solid #dc2626;
                            color: #fca5a5;
                            border-radius: 10px;
                            margin: 16px 24px;
                            font-size: 13px;
                            text-align: center;
                        ",
                        "{error}"
                    }
                }

                // Main content
                div {
                    style: "
                        padding: 20px 24px;
                        overflow-y: auto;
                        flex: 1;
                    ",

                    // Info card
                    div {
                        style: "
                            background: rgba(59, 130, 246, 0.1);
                            border: 1px solid rgba(59, 130, 246, 0.2);
                            border-radius: 12px;
                            padding: 16px;
                            margin-bottom: 20px;
                        ",
                        div {
                            style: "color: #93c5fd; font-size: 14px; font-weight: 600; margin-bottom: 8px;",
                            "What is EJECT?"
                        }
                        div {
                            style: "color: #cbd5e1; font-size: 13px; line-height: 1.5;",
                            "EJECT will attempt to swap your selected tokens to SOL, close token accounts to reclaim rent, and optionally send the SOL to another wallet."
                        }
                    }

                    // Summary card
                    div {
                        style: "
                            background: #1a1a1a;
                            border: 1.5px solid #4a4a4a;
                            border-radius: 12px;
                            padding: 16px;
                            margin-bottom: 20px;
                        ",
                        div {
                            style: "
                                display: flex;
                                justify-content: space-between;
                                margin-bottom: 12px;
                            ",
                            span {
                                style: "color: #94a3b8; font-size: 14px;",
                                "Tokens to eject:"
                            }
                            span {
                                style: "color: #cbd5e1; font-size: 14px; font-weight: 600;",
                                "{selected_tokens().len()}"
                            }
                        }
                        div {
                            style: "
                                display: flex;
                                justify-content: space-between;
                                margin-bottom: 12px;
                            ",
                            span {
                                style: "color: #94a3b8; font-size: 14px;",
                                "Estimated rent reclaim:"
                            }
                            span {
                                style: "color: #10b981; font-size: 14px; font-weight: 600;",
                                "{estimated_rent_reclaim():.6} SOL"
                            }
                        }
                        div {
                            style: "
                                display: flex;
                                justify-content: space-between;
                                margin-bottom: 12px;
                            ",
                            span {
                                style: "color: #94a3b8; font-size: 14px;",
                                "Estimated fees:"
                            }
                            span {
                                style: "color: #cbd5e1; font-size: 14px;",
                                "~{estimated_fees():.6} SOL"
                            }
                        }

                        if !sufficient_sol_for_fees() {
                            div {
                                style: "
                                    background: rgba(220, 38, 38, 0.1);
                                    border: 1px solid #dc2626;
                                    color: #fca5a5;
                                    border-radius: 8px;
                                    padding: 10px 12px;
                                    margin-top: 12px;
                                    font-size: 13px;
                                ",
                                "⚠️ Insufficient SOL for transaction fees"
                            }
                        }
                    }

                    // Token list
                    div {
                        style: "
                            background: #1a1a1a;
                            border: 1.5px solid #4a4a4a;
                            border-radius: 12px;
                            padding: 16px;
                            margin-bottom: 20px;
                        ",
                        div {
                            style: "
                                color: #f8fafc;
                                font-size: 15px;
                                font-weight: 700;
                                margin-bottom: 12px;
                            ",
                            "Selected Tokens"
                        }

                        div {
                            style: "display: flex; flex-direction: column; gap: 10px;",
                            for token in selected_tokens().iter() {
                                div {
                                    style: "
                                        display: flex;
                                        align-items: center;
                                        gap: 12px;
                                        padding: 10px 12px;
                                        background: #2a2a2a;
                                        border-radius: 8px;
                                    ",
                                    img {
                                        src: "{token.icon_type}",
                                        alt: "{token.symbol}",
                                        style: "width: 32px; height: 32px; border-radius: 50%;"
                                    }
                                    div {
                                        style: "flex: 1;",
                                        div {
                                            style: "
                                                color: #f8fafc;
                                                font-size: 14px;
                                                font-weight: 600;
                                            ",
                                            "{token.symbol}"
                                        }
                                        div {
                                            style: "
                                                color: #94a3b8;
                                                font-size: 12px;
                                            ",
                                            "Balance: {token.balance:.6}"
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Optional: Send SOL after eject
                    div {
                        style: "
                            background: #1a1a1a;
                            border: 1.5px solid #4a4a4a;
                            border-radius: 12px;
                            padding: 16px;
                        ",
                        div {
                            style: "
                                display: flex;
                                align-items: center;
                                gap: 12px;
                                margin-bottom: 12px;
                            ",
                            input {
                                r#type: "checkbox",
                                checked: send_sol_enabled(),
                                onchange: move |e| send_sol_enabled.set(e.checked()),
                                style: "
                                    width: 18px;
                                    height: 18px;
                                    cursor: pointer;
                                "
                            }
                            label {
                                style: "
                                    color: #f8fafc;
                                    font-size: 14px;
                                    font-weight: 600;
                                    cursor: pointer;
                                ",
                                onclick: move |_| send_sol_enabled.set(!send_sol_enabled()),
                                "Send resulting SOL to another wallet"
                            }
                        }

                        if send_sol_enabled() {
                            div {
                                style: "margin-top: 12px;",
                                label {
                                    style: "
                                        color: #94a3b8;
                                        font-size: 14px;
                                        display: block;
                                        margin-bottom: 8px;
                                    ",
                                    "Recipient Address (optional)"
                                }
                                AddressInput {
                                    value: recipient(),
                                    on_change: move |val| {
                                        recipient.set(val);
                                        error_message.set(None);
                                    },
                                    on_resolved: move |pubkey_opt| resolved_recipient.set(pubkey_opt),
                                    placeholder: Some("Enter Solana address or .sol domain".to_string()),
                                }
                            }
                        }
                    }
                }

                // Action buttons
                div {
                    style: "
                        display: flex;
                        gap: 12px;
                        padding: 20px 24px 24px;
                        border-top: 1px solid rgba(255, 255, 255, 0.1);
                    ",
                    button {
                        style: "
                            flex: 1;
                            background: #3a3a3a;
                            color: #ffffff;
                            border: 1px solid #5a5a5a;
                            border-radius: 12px;
                            padding: 14px 24px;
                            font-size: 15px;
                            font-weight: 700;
                            cursor: pointer;
                            transition: all 0.2s ease;
                        ",
                        onclick: move |_| onclose.call(()),
                        "Cancel"
                    }
                    button {
                        style: "
                            flex: 1;
                            background: white;
                            color: #1a1a1a;
                            border: none;
                            border-radius: 12px;
                            padding: 14px 24px;
                            font-size: 15px;
                            font-weight: 700;
                            cursor: pointer;
                            transition: all 0.2s ease;
                            text-transform: uppercase;
                            letter-spacing: 0.5px;
                            box-shadow: 0 4px 12px rgba(255, 255, 255, 0.2);
                        ",
                        disabled: ejecting() || !sufficient_sol_for_fees() || (send_sol_enabled() && resolved_recipient().is_none()),
                        onclick: move |_| {
                            println!("🚀 EJECT button clicked - Starting EJECT process!");

                            ejecting.set(true);
                            error_message.set(None);

                            // Use the already-memoized selected_tokens (from use_memo above)
                            let tokens_to_eject = selected_tokens();

                            if tokens_to_eject.is_empty() {
                                error_message.set(Some("No tokens selected".to_string()));
                                ejecting.set(false);
                                return;
                            }

                            // Initialize eject items with Pending status
                            let mut initial_items: Vec<EjectTokenItem> = tokens_to_eject
                                .iter()
                                .map(|token| EjectTokenItem {
                                    token: token.clone(),
                                    status: EjectTokenStatus::Pending,
                                })
                                .collect();
                            
                            // Add Send SOL item if enabled
                            if send_sol_enabled() && resolved_recipient().is_some() {
                                let send_token = Token {
                                    mint: "SEND_SOL".to_string(),
                                    symbol: "📤 Send SOL".to_string(),
                                    name: "Send SOL to recipient".to_string(),
                                    balance: 0.0,
                                    decimals: 9,
                                    value_usd: 0.0,
                                    price: 0.0,
                                    price_change: 0.0,
                                    price_change_1d: 0.0,
                                    price_change_3d: 0.0,
                                    price_change_7d: 0.0,
                                    icon_type: "data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' width='32' height='32' viewBox='0 0 32 32'><rect width='32' height='32' rx='16' fill='%234ade80'/><text x='16' y='22' text-anchor='middle' fill='white' font-family='Arial' font-size='20'>📤</text></svg>".to_string(),
                                };
                                initial_items.push(EjectTokenItem {
                                    token: send_token,
                                    status: EjectTokenStatus::Pending,
                                });
                            }
                            
                            eject_items.set(initial_items);

                            // Show processing modal
                            show_processing_modal.set(true);
                            current_step_text.set("Starting EJECT process...".to_string());

                            let wallet_clone = wallet.clone();
                            let hw_clone = hardware_wallet.clone();
                            let rpc_clone = custom_rpc.clone();
                            let recipient_clone = resolved_recipient();
                            let has_hw = hardware_wallet.is_some();

                            // Execute EJECT asynchronously with status callback
                            spawn(async move {
                                println!("🚀 Executing EJECT for {} tokens...", tokens_to_eject.len());

                                let status_callback = move |index: usize, status: EjectTokenStatus| {
                                    // Update the specific token's status
                                    let mut items = eject_items();
                                    if index < items.len() {
                                        items[index].status = status.clone();
                                        eject_items.set(items);

                                        // Update current step text
                                        let step_text = match &status {
                                            EjectTokenStatus::FetchingQuote => format!("Fetching swap quote for token {}...", index + 1),
                                            EjectTokenStatus::SwappingToSol => format!("Swapping token {} to SOL...", index + 1),
                                            EjectTokenStatus::SwapSuccess { sol_received } => format!("Swap successful! Received {:.4} SOL", sol_received),
                                            EjectTokenStatus::ClosingAccount => format!("Closing token account {}...", index + 1),
                                            EjectTokenStatus::Complete { .. } => format!("Token {} complete!", index + 1),
                                            EjectTokenStatus::Failed { reason } => format!("Token {} failed: {}", index + 1, reason),
                                            _ => format!("Processing token {}...", index + 1),
                                        };
                                        current_step_text.set(step_text);
                                    }
                                };

                                match execute_eject(
                                    tokens_to_eject,
                                    wallet_clone,
                                    hw_clone,
                                    rpc_clone,
                                    recipient_clone,
                                    send_sol_enabled() && resolved_recipient().is_some(),
                                    status_callback,
                                ).await {
                                    Ok((signature, total_sol, send_sig)) => {
                                        println!("✅ EJECT completed: {}", signature);
                                        transaction_signature.set(signature);
                                        total_sol_received.set(total_sol);
                                        current_step_text.set("All tokens ejected successfully!".to_string());

                                        // Mark as complete - modal will transform to success view
                                        processing_complete.set(true);
                                        ejecting.set(false);
                                    }
                                    Err(e) => {
                                        println!("❌ EJECT failed: {}", e);
                                        error_message.set(Some(format!("EJECT failed: {}", e)));
                                        show_processing_modal.set(false);
                                        ejecting.set(false);
                                    }
                                }
                            });
                        },
                        if ejecting() { "EJECTING..." } else { "EJECT" }
                    }
                }
            }
        }
    }
}
