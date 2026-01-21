use dioxus::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use crate::config::tokens::get_token_catalog;
use crate::wallet::WalletInfo;
use crate::hardware::HardwareWallet;
use crate::transaction::TransactionClient;
use crate::components::common::Token;
use crate::signing::hardware::HardwareSigner;
use crate::signing::software::SoftwareSigner;
use crate::signing::TransactionSigner;
use crate::wallet::Wallet;
use crate::prices;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

// Channel message types for iOS-safe signal updates
#[derive(Clone, Debug)]
enum SwapResult {
    Success(String), // transaction signature
    Error(String),   // error message
}

#[derive(Clone, Debug)]
enum SwapUpdate {
    Started,
    HardwareApprovalRequired(bool),
    Result(SwapResult),
}
use solana_sdk::{
    transaction::VersionedTransaction,
    pubkey::Pubkey as SolanaPubkey,
    hash::Hash as SolanaHash,
    instruction::Instruction as SolanaInstruction,
    instruction::AccountMeta as SolanaAccountMeta,
    message::{v0, VersionedMessage, AddressLookupTableAccount},
};
use solana_system_interface::instruction as system_instruction;
use crate::titan::{TitanClient, build_transaction_from_route};
use crate::titan::SwapRoute as TitanSwapRoute;
use crate::timeout;
use std::str::FromStr;

const ICON_SWITCH: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/icons/SWITCH.svg";

// Jules tip address for monetization (0.0001 SOL per swap)
const JULES_TIP_ADDRESS: &str = "juLesoSmdTcRtzjCzYzRoHrnF8GhVu6KCV7uxq7nJGp";
const JULES_TIP_LAMPORTS: u64 = 100_000; //  0.0001 SOL

/// Convert SwapInstruction to Solana Instruction
fn swap_instruction_to_solana(swap_ix: &SwapInstruction) -> Result<SolanaInstruction, String> {
    let program_id = SolanaPubkey::from_str(&swap_ix.program_id)
        .map_err(|e| format!("Invalid program ID: {}", e))?;
    
    let accounts: Result<Vec<SolanaAccountMeta>, String> = swap_ix.accounts
        .iter()
        .map(|acc| {
            let pubkey = SolanaPubkey::from_str(&acc.pubkey)
                .map_err(|e| format!("Invalid account pubkey: {}", e))?;
            Ok(SolanaAccountMeta {
                pubkey,
                is_signer: acc.is_signer,
                is_writable: acc.is_writable,
            })
        })
        .collect();
    
    // Try base64 first (Dflow uses base64), fall back to base58 (Jupiter might use base58)
    let data = if let Ok(decoded) = base64::decode(&swap_ix.data) {
        decoded
    } else {
        bs58::decode(&swap_ix.data)
            .into_vec()
            .map_err(|e| format!("Invalid instruction data (neither base64 nor base58): {}", e))?
    };
    
    Ok(SolanaInstruction {
        program_id,
        accounts: accounts?,
        data,
    })
}

/// Fetch address lookup table accounts from RPC
async fn fetch_lookup_tables(
    lookup_table_addresses: &[String],
    rpc_url: &str,
) -> Result<Vec<AddressLookupTableAccount>, String> {
    let client = reqwest::Client::new();
    let mut lookup_tables = Vec::new();
    
    for address_str in lookup_table_addresses {
        let pubkey = SolanaPubkey::from_str(address_str)
            .map_err(|e| format!("Invalid lookup table address: {}", e))?;
        
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getAccountInfo",
            "params": [
                address_str,
                {
                    "encoding": "base64"
                }
            ]
        });
        
        let response = client
            .post(rpc_url)
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch lookup table: {}", e))?;
        
        let json: serde_json::Value = response.json().await
            .map_err(|e| format!("Failed to parse lookup table response: {}", e))?;
        
        if let Some(data_array) = json["result"]["value"]["data"].as_array() {
            if let Some(data_str) = data_array.get(0).and_then(|v| v.as_str()) {
                let data = base64::decode(data_str)
                    .map_err(|e| format!("Failed to decode lookup table data: {}", e))?;
                
                // Parse lookup table addresses (skip meta, each address is 32 bytes)
                const META_SIZE: usize = 56;
                if data.len() < META_SIZE {
                    continue;
                }
                
                let addresses_data = &data[META_SIZE..];
                let num_addresses = addresses_data.len() / 32;
                
                let mut addresses = Vec::with_capacity(num_addresses);
                for i in 0..num_addresses {
                    let start = i * 32;
                    let end = start + 32;
                    let address_bytes: [u8; 32] = addresses_data[start..end]
                        .try_into()
                        .map_err(|_| "Invalid address bytes".to_string())?;
                    addresses.push(SolanaPubkey::new_from_array(address_bytes));
                }
                
                lookup_tables.push(AddressLookupTableAccount {
                    key: pubkey,
                    addresses,
                });
            }
        }
    }
    
    Ok(lookup_tables)
}

/// Build transaction from swap instructions and add Jules tip (unless hardware wallet)
async fn build_transaction_from_instructions(
    compute_budget_ixs: Vec<SwapInstruction>,
    setup_ixs: Vec<SwapInstruction>,
    swap_ix: SwapInstruction,
    cleanup_ixs: Vec<SwapInstruction>,
    other_ixs: Vec<SwapInstruction>,
    lookup_table_addresses: Vec<String>,
    payer: SolanaPubkey,
    rpc_url: &str,
    is_hardware_wallet: bool,
) -> Result<Vec<u8>, String> {
    println!("🔧 Building transaction from swap instructions");
    
    // Get current blockhash and slot
    let tx_client = TransactionClient::new(Some(rpc_url));
    let recent_blockhash = tx_client.get_recent_blockhash().await
        .map_err(|e| format!("Failed to get blockhash: {}", e))?;
    let current_slot = tx_client.get_current_slot().await
        .map_err(|e| format!("Failed to get current slot: {}", e))?;
    
    // Build timeout instruction (FIRST)
    let timeout_ix = timeout::build_timeout_instruction_from_current(
        current_slot,
        timeout::DEFAULT_SLOT_WINDOW,
    )?;
    
    // Convert all instructions to Solana instructions
    let mut all_instructions = vec![timeout_ix];
    
    // Add compute budget instructions
    for ix in compute_budget_ixs {
        all_instructions.push(swap_instruction_to_solana(&ix)?);
    }
    
    // Add other instructions
    for ix in other_ixs {
        all_instructions.push(swap_instruction_to_solana(&ix)?);
    }
    
    // Add setup instructions
    for ix in setup_ixs {
        all_instructions.push(swap_instruction_to_solana(&ix)?);
    }
    
    // Add swap instruction
    all_instructions.push(swap_instruction_to_solana(&swap_ix)?);
    
    // Add cleanup instructions
    for ix in cleanup_ixs {
        all_instructions.push(swap_instruction_to_solana(&ix)?);
    }
    
    // Add Jules tip instruction (LAST) - skip for hardware wallets
    if !is_hardware_wallet {
        let jules_tip_address = SolanaPubkey::from_str(JULES_TIP_ADDRESS)
            .map_err(|e| format!("Invalid Jules tip address: {}", e))?;
        let tip_ix = system_instruction::transfer(&payer, &jules_tip_address, JULES_TIP_LAMPORTS);
        all_instructions.push(tip_ix);
        
        println!("   Added Jules tip (0.0001 SOL) to swap transaction");
    } else {
        println!("   Hardware wallet detected - skipping Jules tip");
    }
    
    println!("   Total instructions: {}", all_instructions.len());
    
    // Fetch lookup tables if any
    let lookup_tables = if !lookup_table_addresses.is_empty() {
        fetch_lookup_tables(&lookup_table_addresses, rpc_url).await?
    } else {
        Vec::new()
    };
    
    // Build V0 message with lookup tables
    let message = v0::Message::try_compile(
        &payer,
        &all_instructions,
        &lookup_tables,
        recent_blockhash,
    ).map_err(|e| format!("Failed to compile message: {}", e))?;
    
    // Create versioned transaction
    let transaction = VersionedTransaction {
        signatures: vec![solana_sdk::signature::Signature::default()],
        message: VersionedMessage::V0(message),
    };
    
    // Serialize to bytes
    let serialized = bincode::serialize(&transaction)
        .map_err(|e| format!("Failed to serialize transaction: {}", e))?;
    
    println!("   Transaction built: {} bytes", serialized.len());
    
    Ok(serialized)
}

/// Sign a transaction using the provided signer (works for Jupiter, Dflow, and any instruction-based swap)
async fn sign_jupiter_transaction(
    signer: &dyn TransactionSigner,
    unsigned_transaction_b64: &str,
) -> Result<String, String> {
    println!("🔐 Signing transaction...");
    println!("🔍 Signer type: {}", signer.get_name());
    
    // Decode the base64 unsigned transaction
    let unsigned_tx_bytes = match base64::decode(unsigned_transaction_b64) {
        Ok(bytes) => bytes,
        Err(e) => return Err(format!("Failed to decode base64 transaction: {}", e)),
    };
    
    println!("📄 Decoded transaction: {} bytes", unsigned_tx_bytes.len());
    
    // Deserialize the transaction
    let mut transaction: VersionedTransaction = match bincode::deserialize(&unsigned_tx_bytes) {
        Ok(tx) => tx,
        Err(e) => return Err(format!("Failed to deserialize transaction: {}", e)),
    };
    
    // Log transaction type for debugging
    match &transaction.message {
        solana_sdk::message::VersionedMessage::Legacy(_) => {
            println!("📋 Transaction type: Legacy");
        }
        solana_sdk::message::VersionedMessage::V0(_) => {
            println!("📋 Transaction type: V0 (with lookup tables)");
        }
    }
    
    println!("📋 Transaction has {} signatures expected", transaction.signatures.len());
    
    // Serialize the transaction message for signing
    let message_bytes = transaction.message.serialize();
    println!("📝 Message to sign: {} bytes", message_bytes.len());
    println!("🔍 Message bytes (first 32): {:02x?}", &message_bytes[..message_bytes.len().min(32)]);
    
    // Sign the message
    println!("⏳ Waiting for wallet signature...");
    let signature_bytes = match signer.sign_message(&message_bytes).await {
        Ok(sig) => {
            println!("✅ Wallet returned signature: {} bytes", sig.len());
            sig
        }
        Err(e) => {
            println!("❌ Wallet signing failed: {}", e);
            return Err(format!("Failed to sign message: {}", e));
        }
    };
    
    // Ensure we have exactly 64 bytes for the signature
    if signature_bytes.len() != 64 {
        println!("❌ Invalid signature length from wallet");
        return Err(format!("Invalid signature length: expected 64, got {}", signature_bytes.len()));
    }
    
    println!("🔍 Signature bytes (first 32): {:02x?}", &signature_bytes[..32]);
    
    // Convert to Solana signature
    let mut sig_array = [0u8; 64];
    sig_array.copy_from_slice(&signature_bytes);
    let solana_signature = solana_sdk::signature::Signature::from(sig_array);
    
    // Replace the first signature (assumes single signer)
    if transaction.signatures.is_empty() {
        return Err("Transaction has no signature slots".to_string());
    }
    transaction.signatures[0] = solana_signature;
    
    println!("✍️ Applied signature to transaction");
    
    // Serialize the signed transaction
    let signed_tx_bytes = match bincode::serialize(&transaction) {
        Ok(bytes) => bytes,
        Err(e) => return Err(format!("Failed to serialize signed transaction: {}", e)),
    };
    
    // Encode back to base64
    let signed_transaction_b64 = base64::encode(&signed_tx_bytes);
    
    println!("🎯 Signed transaction: {} bytes -> {} chars base64", signed_tx_bytes.len(), signed_transaction_b64.len());
    
    Ok(signed_transaction_b64)
}

// Jupiter Ultra API Types (simple order + execute flow)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JupiterUltraOrderResponse {
    pub mode: String,
    #[serde(rename = "inputMint")]
    pub input_mint: String,
    #[serde(rename = "outputMint")]
    pub output_mint: String,
    #[serde(rename = "inAmount")]
    pub in_amount: String,
    #[serde(rename = "outAmount")]
    pub out_amount: String,
    #[serde(rename = "otherAmountThreshold")]
    pub other_amount_threshold: String,
    #[serde(rename = "swapMode")]
    pub swap_mode: String,
    #[serde(rename = "slippageBps")]
    pub slippage_bps: u16,
    #[serde(rename = "priceImpact")]
    pub price_impact: Option<f64>,
    #[serde(rename = "routePlan")]
    pub route_plan: Vec<serde_json::Value>,
    #[serde(rename = "feeBps")]
    pub fee_bps: u16,
    #[serde(rename = "prioritizationFeeLamports")]
    pub prioritization_fee_lamports: u64,
    pub router: String,
    pub transaction: Option<String>, // base64 encoded unsigned transaction
    pub gasless: bool,
    #[serde(rename = "requestId")]
    pub request_id: String,
    pub taker: Option<String>,
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JupiterUltraExecuteRequest {
    #[serde(rename = "signedTransaction")]
    pub signed_transaction: String,
    #[serde(rename = "requestId")]
    pub request_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JupiterUltraExecuteResponse {
    pub status: String, // "Success" or "Failed"
    pub signature: Option<String>,
    pub error: Option<String>,
}

// Instruction-based API Types (for Dflow only now)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstructionAccount {
    pub pubkey: String,
    #[serde(rename = "isSigner")]
    pub is_signer: bool,
    #[serde(rename = "isWritable")]
    pub is_writable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapInstruction {
    #[serde(rename = "programId")]
    pub program_id: String,
    pub accounts: Vec<InstructionAccount>,
    pub data: String, // base58 or base64 encoded instruction data
}

// Dflow API Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DflowQuoteResponse {
    #[serde(rename = "inputMint")]
    pub input_mint: String,
    #[serde(rename = "outputMint")]
    pub output_mint: String,
    #[serde(rename = "inAmount")]
    pub in_amount: String,
    #[serde(rename = "outAmount")]
    pub out_amount: String,
    #[serde(rename = "otherAmountThreshold")]
    pub other_amount_threshold: String,
    #[serde(rename = "minOutAmount")]
    pub min_out_amount: String,
    #[serde(rename = "slippageBps")]
    pub slippage_bps: u16,
    #[serde(rename = "platformFee")]
    pub platform_fee: Option<serde_json::Value>,
    #[serde(rename = "outTransferFee")]
    pub out_transfer_fee: Option<serde_json::Value>,
    #[serde(rename = "priceImpactPct")]
    pub price_impact_pct: String,
    #[serde(rename = "routePlan")]
    pub route_plan: Vec<serde_json::Value>,
    #[serde(rename = "contextSlot")]
    pub context_slot: u64,
    #[serde(rename = "simulatedComputeUnits")]
    pub simulated_compute_units: u64,
    #[serde(rename = "requestId")]
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DflowSwapInstructionsRequest {
    #[serde(rename = "userPublicKey")]
    pub user_public_key: String,
    #[serde(rename = "wrapAndUnwrapSol")]
    pub wrap_and_unwrap_sol: bool,
    #[serde(rename = "dynamicComputeUnitLimit")]
    pub dynamic_compute_unit_limit: bool,
    #[serde(rename = "prioritizationFeeLamports")]
    pub prioritization_fee_lamports: serde_json::Value,
    #[serde(rename = "quoteResponse")]
    pub quote_response: DflowQuoteResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DflowSwapInstructionsResponse {
    #[serde(rename = "computeBudgetInstructions")]
    pub compute_budget_instructions: Vec<SwapInstruction>,
    #[serde(rename = "setupInstructions")]
    pub setup_instructions: Vec<SwapInstruction>,
    #[serde(rename = "swapInstruction")]
    pub swap_instruction: SwapInstruction,
    #[serde(rename = "cleanupInstructions")]
    pub cleanup_instructions: Vec<SwapInstruction>,
    #[serde(rename = "otherInstructions")]
    pub other_instructions: Vec<SwapInstruction>,
    #[serde(rename = "addressLookupTableAddresses")]
    pub address_lookup_table_addresses: Vec<String>,
    #[serde(rename = "computeUnitLimit")]
    pub compute_unit_limit: u64,
    #[serde(rename = "prioritizationFeeLamports")]
    pub prioritization_fee_lamports: u64,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
struct JupiterTokenMeta {
    pub address: String,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
    #[serde(rename = "logoURI")]
    pub logo_uri: Option<String>,
}

// Get token mint address from actual token data
fn get_token_mint<'a>(symbol: &str, tokens: &'a [Token]) -> &'a str {
    tokens.iter()
        .find(|t| t.symbol == symbol)
        .map(|t| t.mint.as_str())
        .unwrap_or("So11111111111111111111111111111111111111112") // Default to SOL if not found
}

fn get_token_mint_with_meta(symbol: &str, tokens: &[Token], meta: Option<&JupiterTokenMeta>) -> String {
    if let Some(meta) = meta {
        return meta.address.clone();
    }
    get_token_mint(symbol, tokens).to_string()
}

// Get token decimals from tokens vector
fn get_token_decimals(symbol: &str, tokens: &[Token]) -> u8 {
    // First try to find the token in the tokens array
    if let Some(token) = tokens.iter().find(|t| t.symbol == symbol) {
        return token.decimals;
    }

    // Fallback to known token decimals if not found in array
    match symbol {
        "USDC" | "USDT" => 6,  // Stablecoins use 6 decimals
        "SOL" => 9,             // SOL uses 9 decimals
        _ => 9,                 // Default to 9 decimals for unknown tokens
    }
}

fn get_token_decimals_with_meta(symbol: &str, tokens: &[Token], meta: Option<&JupiterTokenMeta>) -> u8 {
    if let Some(meta) = meta {
        return meta.decimals;
    }
    get_token_decimals(symbol, tokens)
}

// Convert human-readable amount to lamports/smallest unit
fn to_lamports(amount: f64, symbol: &str, tokens: &[Token]) -> u64 {
    let decimals = get_token_decimals(symbol, tokens);
    (amount * 10_f64.powi(decimals as i32)) as u64
}

// Convert lamports/smallest unit to human-readable amount  
fn from_lamports(lamports: u64, symbol: &str, tokens: &[Token]) -> f64 {
    let decimals = get_token_decimals(symbol, tokens);
    lamports as f64 / 10_f64.powi(decimals as i32)
}

fn from_lamports_with_meta(lamports: u64, symbol: &str, tokens: &[Token], meta: Option<&JupiterTokenMeta>) -> f64 {
    let decimals = get_token_decimals_with_meta(symbol, tokens, meta);
    lamports as f64 / 10_f64.powi(decimals as i32)
}

// Token icons
// Default fallback icon for tokens without specific icons
const ICON_32: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/solana-mobile@main/assets/icons/32x32.png";

// Get token icon from actual token data
fn get_token_icon<'a>(symbol: &str, tokens: &'a [Token]) -> &'a str {
    tokens.iter()
        .find(|t| t.symbol == symbol)
        .map(|t| t.icon_type.as_str())
        .unwrap_or(ICON_32)
}

fn get_token_icon_with_meta(symbol: &str, tokens: &[Token], meta: Option<&JupiterTokenMeta>) -> String {
    if let Some(meta) = meta {
        if let Some(url) = &meta.logo_uri {
            if !url.is_empty() {
                return url.clone();
            }
        }
    }
    get_token_icon(symbol, tokens).to_string()
}

fn short_mint(mint: &str) -> String {
    if mint.len() <= 8 {
        return mint.to_string();
    }
    format!("{}...{}", &mint[..4], &mint[mint.len() - 4..])
}

fn is_valid_mint(input: &str) -> bool {
    SolanaPubkey::from_str(input).is_ok()
}

// Get full token info by symbol
fn get_token_by_symbol<'a>(symbol: &str, tokens: &'a [Token]) -> Option<&'a Token> {
    tokens.iter().find(|t| t.symbol == symbol)
}

/// Hardware wallet approval overlay component for swap transactions
#[component]
fn HardwareApprovalOverlay(oncancel: EventHandler<()>) -> Element {
    rsx! {
        div {
            class: "hardware-approval-overlay",
            
            div {
                class: "hardware-approval-content",
                
                h3 { 
                    class: "hardware-approval-title",
                    "Confirm Swap on Hardware Wallet"
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
                    "Please check your hardware wallet and confirm the swap transaction details."
                }
                
                div {
                    class: "hardware-steps",
                    div {
                        class: "hardware-step",
                        div { class: "step-number", "1" }
                        span { "Press the button on your Unruggable to confirm the swap" }
                    }
                }
                
                button {
                    class: "hardware-cancel-button",
                    onclick: move |_| oncancel.call(()),
                    "Cancel Swap"
                }
            }
        }
    }
}

/// Swap transaction success modal component
#[component]
pub fn SwapTransactionSuccessModal(
    signature: String,
    selling_token: String,
    selling_amount: String,
    buying_token: String,
    buying_amount: String,
    was_hardware_wallet: bool,
    onclose: EventHandler<()>,
) -> Element {
    // Explorer links - Solscan and Orb
    let solscan_url = format!("https://solscan.io/tx/{}", signature);
    let orb_url = format!("https://orb.helius.dev/tx/{}?cluster=mainnet-beta&tab=summary", signature);
    
    rsx! {
        div {
            style: "
                position: fixed;
                inset: 0;
                background: rgba(0, 0, 0, 0.6);
                z-index: 9999;
                display: flex;
                align-items: flex-start;
                justify-content: center;
            ",
            onclick: move |_| onclose.call(()),

            div {
                style: "
                    width: 100%;
                    max-width: 560px;
                    background: #121212;
                    border-bottom-left-radius: 16px;
                    border-bottom-right-radius: 16px;
                    padding: 16px;
                    border: 1px solid #2a2a2a;
                ",
                onclick: move |e| e.stop_propagation(),

                div {
                    style: "display: flex; align-items: center; justify-content: space-between; margin-bottom: 12px;",
                    div { style: "font-size: 16px; font-weight: 700; color: white;", "Swap complete" }
                    button {
                        style: "background: transparent; border: none; color: #9ca3af; font-size: 16px;",
                        onclick: move |_| onclose.call(()),
                        "Close"
                    }
                }

                div {
                    style: "
                        display: flex;
                        align-items: center;
                        gap: 12px;
                        padding: 12px;
                        border-radius: 12px;
                        background: #1a1a1a;
                        border: 1px solid #2a2a2a;
                        margin-bottom: 12px;
                    ",
                    div {
                        style: "
                            width: 34px;
                            height: 34px;
                            border-radius: 50%;
                            background: rgba(16, 185, 129, 0.15);
                            display: flex;
                            align-items: center;
                            justify-content: center;
                            color: #10b981;
                            font-weight: 700;
                        ",
                        "✓"
                    }
                    div {
                        div { style: "font-size: 14px; font-weight: 700; color: white;", "Transaction submitted" }
                        div { style: "font-size: 12px; color: #94a3b8;", "Your swap is on Solana and should confirm shortly." }
                    }
                }

                div {
                    style: "
                        background: #1a1a1a;
                        border: 1px solid #2a2a2a;
                        border-radius: 12px;
                        padding: 12px;
                        margin-bottom: 12px;
                    ",
                    div {
                        style: "display: flex; justify-content: space-between; margin-bottom: 8px; color: #cbd5e1; font-size: 13px;",
                        span { "Sold" }
                        span { "{selling_amount} {selling_token}" }
                    }
                    div {
                        style: "display: flex; justify-content: space-between; color: #cbd5e1; font-size: 13px;",
                        span { "Received" }
                        span { "~{buying_amount} {buying_token}" }
                    }
                }

                if was_hardware_wallet {
                    div {
                        style: "
                            background: rgba(255, 171, 64, 0.1);
                            border: 1px solid rgba(255, 171, 64, 0.2);
                            color: #fbbf24;
                            border-radius: 10px;
                            padding: 10px 12px;
                            font-size: 12px;
                            margin-bottom: 12px;
                        ",
                        "Your hardware wallet disconnected after signing. Reconnect it for future swaps."
                    }
                }

                div {
                    style: "
                        background: #111;
                        border: 1px solid #2a2a2a;
                        border-radius: 12px;
                        padding: 12px;
                        margin-bottom: 12px;
                    ",
                    div { style: "font-size: 12px; color: #9ca3af; margin-bottom: 6px;", "Transaction signature" }
                    div {
                        style: "
                            font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, \"Liberation Mono\", \"Courier New\", monospace;
                            font-size: 12px;
                            color: #e5e7eb;
                            word-break: break-all;
                        ",
                        onclick: move |_| {
                            log::info!("Signature copied to clipboard: {}", signature);
                        },
                        "{signature}"
                    }
                }

                div {
                    style: "display: flex; gap: 8px;",
                    a {
                        style: "
                            flex: 1;
                            text-decoration: none;
                            background: #1f2937;
                            border: 1px solid #374151;
                            color: #e5e7eb;
                            padding: 10px 12px;
                            border-radius: 10px;
                            text-align: center;
                            font-size: 13px;
                            font-weight: 600;
                        ",
                        href: "{solscan_url}",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        "Solscan"
                    }
                    a {
                        style: "
                            flex: 1;
                            text-decoration: none;
                            background: #1f2937;
                            border: 1px solid #374151;
                            color: #e5e7eb;
                            padding: 10px 12px;
                            border-radius: 10px;
                            text-align: center;
                            font-size: 13px;
                            font-weight: 600;
                        ",
                        href: "{orb_url}",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        "Orb"
                    }
                }
            }
        }
    }
}

#[component]
pub fn SwapModal(
    tokens: Vec<Token>,
    wallet: Option<WalletInfo>,
    hardware_wallet: Option<Arc<HardwareWallet>>,
    custom_rpc: Option<String>,
    onclose: EventHandler<()>,
    onsuccess: EventHandler<String>,
) -> Element {
    println!("🔄 SwapModal component rendered with Jupiter Ultra API!");
    
    // Create channel for iOS-safe signal updates
    // All async tasks send updates here, main thread processes them
    let (swap_tx, swap_rx) = mpsc::unbounded_channel::<SwapUpdate>();
    let swap_tx = use_signal(|| Arc::new(swap_tx));
    let mut swap_rx = use_signal(|| Some(swap_rx));
    
    // State management
    let mut selling_token = use_signal(|| "SOL".to_string());
    let mut buying_token = use_signal(|| "USDC".to_string());
    let mut buying_token_meta = use_signal(|| None as Option<JupiterTokenMeta>);
    let mut selling_amount = use_signal(|| "".to_string());
    let mut buying_amount = use_signal(|| "0.00".to_string());
    let mut swapping = use_signal(|| false);
    let mut error_message = use_signal(|| None as Option<String>);

    // State for transaction success modal
    let mut show_success_modal = use_signal(|| false);
    let mut transaction_signature = use_signal(|| "".to_string());
    let mut was_hardware_transaction = use_signal(|| false);
    let mut show_hardware_approval = use_signal(|| false);

    // Jupiter Ultra API state (simple order + execute)
    let mut jupiter_order = use_signal(||None as Option<JupiterUltraOrderResponse>);
    let mut fetching_jupiter = use_signal(|| false);
    
    // Dflow instruction-based API state
    let mut dflow_quote = use_signal(|| None as Option<DflowQuoteResponse>);
    let mut dflow_instructions = use_signal(|| None as Option<DflowSwapInstructionsResponse>);
    let mut fetching_dflow = use_signal(|| false);

    // Titan Exchange state
    let titan_client = use_signal(|| {
        // Initialize Titan client with production global endpoint and JWT token
        let client = TitanClient::new(
            "partners.api.titan.exchange".to_string(),
            "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCIsImtpZCI6ImI5MzJiMTkwLTkxZTMtNDhkZC04M2JhLWI1ODA0OWQ1NjIzOSJ9.eyJpYXQiOjE3NjA1NjY2MjYsImV4cCI6MTc5MjEwMjYyNiwiYXVkIjoiYXBpLnRpdGFuLmFnIiwiaXNzIjoidGl0YW5fcGFydG5lcnMiLCJzdWIiOiJhcGk6dW5ydWdnYWJsZSJ9.fSI0QYG9jny2c6tWXEwl4JIFHYS1Twi2kiHjj-0e0tg".to_string(),
        );
        Arc::new(tokio::sync::Mutex::new(client))
    });
    let mut titan_quote = use_signal(|| None as Option<(String, TitanSwapRoute)>); // (provider_name, route)
    let mut fetching_titan = use_signal(|| false);
    let mut selected_provider = use_signal(|| None as Option<String>); // "Jupiter" or "Titan"

    // Buy-side token search (Jupiter strict list)
    let mut show_buy_token_search = use_signal(|| false);
    let mut show_sell_token_search = use_signal(|| false);
    let mut token_search_query = use_signal(|| "".to_string());
    let mut sell_search_query = use_signal(|| "".to_string());
    let mut last_buy_search_query = use_signal(|| "".to_string());
    let mut token_catalog = use_signal(|| Vec::<JupiterTokenMeta>::new());
    let mut token_catalog_loading = use_signal(|| false);
    let mut token_catalog_loaded = use_signal(|| false);
    let mut custom_token_loading = use_signal(|| false);
    let mut custom_token_error = use_signal(|| None as Option<String>);
    
    // Store hardware wallet address (fetched async)
    let mut hw_address = use_signal(|| None as Option<String>);
    
    // Clone tokens for closures - need separate clones for each closure
    let tokens_clone = tokens.clone();
    let tokens_clone2 = tokens.clone();
    let tokens_clone3 = tokens.clone();
    let tokens_clone4 = tokens.clone(); // For handle_amount_change
    let tokens_clone5 = tokens.clone(); // For quote comparison
    let tokens_clone6 = tokens.clone(); // For UI rendering
    let tokens_clone_price = tokens.clone(); // For price calculations in provider comparison
    let tokens_clone_swap = tokens.clone(); // For handle_token_swap
    let tokens_clone_price = tokens.clone(); // For provider comparison
    let tokens_clone_exchange_rate = tokens.clone(); // For exchange_rate calculation
    let tokens_clone_selling_usd = tokens.clone(); // For selling_usd_value
    let tokens_clone_sell_search = tokens.clone(); // For selling token search
    let tokens_clone_buying_usd = tokens.clone(); // For buying_usd_value

    // Show transaction success modal if swap completed
    if show_success_modal() {
        return rsx! {
            SwapTransactionSuccessModal {
                signature: transaction_signature(),
                selling_token: selling_token(),
                selling_amount: selling_amount(),
                buying_token: buying_token(),
                buying_amount: buying_amount(),
                was_hardware_wallet: was_hardware_transaction(),
                onclose: move |_| {
                    show_success_modal.set(false);
                    // Call onsuccess when the user closes the modal
                    onsuccess.call(transaction_signature());
                }
            }
        };
    }

    // Show hardware approval overlay if needed
    if show_hardware_approval() {
        return rsx! {
            HardwareApprovalOverlay {
                oncancel: move |_| {
                    show_hardware_approval.set(false);
                    swapping.set(false);
                    error_message.set(Some("Transaction cancelled".to_string()));
                }
            }
        };
    }

    // Clone values early to avoid move conflicts
    let hardware_wallet_clone = hardware_wallet.clone();
    let wallet_clone = wallet.clone();
    let wallet_clone_for_titan = wallet.clone(); // Separate clone for Titan swap
    let wallet_clone_for_buying = wallet.clone(); // For handle_token_swap
    let wallet_clone_for_buying_dropdown = wallet.clone(); // For buying token dropdown handler
    let hardware_wallet_clone2 = hardware_wallet.clone(); 
    let wallet_clone2 = wallet.clone();
    let custom_rpc_clone = custom_rpc.clone();
    
    // Fetch hardware wallet address on mount
    let hw_clone_for_effect = hardware_wallet.clone();
    use_effect(move || {
        if let Some(hw) = hw_clone_for_effect.clone() {
            spawn(async move {
                match hw.get_public_key().await {
                    Ok(address) => {
                        println!("📍 Hardware wallet address fetched: {}", address);
                        hw_address.set(Some(address));
                    }
                    Err(e) => {
                        println!("⚠️ Failed to get hardware wallet address: {}", e);
                        hw_address.set(None);
                    }
                }
            });
        }
    });

    // Update buying token selection (owned or search result)

    // Load local token catalog when search opens
    use_effect(move || {
        if show_buy_token_search() && !token_catalog_loaded() && !token_catalog_loading() {
            token_catalog_loading.set(true);
            let tokens: Vec<JupiterTokenMeta> = get_token_catalog()
                .iter()
                .map(|token| JupiterTokenMeta {
                    address: token.address.clone(),
                    name: token.name.clone(),
                    symbol: token.symbol.clone(),
                    logo_uri: token.logo_uri.clone(),
                    decimals: token.decimals,
                })
                .collect();
            println!("✅ Loaded {} local token catalog entries", tokens.len());
            token_catalog.set(tokens);
            token_catalog_loaded.set(true);
            token_catalog_loading.set(false);
        }
    });

    // Filter token catalog based on search query
    let token_search_results = use_memo(move || {
        let query = token_search_query().trim().to_lowercase();
        if query.is_empty() {
            return Vec::new();
        }

        let mut results: Vec<JupiterTokenMeta> = token_catalog()
            .into_iter()
            .filter(|token| {
                token.symbol.to_lowercase().contains(&query)
                    || token.name.to_lowercase().contains(&query)
                    || token.address.to_lowercase().contains(&query)
            })
            .take(50)
            .collect();

        results.sort_by_key(|t| {
            let sym = t.symbol.to_lowercase();
            if sym == query { 0 } else if sym.starts_with(&query) { 1 } else { 2 }
        });

        results
    });

    let sell_search_results = use_memo(move || {
        let query = sell_search_query().trim().to_lowercase();
        let catalog = tokens_clone_sell_search.clone();

        if query.is_empty() {
            return catalog;
        }

        let mut results: Vec<Token> = catalog
            .into_iter()
            .filter(|token| {
                token.symbol.to_lowercase().contains(&query)
                    || token.name.to_lowercase().contains(&query)
                    || token.mint.to_lowercase().contains(&query)
            })
            .take(50)
            .collect();

        results.sort_by_key(|t| {
            let sym = t.symbol.to_lowercase();
            if sym == query { 0 } else if sym.starts_with(&query) { 1 } else { 2 }
        });

        results
    });

    use_effect(move || {
        let query = token_search_query().trim().to_string();
        if query == last_buy_search_query() {
            return;
        }
        last_buy_search_query.set(query.clone());

        if query.is_empty() {
            return;
        }

        let results = token_search_results();
        let preview: Vec<String> = results.iter().take(3).map(|t| t.symbol.clone()).collect();
        println!(
            "🔎 Buy search \"{}\": catalog={} results={} preview={:?}",
            query,
            token_catalog().len(),
            results.len(),
            preview
        );
    });
    
    // iOS-SAFE: Listen to swap updates channel and update signals on main thread
    // This prevents panic_cannot_unwind crashes on iOS
    use_effect(move || {
        if let Some(mut rx) = swap_rx.write().take() {
            spawn(async move {
                while let Some(update) = rx.recv().await {
                    match update {
                        SwapUpdate::Started => {
                            println!("[iOS-SAFE] Swap started");
                            swapping.set(true);
                            error_message.set(None);
                        }
                        SwapUpdate::HardwareApprovalRequired(required) => {
                            println!("[iOS-SAFE] Hardware approval: {}", required);
                            show_hardware_approval.set(required);
                        }
                        SwapUpdate::Result(result) => {
                            match result {
                                SwapResult::Success(signature) => {
                                    println!("[iOS-SAFE] Swap success: {}", signature);
                                    transaction_signature.set(signature);
                                    swapping.set(false);
                                    show_success_modal.set(true);
                                }
                                SwapResult::Error(error) => {
                                    println!("[iOS-SAFE] Swap error: {}", error);
                                    swapping.set(false);
                                    error_message.set(Some(error));
                                }
                            }
                        }
                    }
                }
            });
        }
    });

    // Get user public key - prioritize hardware wallet when present
    let get_user_pubkey = move || -> Option<String> {
        // Check hardware wallet first - it takes precedence over software wallet
        if let Some(_hw) = &hardware_wallet_clone {
            // Get address from hardware wallet signal (pre-fetched)
            if let Some(address) = hw_address() {
                println!("📍 Using hardware wallet address: {}", address);
                return Some(address);
            } else {
                println!("⚠️ Hardware wallet address not yet fetched");
                return None;
            }
        }
        
        // Fall back to software wallet if no hardware wallet
        if let Some(wallet_info) = &wallet_clone {
            let address = wallet_info.address.clone();
            println!("📍 Using wallet address: {}", address);
            Some(address)
        } else {
            println!("⚠️ No wallet available");
            None
        }
    };

    // Titan Exchange: Fetch quotes with WebSocket streaming
    let fetch_titan_quotes = move |input_mint: String, output_mint: String, amount_lamports: u64, user_pubkey: Option<String>| {
        println!("[TITAN-DEBUG] fetch_titan_quotes called with input={}, output={}, amount={}", input_mint, output_mint, amount_lamports);

        let client = titan_client();

        println!("[TITAN-DEBUG] About to spawn async task");
        spawn(async move {
            println!("[TITAN-DEBUG] Inside spawned async task");

            // Prevent multiple simultaneous requests
            if fetching_titan() {
                println!("[TITAN-DEBUG] Already fetching, returning early");
                return;
            }

            fetching_titan.set(true);
            println!("[TITAN-DEBUG] Set fetching_titan to true");

            println!("🔷 Fetching Titan quotes...");

            // Get user pubkey - require valid address for transaction generation
            let user_pk = match user_pubkey {
                Some(pk) => {
                    println!("📍 Titan user pubkey: {}", pk);
                    pk
                }
                None => {
                    println!("❌ No user pubkey available - cannot generate Titan transaction");
                    fetching_titan.set(false);
                    return;
                }
            };

            println!("[TITAN-DEBUG] User pubkey validated: {}", user_pk);

            // iOS-SAFE: Use timeout wrapper for all operations to prevent iOS from killing the task
            let timeout_duration = std::time::Duration::from_secs(10);

            // Connect with timeout - release lock immediately after
            println!("[Titan] Connecting to WebSocket...");
            let connect_result = tokio::time::timeout(timeout_duration, async {
                let mut client_lock = client.lock().await;
                client_lock.connect().await
            }).await;

            match connect_result {
                Ok(Ok(())) => {
                    println!("[Titan] ✓ Connected successfully");
                }
                Ok(Err(e)) => {
                    println!("❌ Failed to connect to Titan: {}", e);
                    fetching_titan.set(false);
                    return;
                }
                Err(_) => {
                    println!("❌ Titan connection timeout (iOS network issue)");
                    fetching_titan.set(false);
                    return;
                }
            }

            // Request quotes with timeout - shorter lock duration
            println!("[Titan] Requesting swap quotes...");
            let quote_result = tokio::time::timeout(timeout_duration, async {
                let mut client_lock = client.lock().await;
                client_lock.request_swap_quotes(
                    &input_mint,
                    &output_mint,
                    amount_lamports,
                    &user_pk,
                    Some(50), // 0.5% slippage
                ).await
            }).await;

            match quote_result {
                Ok(Ok((provider_name, route))) => {
                    println!("✅ Titan quote received from provider: {}", provider_name);
                    println!("📊 Output amount: {} lamports", route.out_amount);
                    println!("🔍 Transaction field present: {}", route.transaction.is_some());
                    if let Some(ref tx) = route.transaction {
                        println!("📄 Transaction size: {} bytes", tx.len());
                    } else {
                        println!("⚠️ No transaction data in Titan quote!");
                    }
                    titan_quote.set(Some((provider_name, route)));
                }
                Ok(Err(e)) => {
                    println!("❌ Failed to get Titan quote: {}", e);
                    titan_quote.set(None);
                }
                Err(_) => {
                    println!("❌ Titan quote request timeout (took > 10s)");
                    titan_quote.set(None);
                }
            }

            // Close connection with timeout
            println!("[Titan] Closing connection...");
            let close_result = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                async {
                    let mut client_lock = client.lock().await;
                    client_lock.close().await
                }
            ).await;

            match close_result {
                Ok(Ok(())) => println!("[Titan] ✓ Connection closed"),
                Ok(Err(e)) => println!("[Titan] Warning: close error: {}", e),
                Err(_) => println!("[Titan] Warning: close timeout"),
            }

            fetching_titan.set(false);
        });
    };

    // Dflow: Fetch quote
    let fetch_dflow_quote = move |input_mint: String, output_mint: String, amount_lamports: u64, slippage_bps: u16| {
        spawn(async move {
            if fetching_dflow() {
                return;
            }
            
            fetching_dflow.set(true);
            
            let client = reqwest::Client::new();
            // Step 1: Get quote from Dflow
            let quote_url = format!(
                "https://quote-api.dflow.net/quote?inputMint={}&outputMint={}&amount={}&slippageBps={}",
                input_mint, output_mint, amount_lamports, slippage_bps
            );
            
            println!("💜 Fetching Dflow quote: {}", quote_url);
            
            match client
                .get(&quote_url)
                .header("x-api-key", "HboXeWH6dkjayWfKnkmh")
                .send()
                .await
            {
                Ok(response) => {
                    if response.status().is_success() {
                        match response.json::<DflowQuoteResponse>().await {
                            Ok(quote) => {
                                println!("✅ Dflow quote received: {} -> {}", quote.in_amount, quote.out_amount);
                                
                                // Store quote for comparison
                                dflow_quote.set(Some(quote.clone()));
                                
                                // Step 2: Instructions will be fetched in CHUNK 5 during swap execution
                                println!("✅ Dflow quote stored (instruction fetch pending until swap)");
                            }
                            Err(e) => {
                                println!("❌ Failed to parse Dflow quote: {}", e);
                            }
                        }
                    } else {
                        println!("❌ Dflow quote API error: {}", response.status());
                    }
                }
                Err(e) => {
                    println!("❌ Dflow quote request failed: {}", e);
                }
            }
            
            fetching_dflow.set(false);
        });
    };
    
    // Jupiter Ultra API: Fetch order (quote + unsigned transaction)
    let fetch_jupiter_order = move |input_mint: String, output_mint: String, amount_lamports: u64, user_pubkey: Option<String>| {
        spawn(async move {
            if fetching_jupiter() {
                return;
            }
            
            fetching_jupiter.set(true);
            error_message.set(None);
            
            let client = reqwest::Client::new();
            
            // Build Jupiter Ultra order URL
            let mut url = format!(
                "https://api.jup.ag/ultra/v1/order?inputMint={}&outputMint={}&amount={}",
                input_mint, output_mint, amount_lamports
            );
            
            // Add taker (user pubkey) if available for unsigned transaction
            if let Some(pubkey) = user_pubkey {
                url.push_str(&format!("&taker={}", pubkey));
            }
            
            println!("🪐 Fetching Jupiter Ultra order: {}", url);
            
            match client
                .get(&url)
                .header("x-api-key", "ddbf7533-efd7-41a4-b794-59325ccbc383")
                .send()
                .await {
                Ok(response) => {
                    if response.status().is_success() {
                        match response.json::<JupiterUltraOrderResponse>().await {
                            Ok(order) => {
                                println!("✅ Jupiter Ultra order received: {} -> {}", order.in_amount, order.out_amount);
                                
                                // Check for API-level errors first
                                if let Some(error_msg) = &order.error_message {
                                    println!("❌ Jupiter Ultra API Error: {}", error_msg);
                                    error_message.set(Some(match error_msg.as_str() {
                                        "Taker has insufficient input" => "Insufficient balance for this swap".to_string(),
                                        msg if msg.contains("insufficient") => "Insufficient balance".to_string(),
                                        _ => format!("Swap error: {}", error_msg),
                                    }));
                                } else {
                                    // Store order for comparison and swap execution
                                    jupiter_order.set(Some(order));
                                }
                            }
                            Err(e) => {
                                println!("❌ Failed to parse Jupiter Ultra response: {}", e);
                                error_message.set(Some("Failed to get swap quote".to_string()));
                            }
                        }
                    } else {
                        println!("❌ Jupiter Ultra API returned error status: {}", response.status());
                        error_message.set(Some(format!("API error: {}", response.status())));
                    }
                }
                Err(e) => {
                    println!("❌ Jupiter Ultra request failed: {}", e);
                    error_message.set(Some("Network error - please try again".to_string()));
                }
            }
            
            fetching_jupiter.set(false);
        });
    };

    let update_buying_token = {
        let tokens_for_update = tokens_clone6.clone();
        let wallet_for_update = wallet_clone_for_buying_dropdown.clone();
        let hw_for_update = hw_address.clone();
        Rc::new(RefCell::new(move |symbol: String, meta: Option<JupiterTokenMeta>| {
            let mut final_symbol = symbol;
            let mut final_meta = meta;

            if let Some(meta) = final_meta.clone() {
                if let Some(owned) = tokens_for_update.iter().find(|t| t.mint == meta.address) {
                    final_symbol = owned.symbol.clone();
                    final_meta = None;
                } else {
                    final_symbol = meta.symbol.clone();
                }
            }

            buying_token.set(final_symbol.clone());
            buying_token_meta.set(final_meta.clone());
            show_buy_token_search.set(false);
            buying_amount.set("0.00".to_string());
            jupiter_order.set(None);
            dflow_quote.set(None);
            titan_quote.set(None);
            selected_provider.set(None);

            if !selling_amount().is_empty() {
                if let Ok(amount) = selling_amount().parse::<f64>() {
                    if amount > 0.0 {
                        let amount_lamports = to_lamports(amount, &selling_token(), &tokens_for_update);
                        let input_mint = get_token_mint(&selling_token(), &tokens_for_update).to_string();
                        let output_mint = get_token_mint_with_meta(&final_symbol, &tokens_for_update, final_meta.as_ref());

                        let user_pubkey_str = if let Some(address) = hw_for_update() {
                            Some(address)
                        } else if let Some(wallet_info) = &wallet_for_update {
                            Some(wallet_info.address.clone())
                        } else {
                            None
                        };

                        if let Some(user_pubkey) = user_pubkey_str {
                            let input_mint_jup = input_mint.clone();
                            let output_mint_jup = output_mint.clone();
                            let user_pubkey_jup = user_pubkey.clone();

                            let input_mint_dflow = input_mint.clone();
                            let output_mint_dflow = output_mint.clone();

                            let input_mint_titan = input_mint.clone();
                            let output_mint_titan = output_mint.clone();
                            let user_pubkey_titan = user_pubkey.clone();

                            spawn(async move {
                                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                                println!("🔄 Refetching quotes for new buying token...");
                                fetch_jupiter_order(input_mint_jup, output_mint_jup, amount_lamports, Some(user_pubkey_jup));
                                fetch_dflow_quote(input_mint_dflow, output_mint_dflow, amount_lamports, 50);
                                fetch_titan_quotes(input_mint_titan, output_mint_titan, amount_lamports, Some(user_pubkey_titan));
                            });
                        }
                    }
                }
            }
        }))
    };

    let update_selling_token = {
        Rc::new(RefCell::new(move |symbol: String| {
            selling_token.set(symbol);
            selling_amount.set("".to_string());
            buying_amount.set("0.00".to_string());
            jupiter_order.set(None);
            dflow_quote.set(None);
            titan_quote.set(None);
            selected_provider.set(None);
            show_sell_token_search.set(false);
        }))
    };





    // Calculate exchange rate for fallback display using live prices
    let exchange_rate = use_memo(move || {
        let selling_price = tokens_clone_exchange_rate.iter()
            .find(|t| t.symbol == selling_token())
            .map(|t| t.price)
            .unwrap_or(1.0);
        
        let buying_price = tokens_clone_exchange_rate.iter()
            .find(|t| t.symbol == buying_token())
            .map(|t| t.price)
            .unwrap_or(1.0);
        
        if buying_price > 0.0 {
            selling_price / buying_price
        } else {
            1.0
        }
    });

    // Handle amount input changes with debouncing and balance validation
    let mut handle_amount_change = move |value: String| {
        selling_amount.set(value.clone());
        error_message.set(None);
        jupiter_order.set(None); // Clear previous Jupiter order
        titan_quote.set(None); // Clear previous Titan quote
        selected_provider.set(None); // Clear provider selection
        
        if !value.is_empty() {
            if let Ok(amount) = value.parse::<f64>() {
                // Check balance before making API call
                let selling_balance = tokens_clone4.iter()
                    .find(|t| t.symbol == selling_token())
                    .map(|t| t.balance)
                    .unwrap_or(0.0);
                
                if amount > selling_balance {
                    error_message.set(Some(format!("Insufficient balance. You have {:.6} {}", selling_balance, selling_token())));
                    buying_amount.set("0.00".to_string());
                    return;
                }
                
                // Show fallback rate immediately
                let fallback_rate = exchange_rate();
                let fallback_converted = amount * fallback_rate;
                let fallback_formatted = if fallback_converted < 0.01 && fallback_converted > 0.0 {
                    format!("{:.6}", fallback_converted)
                } else {
                    format!("{:.2}", fallback_converted)
                };
                buying_amount.set(fallback_formatted);
                
                // Fetch quotes from BOTH Jupiter and Titan in parallel
                if amount <= selling_balance && amount > 0.0 {
                    let amount_lamports = to_lamports(amount, &selling_token(), &tokens_clone4);
                    
                    let buying_meta = buying_token_meta();
                    let input_mint = get_token_mint(&selling_token(), &tokens_clone4).to_string();
                    let output_mint = get_token_mint_with_meta(&buying_token(), &tokens_clone4, buying_meta.as_ref());
                    let user_pubkey = get_user_pubkey();
                    
                    // Clone for each async call
                    let input_mint_jup = input_mint.clone();
                    let output_mint_jup = output_mint.clone();
                    let user_pubkey_jup = user_pubkey.clone();
                    
                    let input_mint_dflow = input_mint.clone();
                    let output_mint_dflow = output_mint.clone();
                    
                    let input_mint_titan = input_mint.clone();
                    let output_mint_titan = output_mint.clone();
                    let user_pubkey_titan = user_pubkey.clone();
                    
                    // Add small delay to prevent too many API calls
                    spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                        
                        // Spawn both quote requests in parallel
                        println!("🔄 Fetching quotes from both Jupiter and Titan...");
                        
                        // Fetch all three providers in parallel
                        fetch_jupiter_order(input_mint_jup, output_mint_jup, amount_lamports, user_pubkey_jup);
                        fetch_dflow_quote(input_mint_dflow, output_mint_dflow, amount_lamports, 50);
                        fetch_titan_quotes(input_mint_titan, output_mint_titan, amount_lamports, user_pubkey_titan);
                    });
                }
            }
        } else {
            buying_amount.set("0.00".to_string());
            jupiter_order.set(None);
            titan_quote.set(None);
        }
    };

    // Quote comparison logic: Compare all three providers and select the best
    use_effect(move || {
        let jupiter_o = jupiter_order();
        let dflow_q = dflow_quote();
        let titan_q = titan_quote();
        
        // Collect all available quotes with their output amounts
        let mut quotes = Vec::new();
        
        if let Some(order) = jupiter_o.clone() {
            let output = order.out_amount.parse::<u64>().unwrap_or(0);
            quotes.push(("Jupiter", output));
        }
        
        if let Some(order) = dflow_q.clone() {
            let output = order.out_amount.parse::<u64>().unwrap_or(0);
            quotes.push(("Dflow", output));
        }
        
        if let Some((_, route)) = titan_q.clone() {
            quotes.push(("Titan", route.out_amount));
        }
        
        if !quotes.is_empty() {
            // Find the best quote (highest output)
            let best = quotes.iter().max_by_key(|(_, amount)| amount).unwrap();
            let (winner, best_amount) = best;
            
            println!("📊 Quote Comparison:");
            for (provider, amount) in &quotes {
                println!("   {}: {} lamports", provider, amount);
            }
            println!("🏆 {} wins with {} lamports", winner, best_amount);
            
            selected_provider.set(Some(winner.to_string()));
            
            // Update buying amount with winner's quote
            let buying_meta = buying_token_meta();
            let converted_amount = from_lamports_with_meta(*best_amount, &buying_token(), &tokens_clone5, buying_meta.as_ref());
            let formatted = if converted_amount < 0.01 && converted_amount > 0.0 {
                format!("{:.6}", converted_amount)
            } else {
                format!("{:.2}", converted_amount)
            };
            buying_amount.set(formatted);
        }
    });

    // Handle swap execution with real transaction signing
    let handle_swap = {
        move |_| {
            println!("🔄 Swap button clicked! Selling: {} {} -> Buying: {} {}", 
                selling_amount(), selling_token(), buying_amount(), buying_token());
            
            if selling_amount().is_empty() {
                error_message.set(Some("Please enter an amount to sell".to_string()));
                return;
            }

            // Clone custom_rpc at the start so it can be used in multiple spawn blocks
            let custom_rpc_for_titan = custom_rpc_clone.clone();

            // Double-check balance validation
            if let Ok(amount) = selling_amount().parse::<f64>() {
                let selling_balance = tokens_clone3.iter()
                    .find(|t| t.symbol == selling_token())
                    .map(|t| t.balance)
                    .unwrap_or(0.0);
                    
                if amount > selling_balance {
                    error_message.set(Some(format!("Insufficient balance. You have {:.6} {}", selling_balance, selling_token())));
                    return;
                }

                // Check which provider won the quote comparison
                let provider = selected_provider();
                
                if provider == Some("Titan".to_string()) {
                    // Titan won - build transaction from instructions
                    if let Some((provider_name, titan_route)) = titan_quote() {
                        println!("✅ Using Titan ({}) for swap", provider_name);
                        println!("📊 Building transaction from {} instructions", titan_route.instructions.len());
                        
                        swapping.set(true);
                        error_message.set(None);
                        
                        // Get user pubkey for transaction building - prioritize hardware wallet
                        // Check hardware wallet FIRST, then fall back to software wallet
                        let user_pubkey_str = if let Some(address) = hw_address() {
                            Some(address)
                        } else if let Some(wallet_info) = &wallet_clone_for_titan {
                            Some(wallet_info.address.clone())
                        } else {
                            None
                        };
                        
                        let user_pubkey_str = match user_pubkey_str {
                            Some(pk) => pk,
                            None => {
                                error_message.set(Some("No wallet available".to_string()));
                                swapping.set(false);
                                return;
                            }
                        };
                        
                        // Parse pubkey
                        let user_pubkey = match user_pubkey_str.parse::<SolanaPubkey>() {
                            Ok(pk) => pk,
                            Err(e) => {
                                error_message.set(Some(format!("Invalid pubkey: {}", e)));
                                swapping.set(false);
                                return;
                            }
                        };
                        
                        // Clone values for the async block
                        let hw_clone = hardware_wallet_clone2.clone();
                        let wallet_info_clone = wallet_clone2.clone();
                        let custom_rpc_titan = custom_rpc_for_titan.clone();
                        let custom_rpc_for_client = custom_rpc_for_titan.clone();
                        
                        // Build transaction from Titan's instructions
                        spawn(async move {
                            println!("🔧 Fetching recent blockhash...");
                            
                            // Create RPC client to fetch recent blockhash
                            let rpc_url_for_client = custom_rpc_titan.as_deref();
                            let rpc_client = TransactionClient::new(rpc_url_for_client);
                            
                            // Fetch recent blockhash
                            let recent_blockhash = match rpc_client.get_recent_blockhash().await {
                                Ok(hash) => {
                                    println!("✅ Recent blockhash: {}", hash);
                                    hash
                                }
                                Err(e) => {
                                    println!("❌ Failed to fetch blockhash: {}", e);
                                    swapping.set(false);
                                    error_message.set(Some(format!("Failed to get blockhash: {}", e)));
                                    return;
                                }
                            };
                            
                            // Build transaction from Titan route with lookup tables
                            let rpc_url = custom_rpc_titan.as_deref().unwrap_or("https://johna-k3cr1v-fast-mainnet.helius-rpc.com");
                            let is_hardware = hw_clone.is_some();
                            let unsigned_tx_bytes = match build_transaction_from_route(
                                &titan_route,
                                user_pubkey,
                                recent_blockhash,
                                rpc_url,
                                is_hardware,
                            ).await {
                                Ok(bytes) => {
                                    println!("✅ Transaction built: {} bytes", bytes.len());
                                    bytes
                                }
                                Err(e) => {
                                    println!("❌ Failed to build transaction: {}", e);
                                    swapping.set(false);
                                    error_message.set(Some(format!("Failed to build transaction: {}", e)));
                                    return;
                                }
                            };
                            
                            // Convert to base64 for signing
                            let unsigned_tx_b64 = base64::encode(&unsigned_tx_bytes);
                            
                            // Continue with signing flow
                            // Determine if this is a hardware wallet transaction
                            let is_hardware = hw_clone.is_some();
                            was_hardware_transaction.set(is_hardware);
                            
                            if is_hardware {
                                show_hardware_approval.set(true);
                            }
                            
                            println!("🔐 Signing Titan transaction...");
                            
                            // Create the appropriate signer
                            let signing_result = if let Some(hw) = hw_clone {
                                println!("💻 Using hardware wallet signer");
                                let hw_signer = HardwareSigner::from_wallet(hw);
                                sign_jupiter_transaction(&hw_signer, &unsigned_tx_b64).await
                            } else if let Some(wallet_info) = wallet_info_clone {
                                println!("🔑 Using software wallet signer");
                                match Wallet::from_wallet_info(&wallet_info) {
                                    Ok(wallet) => {
                                        let sw_signer = SoftwareSigner::new(wallet);
                                        sign_jupiter_transaction(&sw_signer, &unsigned_tx_b64).await
                                    }
                                    Err(e) => {
                                        Err(format!("Failed to load wallet: {}", e))
                                    }
                                }
                            } else {
                                Err("No wallet available for signing".to_string())
                            };
                            
                            if is_hardware {
                                show_hardware_approval.set(false);
                            }
                            
                            match signing_result {
                                Ok(signed_transaction_b64) => {
                                    println!("✅ Transaction signed successfully!");
                                    println!("🚀 Submitting to Solana RPC...");
                                    
                                    // Execute Titan swap via direct Solana RPC submission
                                    let custom_rpc_final = custom_rpc_for_client.clone();
                                    spawn(async move {
                                        let tx_client = TransactionClient::new(custom_rpc_final.as_deref());
                                        println!("🔷 Executing Titan swap via Solana RPC...");
                                        
                                        // Convert base64 signed transaction to bytes
                                        let signed_tx_bytes = match base64::decode(&signed_transaction_b64) {
                                            Ok(bytes) => bytes,
                                            Err(e) => {
                                                println!("❌ Failed to decode base64 transaction: {}", e);
                                                swapping.set(false);
                                                error_message.set(Some(format!("Transaction decode error: {}", e)));
                                                return;
                                            }
                                        };
                                        
                                        println!("📄 Decoded transaction: {} bytes", signed_tx_bytes.len());
                                        
                                        // Encode to base58 for Solana RPC submission
                                        let signed_tx_b58 = bs58::encode(&signed_tx_bytes).into_string();
                                        
                                        println!("📝 Encoded to base58: {} chars", signed_tx_b58.len());
                                        
                                        // Use the pre-initialized global TransactionClient (TPU already initialized at app startup)
                                        println!("[TPU] Using pre-initialized TransactionClient with TPU ready");
                                        
                                        // Submit directly to Solana RPC (via TPU + RPC in parallel)
                                        match tx_client.send_transaction(&signed_tx_b58).await {
                                            Ok(signature) => {
                                                println!("✅ Titan swap executed successfully! Signature: {}", signature);
                                                transaction_signature.set(signature);
                                                swapping.set(false);
                                                show_success_modal.set(true);
                                            }
                                            Err(e) => {
                                                println!("❌ Titan swap failed: {}", e);
                                                swapping.set(false);
                                                error_message.set(Some(format!("Swap failed: {}", e)));
                                            }
                                        }
                                    });
                                }
                                Err(e) => {
                                    println!("❌ Transaction signing failed: {}", e);
                                    swapping.set(false);
                                    error_message.set(Some(format!("Failed to sign transaction: {}", e)));
                                }
                            }
                        });
                    } else {
                        error_message.set(Some("No Titan quote available".to_string()));
                    }
                } else if provider == Some("Jupiter".to_string()) {
                    // Jupiter won - use Ultra API (simple sign + execute)
                    if let Some(order) = jupiter_order.read().as_ref().cloned() {
                        println!("✅ Using Jupiter Ultra for swap");
                        
                        // Check for order errors
                        if let Some(error_msg) = &order.error_message {
                            error_message.set(Some(format!("Cannot swap: {}", error_msg)));
                            return;
                        }
                        
                        // Check for transaction
                        let unsigned_tx_b64 = match &order.transaction {
                            Some(tx) if !tx.is_empty() => tx.clone(),
                            _ => {
                                error_message.set(Some("No transaction in Jupiter order".to_string()));
                                return;
                            }
                        };
                        
                        swapping.set(true);
                        error_message.set(None);
                        
                        // Clone values for async block
                        let hw_clone = hardware_wallet_clone2.clone();
                        let wallet_info_clone = wallet_clone2.clone();
                        let request_id = order.request_id.clone();
                        
                        spawn(async move {
                            // Determine if hardware wallet
                            let is_hardware = hw_clone.is_some();
                            was_hardware_transaction.set(is_hardware);
                            
                            if is_hardware {
                                show_hardware_approval.set(true);
                            }
                            
                            println!("🔐 Signing Jupiter Ultra transaction...");
                            
                            // Sign transaction
                            let signing_result = if let Some(hw) = hw_clone {
                                println!("💻 Using hardware wallet signer");
                                let hw_signer = HardwareSigner::from_wallet(hw);
                                sign_jupiter_transaction(&hw_signer, &unsigned_tx_b64).await
                            } else if let Some(wallet_info) = wallet_info_clone {
                                println!("🔑 Using software wallet signer");
                                match Wallet::from_wallet_info(&wallet_info) {
                                    Ok(wallet) => {
                                        let sw_signer = SoftwareSigner::new(wallet);
                                        sign_jupiter_transaction(&sw_signer, &unsigned_tx_b64).await
                                    }
                                    Err(e) => {
                                        Err(format!("Failed to load wallet: {}", e))
                                    }
                                }
                            } else {
                                Err("No wallet available for signing".to_string())
                            };
                            
                            if is_hardware {
                                show_hardware_approval.set(false);
                            }
                            
                            match signing_result {
                                Ok(signed_transaction_b64) => {
                                    println!("✅ Jupiter transaction signed!");
                                    println!("🚀 Executing via Jupiter Ultra API...");
                                    
                                    // Execute via Jupiter Ultra execute endpoint
                                    let client = reqwest::Client::new();
                                    let execute_request = JupiterUltraExecuteRequest {
                                        signed_transaction: signed_transaction_b64,
                                        request_id,
                                    };
                                    
                                    match client
                                        .post("https://api.jup.ag/ultra/v1/execute")
                                        .header("x-api-key", "ddbf7533-efd7-41a4-b794-59325ccbc383")
                                        .json(&execute_request)
                                        .send()
                                        .await
                                    {
                                        Ok(response) => {
                                            if response.status().is_success() {
                                                match response.json::<JupiterUltraExecuteResponse>().await {
                                                    Ok(result) => {
                                                        if result.status == "Success" {
                                                            if let Some(signature) = result.signature {
                                                                println!("✅ Jupiter Ultra swap executed! Signature: {}", signature);
                                                                transaction_signature.set(signature);
                                                                swapping.set(false);
                                                                show_success_modal.set(true);
                                                            } else {
                                                                println!("❌ No signature in success response");
                                                                swapping.set(false);
                                                                error_message.set(Some("No signature returned".to_string()));
                                                            }
                                                        } else {
                                                            let error = result.error.unwrap_or("Unknown error".to_string());
                                                            println!("❌ Jupiter Ultra execute failed: {}", error);
                                                            swapping.set(false);
                                                            error_message.set(Some(format!("Swap failed: {}", error)));
                                                        }
                                                    }
                                                    Err(e) => {
                                                        println!("❌ Failed to parse execute response: {}", e);
                                                        swapping.set(false);
                                                        error_message.set(Some("Failed to parse response".to_string()));
                                                    }
                                                }
                                            } else {
                                                println!("❌ Jupiter Ultra execute error: {}", response.status());
                                                swapping.set(false);
                                                error_message.set(Some(format!("Execute error: {}", response.status())));
                                            }
                                        }
                                        Err(e) => {
                                            println!("❌ Jupiter Ultra execute request failed: {}", e);
                                            swapping.set(false);
                                            error_message.set(Some("Network error".to_string()));
                                        }
                                    }
                                }
                                Err(e) => {
                                    println!("❌ Transaction signing failed: {}", e);
                                    swapping.set(false);
error_message.set(Some(format!("Failed to sign: {}", e)));
                                }
                            }
                        });
                    } else {
                        error_message.set(Some("No Jupiter order available".to_string()));
                        swapping.set(false);
                    }
                } else if provider == Some("Dflow".to_string()) {
                    // Dflow won - fetch instructions and execute swap
                    if let Some(quote) = dflow_quote.read().as_ref().cloned() {
                        println!("✅ Using Dflow for swap");
                        println!("📊 Fetching Dflow swap instructions...");
                        
                        swapping.set(true);
                        error_message.set(None);
                        
                        // Get user pubkey - prioritize hardware wallet
                        let user_pubkey_str = if let Some(address) = hw_address() {
                            Some(address)
                        } else if let Some(wallet_info) = &wallet_clone_for_titan {
                            Some(wallet_info.address.clone())
                        } else {
                            None
                        };
                        
                        let user_pubkey_str = match user_pubkey_str {
                            Some(pk) => pk,
                            None => {
                                error_message.set(Some("No wallet available".to_string()));
                                swapping.set(false);
                                return;
                            }
                        };
                        
                        // Parse pubkey
                        let user_pubkey = match user_pubkey_str.parse::<SolanaPubkey>() {
                            Ok(pk) => pk,
                            Err(e) => {
                                error_message.set(Some(format!("Invalid pubkey: {}", e)));
                                swapping.set(false);
                                return;
                            }
                        };
                        
                        // Clone values for async block
                        let hw_clone = hardware_wallet_clone2.clone();
                        let wallet_info_clone = wallet_clone2.clone();
                        let custom_rpc_dflow = custom_rpc_clone.clone();
                        
                        // Fetch Dflow swap instructions then build transaction
                        spawn(async move {
                            println!("🔧 Fetching Dflow swap instructions...");
                            
                            let client = reqwest::Client::new();
                            
                            // Request swap instructions from Dflow
                            let instructions_request = DflowSwapInstructionsRequest {
                                user_public_key: user_pubkey_str.clone(),
                                wrap_and_unwrap_sol: true,
                                dynamic_compute_unit_limit: true,
                                prioritization_fee_lamports: serde_json::json!("auto"),
                                quote_response: quote,
                            };
                            
                            let instructions = match client
                                .post("https://quote-api.dflow.net/swap-instructions")
                                .header("x-api-key", "HboXeWH6dkjayWfKnkmh")
                                .json(&instructions_request)
                                .send()
                                .await
                            {
                                Ok(response) => {
                                    if response.status().is_success() {
                                        match response.json::<DflowSwapInstructionsResponse>().await {
                                            Ok(inst) => {
                                                println!("✅ Dflow instructions received ({} total)", 
                                                    inst.compute_budget_instructions.len() + 
                                                    inst.setup_instructions.len() + 1);
                                                inst
                                            }
                                            Err(e) => {
                                                println!("❌ Failed to parse Dflow instructions: {}", e);
                                                swapping.set(false);
                                                error_message.set(Some(format!("Failed to parse instructions: {}", e)));
                                                return;
                                            }
                                        }
                                    } else {
                                        println!("❌ Dflow instructions API error: {}", response.status());
                                        swapping.set(false);
                                        error_message.set(Some(format!("Dflow API error: {}", response.status())));
                                        return;
                                    }
                                }
                                Err(e) => {
                                    println!("❌ Dflow instructions request failed: {}", e);
                                    swapping.set(false);
                                    error_message.set(Some(format!("Network error: {}", e)));
                                    return;
                                }
                            };
                            
                            println!("🔧 Building Dflow transaction from instructions...");
                            
                            let rpc_url = custom_rpc_dflow.as_deref().unwrap_or("https://johna-k3cr1v-fast-mainnet.helius-rpc.com");
                            let is_hardware = hw_clone.is_some();
                            
                            // Build transaction using unified builder (includes timeout + Jules tip unless hardware wallet)
                            let unsigned_tx_bytes = match build_transaction_from_instructions(
                                instructions.compute_budget_instructions,
                                instructions.setup_instructions,
                                instructions.swap_instruction,
                                instructions.cleanup_instructions,
                                instructions.other_instructions,
                                instructions.address_lookup_table_addresses,
                                user_pubkey,
                                rpc_url,
                                is_hardware,
                            ).await {
                                Ok(bytes) => {
                                    println!("✅ Dflow transaction built: {} bytes", bytes.len());
                                    bytes
                                }
                                Err(e) => {
                                    println!("❌ Failed to build Dflow transaction: {}", e);
                                    swapping.set(false);
                                    error_message.set(Some(format!("Failed to build transaction: {}", e)));
                                    return;
                                }
                            };
                            
                            // Convert to base64 for signing
                            let unsigned_tx_b64 = base64::encode(&unsigned_tx_bytes);
                            
                            // Determine if hardware wallet
                            let is_hardware = hw_clone.is_some();
                            was_hardware_transaction.set(is_hardware);
                            
                            if is_hardware {
                                show_hardware_approval.set(true);
                            }
                            
                            println!("🔐 Signing Dflow transaction...");
                            
                            // Sign transaction
                            let signing_result = if let Some(hw) = hw_clone {
                                println!("💻 Using hardware wallet signer");
                                let hw_signer = HardwareSigner::from_wallet(hw);
                                sign_jupiter_transaction(&hw_signer, &unsigned_tx_b64).await
                            } else if let Some(wallet_info) = wallet_info_clone {
                                println!("🔑 Using software wallet signer");
                                match Wallet::from_wallet_info(&wallet_info) {
                                    Ok(wallet) => {
                                        let sw_signer = SoftwareSigner::new(wallet);
                                        sign_jupiter_transaction(&sw_signer, &unsigned_tx_b64).await
                                    }
                                    Err(e) => {
                                        Err(format!("Failed to load wallet: {}", e))
                                    }
                                }
                            } else {
                                Err("No wallet available for signing".to_string())
                            };
                            
                            if is_hardware {
                                show_hardware_approval.set(false);
                            }
                            
                            match signing_result {
                                Ok(signed_transaction_b64) => {
                                    println!("✅ Dflow transaction signed successfully!");
                                    println!("🚀 Submitting to Solana via RPC...");
                                    
                                    // Execute Dflow swap via RPC
                                    let custom_rpc_final = custom_rpc_dflow.clone();
                                    spawn(async move {
                                        let tx_client = TransactionClient::new(custom_rpc_final.as_deref());
                                        println!("💜 Executing Dflow swap via TPU + RPC...");
                                        
                                        // Convert base64 to bytes
                                        let signed_tx_bytes = match base64::decode(&signed_transaction_b64) {
                                            Ok(bytes) => bytes,
                                            Err(e) => {
                                                println!("❌ Failed to decode transaction: {}", e);
                                                swapping.set(false);
                                                error_message.set(Some(format!("Transaction decode error: {}", e)));
                                                return;
                                            }
                                        };
                                        
                                        println!("📄 Decoded transaction: {} bytes", signed_tx_bytes.len());
                                        
                                        // Encode to base58 for submission
                                        let signed_tx_b58 = bs58::encode(&signed_tx_bytes).into_string();
                                        
                                        println!("📝 Encoded to base58: {} chars", signed_tx_b58.len());
                                        
                                        // Submit via pre-initialized TPU client (initialized at app startup)
                                        println!("[TPU] Using pre-initialized TransactionClient with TPU ready");
                                        
                                        match tx_client.send_transaction(&signed_tx_b58).await {
                                            Ok(signature) => {
                                                println!("✅ Dflow swap executed successfully! Signature: {}", signature);
                                                transaction_signature.set(signature);
                                                swapping.set(false);
                                                show_success_modal.set(true);
                                            }
                                            Err(e) => {
                                                println!("❌ Dflow swap failed: {}", e);
                                                swapping.set(false);
                                                error_message.set(Some(format!("Swap failed: {}", e)));
                                            }
                                        }
                                    });
                                }
                                Err(e) => {
                                    println!("❌ Transaction signing failed: {}", e);
                                    swapping.set(false);
                                    error_message.set(Some(format!("Failed to sign transaction: {}", e)));
                                }
                            }
                        });
                    } else {
                        error_message.set(Some("No Dflow quote available".to_string()));
                        swapping.set(false);
                    }
                } else {
                    // No provider selected or no quotes available
                    error_message.set(Some("No quote available - please wait for quotes".to_string()));
                }
            }
        }
    };

    // Handle token swap direction - preserve buying amount and refetch quotes
    let handle_token_swap = move |_| {
        println!("🔄 Token swap direction clicked!");
        if buying_token_meta().is_some() {
            error_message.set(Some("Swap direction disabled for external buy tokens".to_string()));
            return;
        }
        let current_selling = selling_token();
        let current_buying = buying_token();
        let current_buying_amount = buying_amount();
        
        // Swap tokens
        selling_token.set(current_buying.clone());
        buying_token.set(current_selling.clone());
        
        // Preserve buying amount as new selling amount and refetch quotes
        selling_amount.set(current_buying_amount.clone());
        buying_amount.set("0.00".to_string());
        error_message.set(None);
        jupiter_order.set(None);
        dflow_quote.set(None);
        titan_quote.set(None);
        selected_provider.set(None);
        
        // Refetch quotes if there's an amount
        if !current_buying_amount.is_empty() && current_buying_amount != "0.00" {
            if let Ok(amount) = current_buying_amount.parse::<f64>() {
                if amount > 0.0 {
                    // Use tokens_clone_swap for this handler
                    let tokens = tokens_clone_swap.clone();
                    let amount_lamports = to_lamports(amount, &current_buying, &tokens);
                    
                    // Get mints before spawning
                    let input_mint = get_token_mint(&current_buying, &tokens).to_string();
                    let output_mint = get_token_mint(&current_selling, &tokens).to_string();
                    
                    // Get user pubkey inline (can't reuse get_user_pubkey closure)
                    let user_pubkey_result = if let Some(address) = hw_address() {
                        Some(address)
                    } else if let Some(wallet_info) = &wallet_clone_for_buying {
                        Some(wallet_info.address.clone())
                    } else {
                        None
                    };
                    
                    spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                        println!("🔄 Refetching quotes after swap...");
                        fetch_jupiter_order(input_mint.clone(), output_mint.clone(), amount_lamports, user_pubkey_result.clone());
                        fetch_dflow_quote(input_mint.clone(), output_mint.clone(), amount_lamports, 50);
                        fetch_titan_quotes(input_mint, output_mint, amount_lamports, user_pubkey_result);
                    });
                }
            }
        }
    };

    // Get token balances using cloned tokens
    let selling_balance = use_memo(move || {
        tokens_clone.iter()
            .find(|t| t.symbol == selling_token())
            .map(|t| t.balance)
            .unwrap_or(0.0)
    });

    let buying_balance = use_memo(move || {
        tokens_clone2.iter()
            .find(|t| t.symbol == buying_token())
            .map(|t| t.balance)
            .unwrap_or(0.0)
    });

    // Calculate USD values
    let selling_usd_value = use_memo(move || {
        if let Ok(amount) = selling_amount().parse::<f64>() {
            let price = tokens_clone_selling_usd.iter()
                .find(|t| t.symbol == selling_token())
                .map(|t| t.price)
                .unwrap_or(1.0);
            amount * price
        } else {
            0.0
        }
    });
    
    let buying_usd_value = use_memo(move || {
        if let Ok(amount) = buying_amount().parse::<f64>() {
            let price = tokens_clone_buying_usd.iter()
                .find(|t| t.symbol == buying_token())
                .map(|t| t.price)
                .unwrap_or(1.0);
            amount * price
        } else {
            0.0
        }
    });

    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| onclose.call(()),
            
            div {
                class: "modal-content swap-modal-v2",
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
                
                // Modal header
                div {
                    class: "swap-header-v2",
                    style: "
                        display: flex;
                        justify-content: space-between;
                        align-items: center;
                        padding: 24px;
                        border-bottom: none;
                        background: transparent;
                    ",
                    h2 { 
                        class: "swap-title-v2",
                        style: "
                            color: #f8fafc;
                            font-size: 22px;
                            font-weight: 700;
                            margin: 0;
                            letter-spacing: -0.025em;
                        ",
                        "Swap" 
                    }
                    button {
                        class: "swap-close-button-v2",
                        style: "
                            background: none;
                            border: none;
                            color: white;
                            font-size: 28px;
                            cursor: pointer;
                            padding: 0;
                            border-radius: 0;
                            transition: all 0.2s ease;
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
                        class: "error-message",
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
                
                // Selling section
                div {
                    class: "swap-section",
                    style: "padding: 16px 24px 12px;",
                    
                    div {
                        class: "swap-section-header",
                        style: "
                            display: flex;
                            justify-content: space-between;
                            align-items: center;
                            margin-bottom: 16px;
                        ",
                        span { 
                            style: "color: #94a3b8; font-size: 15px; font-weight: 500;",
                            "You're selling" 
                        }
                        span { 
                            class: "swap-balance",
                            style: "color: #cbd5e1; font-size: 13px;",
                            {format!("Balance: {:.6} {}", selling_balance(), selling_token())}
                        }
                    }
                    
                    div {
                        class: "swap-trading-row",
                        style: "
                            display: flex;
                            justify-content: space-between;
                            align-items: center;
                            background: #1a1a1a;
                            border: 1.5px solid #4a4a4a;
                            border-radius: 12px;
                            padding: 20px;
                            gap: 16px;
                            transition: border-color 0.2s ease;
                        ",
                        
                        // Token selector
                        div {
                            class: "swap-token-side",
                            style: "display: flex; align-items: center; gap: 12px; flex-shrink: 0;",
                            img {
                                class: "swap-token-icon",
                                style: "width: 32px; height: 32px; border-radius: 50%;",
                                src: get_token_icon(&selling_token(), &tokens_clone6),
                                alt: selling_token()
                            }
                            button {
                                class: "swap-token-picker",
                                style: "
                                    background: #2a2a2a;
                                    border: 1px solid #5a5a5a;
                                    border-radius: 10px;
                                    color: #ffffff;
                                    font-size: 17px;
                                    font-weight: 700;
                                    cursor: pointer;
                                    outline: none;
                                    padding: 10px 14px;
                                    min-height: 48px;
                                    display: inline-flex;
                                    align-items: center;
                                    gap: 8px;
                                ",
                                onclick: move |_| {
                                    sell_search_query.set("".to_string());
                                    show_sell_token_search.set(true);
                                },
                                span { "{selling_token()}" }
                                span { style: "opacity: 0.6; font-size: 14px;", "▾" }
                            }
                        }
                        
                        // Amount input
                        div {
                            class: "swap-amount-side",
                            style: "
                                display: flex;
                                flex-direction: column;
                                align-items: flex-end;
                                justify-content: center;
                                flex: 1;
                                min-width: 0;
                            ",
                            input {
                                class: "swap-amount-field",
                                style: "
                                    background: transparent;
                                    border: none;
                                    color: #ffffff;
                                    font-size: 28px;
                                    font-weight: 700;
                                    text-align: right;
                                    width: 100%;
                                    outline: none;
                                    padding: 0;
                                    margin: 0;
                                    min-height: 48px;
                                ",
                                r#type: "text",
                                inputmode: "decimal",
                                placeholder: "0.00",
                                value: selling_amount(),
                                oninput: move |e| handle_amount_change(e.value()),
                                disabled: swapping()
                            }
                            div {
                                class: "swap-amount-usd",
                                style: "
                                    color: #94a3b8;
                                    font-size: 14px;
                                    text-align: right;
                                    margin-top: 4px;
                                    font-weight: 500;
                                ",
                                "${selling_usd_value():.2}"
                            }
                        }
                    }
                }

                // Sell token search overlay
                if show_sell_token_search() {
                    div {
                        style: "
                            position: fixed;
                            inset: 0;
                            background: rgba(0, 0, 0, 0.6);
                            z-index: 9999;
                            display: flex;
                            align-items: flex-start;
                            justify-content: center;
                        ",
                        div {
                            style: "
                                width: 100%;
                                max-width: 560px;
                                background: #121212;
                                border-bottom-left-radius: 16px;
                                border-bottom-right-radius: 16px;
                                padding: 16px;
                                border: 1px solid #2a2a2a;
                                margin-top: 0;
                            ",
                            div {
                                style: "display: flex; align-items: center; justify-content: space-between; margin-bottom: 12px;",
                                div { style: "font-size: 16px; font-weight: 700; color: white;", "Sell token" }
                                button {
                                    style: "background: transparent; border: none; color: #9ca3af; font-size: 16px;",
                                    onclick: move |_| show_sell_token_search.set(false),
                                    "Close"
                                }
                            }
                            input {
                                style: "
                                    width: 100%;
                                    background: #1a1a1a;
                                    border: 1px solid #333;
                                    border-radius: 10px;
                                    padding: 12px 14px;
                                    color: white;
                                    font-size: 14px;
                                    margin-bottom: 12px;
                                ",
                                value: sell_search_query(),
                                placeholder: "Search your tokens",
                                oninput: move |e| sell_search_query.set(e.value()),
                            }
                            div { style: "max-height: 320px; overflow-y: auto;" ,
                                if sell_search_results().is_empty() {
                                    div { style: "color: #9ca3af; font-size: 13px; padding: 8px 0;", "No results" }
                                } else {
                                    for token in sell_search_results() {
                                        button {
                                            style: "width: 100%; background: #1a1a1a; border: 1px solid #2a2a2a; border-radius: 10px; padding: 10px 12px; margin-bottom: 8px; display: flex; align-items: center; gap: 10px; color: white; text-align: left;",
                                            onclick: {
                                                let update_selling_token = update_selling_token.clone();
                                                let symbol = token.symbol.clone();
                                                move |_| {
                                                    let mut handler = update_selling_token.borrow_mut();
                                                    handler(symbol.clone());
                                                }
                                            },
                                            img { src: "{token.icon_type}", style: "width: 28px; height: 28px; border-radius: 50%;" }
                                            div {
                                                div { style: "font-size: 14px; font-weight: 600;", "{token.symbol}" }
                                                div { style: "font-size: 11px; color: #9ca3af;", "{token.name}" }
                                            }
                                            div { style: "margin-left: auto; font-size: 11px; color: #6b7280;", {format!("{:.4}", token.balance)} }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Swap direction arrow
                div {
                    class: "swap-arrow-container",
                    style: "
                        display: flex;
                        justify-content: center;
                        margin: 12px 0;
                        position: relative;
                        z-index: 10;
                    ",
                    button {
                        class: "swap-arrow-button",
                        style: "
                            background: #3a3a3a;
                            border: 1.5px solid #5a5a5a;
                            border-radius: 50%;
                            min-width: 48px;
                            min-height: 48px;
                            color: #ffffff;
                            font-size: 20px;
                            cursor: pointer;
                            transition: all 0.2s ease;
                            display: flex;
                            align-items: center;
                            justify-content: center;
                            font-weight: bold;
                            box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
                        ",
                        onclick: handle_token_swap,
                        img {
                            src: "{ICON_SWITCH}",
                            alt: "Switch",
                            style: "width: 24px; height: 24px; transform: rotate(90deg); filter: brightness(0) invert(1);"
                        }
                    }
                }
                
                // Buying section
                div {
                    class: "swap-section",
                    style: "padding: 16px 24px 20px;",
                    
                    div {
                        class: "swap-section-header",
                        style: "
                            display: flex;
                            justify-content: space-between;
                            align-items: center;
                            margin-bottom: 16px;
                        ",
                        span { 
                            style: "color: #94a3b8; font-size: 15px; font-weight: 500;",
                            "You're buying" 
                        }
                        span { 
                            class: "swap-balance",
                            style: "color: #cbd5e1; font-size: 13px;",
                            {format!("Balance: {:.6} {}", buying_balance(), buying_token())}
                        }
                    }
                    
                    div {
                        class: "swap-trading-row",
                        style: "
                            display: flex;
                            justify-content: space-between;
                            align-items: center;
                            background: #1a1a1a;
                            border: 1.5px solid #4a4a4a;
                            border-radius: 12px;
                            padding: 20px;
                            gap: 16px;
                        ",
                        
                        // Token selector (buy side search)
                        div {
                            class: "swap-token-side",
                            style: "display: flex; align-items: center; gap: 12px; flex-shrink: 0;",
                            img {
                                class: "swap-token-icon",
                                style: "width: 32px; height: 32px; border-radius: 50%;",
                                src: get_token_icon_with_meta(&buying_token(), &tokens_clone6, buying_token_meta().as_ref()),
                                alt: buying_token()
                            }
                            button {
                                class: "swap-token-picker",
                                style: "
                                    background: #2a2a2a;
                                    border: 1px solid #5a5a5a;
                                    border-radius: 10px;
                                    color: #ffffff;
                                    font-size: 17px;
                                    font-weight: 700;
                                    cursor: pointer;
                                    outline: none;
                                    padding: 10px 14px;
                                    min-height: 48px;
                                    display: inline-flex;
                                    align-items: center;
                                    gap: 8px;
                                ",
                                onclick: move |_| {
                                    token_search_query.set("".to_string());
                                    show_buy_token_search.set(true);
                                },
                                span { "{buying_token()}" }
                                span { style: "opacity: 0.6; font-size: 14px;", "▾" }
                            }
                        }
                        
                        // Amount display (read-only)
                        div {
                            class: "swap-amount-side",
                            style: "
                                display: flex;
                                flex-direction: column;
                                align-items: flex-end;
                                justify-content: center;
                                flex: 1;
                                min-width: 0;
                            ",
                            div {
                                class: "swap-amount-field swap-amount-readonly",
                                style: "
                                    background: transparent;
                                    border: none;
                                    color: #10b981;
                                    font-size: 28px;
                                    font-weight: 700;
                                    text-align: right;
                                    width: 100%;
                                    min-height: 48px;
                                    display: flex;
                                    align-items: center;
                                    justify-content: flex-end;
                                ",
                                "{buying_amount()}"
                            }
                            div {
                                class: "swap-amount-usd",
                                style: "
                                    color: #94a3b8;
                                    font-size: 14px;
                                    text-align: right;
                                    margin-top: 4px;
                                    font-weight: 500;
                                ",
                                "${buying_usd_value():.2}"
                            }
                        }
                    }
                }

                // Buy token search overlay
                if show_buy_token_search() {
                    div {
                        style: "
                            position: fixed;
                            inset: 0;
                            background: rgba(0, 0, 0, 0.6);
                            z-index: 9999;
                            display: flex;
                            align-items: flex-start;
                            justify-content: center;
                        ",
                        div {
                            style: "
                                width: 100%;
                                max-width: 560px;
                                background: #121212;
                                border-bottom-left-radius: 16px;
                                border-bottom-right-radius: 16px;
                                padding: 16px;
                                border: 1px solid #2a2a2a;
                            ",
                            div {
                                style: "display: flex; align-items: center; justify-content: space-between; margin-bottom: 12px;",
                                div { style: "font-size: 16px; font-weight: 700; color: white;", "Select token" }
                                button {
                                    style: "background: transparent; border: none; color: #9ca3af; font-size: 16px;",
                                    onclick: move |_| show_buy_token_search.set(false),
                                    "Close"
                                }
                            }
                            input {
                                style: "
                                    width: 100%;
                                    background: #1a1a1a;
                                    border: 1px solid #333;
                                    border-radius: 10px;
                                    padding: 12px 14px;
                                    color: white;
                                    font-size: 14px;
                                    margin-bottom: 12px;
                                ",
                                value: token_search_query(),
                                placeholder: "Search name, symbol, or mint",
                                oninput: move |e| token_search_query.set(e.value()),
                            }

                            if token_catalog_loading() {
                                div { style: "color: #9ca3af; font-size: 13px; padding: 8px 0;", "Loading token list..." }
                            }

                            if token_search_query().is_empty() {
                                div { style: "color: #9ca3af; font-size: 12px; margin-bottom: 8px;", "Your tokens" }
                                div {
                                    style: "max-height: 320px; overflow-y: auto;",
                                    for token in tokens_clone6.iter() {
                                        button {
                                            style: "width: 100%; background: #1a1a1a; border: 1px solid #2a2a2a; border-radius: 10px; padding: 10px 12px; margin-bottom: 8px; display: flex; align-items: center; gap: 10px; color: white; text-align: left;",
                                            onclick: {
                                                let update_buying_token = update_buying_token.clone();
                                                let symbol = token.symbol.clone();
                                                move |_| {
                                                    let mut handler = update_buying_token.borrow_mut();
                                                    handler(symbol.clone(), None);
                                                }
                                            },
                                            img { src: "{token.icon_type}", style: "width: 28px; height: 28px; border-radius: 50%;" }
                                            div {
                                                div { style: "font-size: 14px; font-weight: 600;", "{token.symbol}" }
                                                div { style: "font-size: 11px; color: #9ca3af;", "{token.name}" }
                                            }
                                            div { style: "margin-left: auto; font-size: 11px; color: #6b7280;", {format!("{:.4}", token.balance)} }
                                        }
                                    }
                                }
                            } else {
                                div { style: "color: #9ca3af; font-size: 12px; margin-bottom: 8px;", "Search results" }
                                div {
                                    style: "max-height: 320px; overflow-y: auto;",
                                    {
                                        let query = token_search_query().trim().to_string();
                                        let is_mint_query = is_valid_mint(&query);
                                        let has_results = !token_search_results().is_empty();

                                        rsx! {
                                            if is_mint_query {
                                                button {
                                                    style: "width: 100%; background: #0f172a; border: 1px solid #1e293b; border-radius: 10px; padding: 10px 12px; margin-bottom: 8px; display: flex; align-items: center; gap: 10px; color: #e2e8f0; text-align: left;",
                                                    onclick: {
                                                        let update_buying_token = update_buying_token.clone();
                                                        let mint = query.clone();
                                                        let mut custom_token_loading = custom_token_loading.clone();
                                                        let mut custom_token_error = custom_token_error.clone();
                                                        move |_| {
                                                            custom_token_loading.set(true);
                                                            custom_token_error.set(None);
                                                            let mint_clone = mint.clone();
                                                            let update_buying_token = update_buying_token.clone();
                                                            spawn(async move {
                                                                let mut meta = None;
                                                                if let Ok(info_map) = prices::get_token_metadata(vec![mint_clone.clone()]).await {
                                                                    if let Some(info) = info_map.get(&mint_clone) {
                                                                        meta = Some(JupiterTokenMeta {
                                                                            address: info.id.clone(),
                                                                            symbol: info.symbol.clone(),
                                                                            name: info.name.clone(),
                                                                            decimals: info.decimals,
                                                                            logo_uri: info.icon.clone(),
                                                                        });
                                                                    }
                                                                }
                                                                let meta = meta.unwrap_or_else(|| JupiterTokenMeta {
                                                                    address: mint_clone.clone(),
                                                                    symbol: short_mint(&mint_clone),
                                                                    name: format!("Token {}", short_mint(&mint_clone)),
                                                                    decimals: 9,
                                                                    logo_uri: None,
                                                                });
                                                                custom_token_loading.set(false);
                                                                let mut handler = update_buying_token.borrow_mut();
                                                                handler(meta.symbol.clone(), Some(meta));
                                                            });
                                                        }
                                                    },
                                                    div {
                                                        div { style: "font-size: 13px; font-weight: 600;", "Use mint address" }
                                                        div { style: "font-size: 11px; color: #94a3b8;", "{short_mint(&query)}" }
                                                    }
                                                    div { style: "margin-left: auto; font-size: 11px; color: #38bdf8;", if custom_token_loading() { "Loading..." } else { "Select" } }
                                                }
                                                if let Some(err) = custom_token_error() {
                                                    div { style: "color: #f87171; font-size: 12px; padding: 4px 0;", "{err}" }
                                                }
                                            }

                                            if !has_results {
                                                div { style: "color: #9ca3af; font-size: 13px; padding: 8px 0;", "No results" }
                                            } else {
                                                for token in token_search_results() {
                                                    button {
                                                        style: "width: 100%; background: #1a1a1a; border: 1px solid #2a2a2a; border-radius: 10px; padding: 10px 12px; margin-bottom: 8px; display: flex; align-items: center; gap: 10px; color: white; text-align: left;",
                                                        onclick: {
                                                            let update_buying_token = update_buying_token.clone();
                                                            let token = token.clone();
                                                            move |_| {
                                                                let mut handler = update_buying_token.borrow_mut();
                                                                handler(token.symbol.clone(), Some(token.clone()));
                                                            }
                                                        },
                                                        img { src: "{token.logo_uri.clone().unwrap_or_else(|| ICON_32.to_string())}", style: "width: 28px; height: 28px; border-radius: 50%;" }
                                                        div {
                                                            div { style: "font-size: 14px; font-weight: 600;", "{token.symbol}" }
                                                            div { style: "font-size: 11px; color: #9ca3af;", "{token.name}" }
                                                        }
                                                        div { style: "margin-left: auto; font-size: 10px; color: #6b7280;", "{short_mint(&token.address)}" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                
                // Three-provider comparison
                if !selling_amount().is_empty() && selling_amount() != "0" {
                    div {
                        style: "padding: 0 24px; margin-bottom: 12px;",
                        div {
                            style: "display: flex; flex-direction: column; gap: 6px;",
                            div {
                                style: format!("background: linear-gradient(90deg, {} 0%, {} 100%); border-left: 4px solid {}; border-radius: 10px; padding: 12px 14px; transition: all 0.4s cubic-bezier(0.4, 0, 0.2, 1); display: flex; justify-content: space-between; align-items: center; cursor: pointer; {}",
                                    if selected_provider() == Some("Jupiter".to_string()) { "#1e3a5f" } else { "#1a1a1a" },
                                    if selected_provider() == Some("Jupiter".to_string()) { "#1a2942" } else { "#1a1a1a" },
                                    if selected_provider() == Some("Jupiter".to_string()) { "#3b82f6" } else { "transparent" },
                                    if selected_provider() == Some("Jupiter".to_string()) { "box-shadow: 0 8px 16px rgba(59,130,246,0.25); transform: scale(1.02);" } else { "" }
                                ),
                                onclick: move |_| {
                                    if jupiter_order().is_some() {
                                        selected_provider.set(Some("Jupiter".to_string()));
                                        println!("👆 User manually selected Jupiter");
                                    }
                                },
                                div { 
                                    style: format!("font-size: 13px; font-weight: 700; color: {};", 
                                        if selected_provider() == Some("Jupiter".to_string()) { "#60a5fa" } else { "#94a3b8" }
                                    ),
                                    "Jupiter"
                                }
                                if fetching_jupiter() { div { style: "height: 18px; width: 90px; background: linear-gradient(90deg, #2a2a2a 25%, #3a3a3a 50%, #2a2a2a 75%); background-size: 200% 100%; animation: shimmer 1.5s infinite; border-radius: 4px;", } }
                                else if let Some(order) = jupiter_order() {
                                    div {
                                        style: format!("color: {}; font-size: 14px; font-weight: 700;", if selected_provider() == Some("Jupiter".to_string()) { "#10b981" } else { "#e2e8f0" }),
                                        {format!("{:.6} {}", from_lamports_with_meta(order.out_amount.parse().unwrap_or(0), &buying_token(), &tokens_clone_price, buying_token_meta().as_ref()), buying_token())}
                                    }
                                }
                                else { div { style: "color: #64748b; font-size: 12px;", "..." } }
                            }
                            div {
                                style: format!("background: linear-gradient(90deg, {} 0%, {} 100%); border-left: 4px solid {}; border-radius: 10px; padding: 12px 14px; transition: all 0.4s cubic-bezier(0.4, 0, 0.2, 1); display: flex; justify-content: space-between; align-items: center; cursor: pointer; {}",
                                    if selected_provider() == Some("Dflow".to_string()) { "#2d1b4e" } else { "#1a1a1a" },
                                    if selected_provider() == Some("Dflow".to_string()) { "#1f1335" } else { "#1a1a1a" },
                                    if selected_provider() == Some("Dflow".to_string()) { "#8b5cf6" } else { "transparent" },
                                    if selected_provider() == Some("Dflow".to_string()) { "box-shadow: 0 8px 16px rgba(139,92,246,0.25); transform: scale(1.02);" } else { "" }
                                ),
                                onclick: move |_| {
                                    if dflow_quote().is_some() {
                                        selected_provider.set(Some("Dflow".to_string()));
                                        println!("👆 User manually selected Dflow");
                                    }
                                },
                                div { 
                                    style: format!("font-size: 13px; font-weight: 700; color: {};", 
                                        if selected_provider() == Some("Dflow".to_string()) { "#a78bfa" } else { "#94a3b8" }
                                    ),
                                    "Dflow"
                                }
                                if fetching_dflow() { div { style: "height: 18px; width: 90px; background: linear-gradient(90deg, #2a2a2a 25%, #3a3a3a 50%, #2a2a2a 75%); background-size: 200% 100%; animation: shimmer 1.5s infinite; border-radius: 4px;", } }
                                else if let Some(quote) = dflow_quote() {
                                    div {
                                        style: format!("color: {}; font-size: 14px; font-weight: 700;", if selected_provider() == Some("Dflow".to_string()) { "#10b981" } else { "#e2e8f0" }),
                                        {format!("{:.6} {}", from_lamports_with_meta(quote.out_amount.parse().unwrap_or(0), &buying_token(), &tokens_clone_price, buying_token_meta().as_ref()), buying_token())}
                                    }
                                }
                                else { div { style: "color: #64748b; font-size: 12px;", "..." } }
                            }
                            div {
                                style: format!("background: linear-gradient(90deg, {} 0%, {} 100%); border-left: 4px solid {}; border-radius: 10px; padding: 12px 14px; transition: all 0.4s cubic-bezier(0.4, 0, 0.2, 1); display: flex; justify-content: space-between; align-items: center; cursor: pointer; {}",
                                    if selected_provider() == Some("Titan".to_string()) { "#3d2817" } else { "#1a1a1a" },
                                    if selected_provider() == Some("Titan".to_string()) { "#2d1f12" } else { "#1a1a1a" },
                                    if selected_provider() == Some("Titan".to_string()) { "#f59e0b" } else { "transparent" },
                                    if selected_provider() == Some("Titan".to_string()) { "box-shadow: 0 8px 16px rgba(245,158,11,0.25); transform: scale(1.02);" } else { "" }
                                ),
                                onclick: move |_| {
                                    if titan_quote().is_some() {
                                        selected_provider.set(Some("Titan".to_string()));
                                        println!("👆 User manually selected Titan");
                                    }
                                },
                                div { 
                                    style: format!("font-size: 13px; font-weight: 700; color: {};", 
                                        if selected_provider() == Some("Titan".to_string()) { "#fbbf24" } else { "#94a3b8" }
                                    ),
                                    "Titan"
                                }
                                if fetching_titan() { div { style: "height: 18px; width: 90px; background: linear-gradient(90deg, #2a2a2a 25%, #3a3a3a 50%, #2a2a2a 75%); background-size: 200% 100%; animation: shimmer 1.5s infinite; border-radius: 4px;", } }
                                else if let Some((_, route)) = titan_quote() {
                                    div {
                                        style: format!("color: {}; font-size: 14px; font-weight: 700;", if selected_provider() == Some("Titan".to_string()) { "#10b981" } else { "#e2e8f0" }),
                                        {format!("{:.6} {}", from_lamports_with_meta(route.out_amount, &buying_token(), &tokens_clone_price, buying_token_meta().as_ref()), buying_token())}
                                    }
                                }
                                else { div { style: "color: #64748b; font-size: 12px;", "..." } }
                            }
                        }
                    }
                }
                

                
                // Action button
                div {
                    class: "modal-buttons",
                    style: "
                        display: flex;
                        padding: 0 24px 28px;
                    ",
                    button {
                        class: "button-standard primary",
                        style: "
                            width: 100%;
                            padding: 18px 24px;
                            border-radius: 12px;
                            border: none;
                            cursor: pointer;
                            font-size: 16px;
                            font-weight: 700;
                            text-transform: uppercase;
                            letter-spacing: 0.5px;
                            transition: all 0.2s ease;
                            background: white;
                            color: #1a1a1a;
                            min-height: 56px;
                            box-shadow: 0 4px 12px rgba(255, 255, 255, 0.2);
                        ",
                        disabled: swapping() || selling_amount().is_empty() || fetching_jupiter(),
                        onclick: handle_swap,
                        
                        if fetching_jupiter() {
                            "Getting Quote..."
                        } else if swapping() {
                            "Swapping..."
                        } else {
                            "Swap"
                        }
                    }
                }
                

            }
        }
    }
}
