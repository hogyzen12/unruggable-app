use dioxus::prelude::*;
use crate::wallet::WalletInfo;
use crate::hardware::HardwareWallet;
use crate::components::common::Token;
use crate::transaction::TransactionClient;
use crate::signing::hardware::HardwareSigner;
use crate::signing::software::SoftwareSigner;
use crate::signing::TransactionSigner;
use crate::wallet::Wallet;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use solana_sdk::transaction::VersionedTransaction;

/// Sign a Jupiter Ultra transaction using the provided signer
async fn sign_jupiter_transaction(
    signer: &dyn TransactionSigner,
    unsigned_transaction_b64: &str,
) -> Result<String, String> {
    println!("🔐 Signing Jupiter Ultra transaction...");
    
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
    
    println!("📋 Transaction has {} signatures expected", transaction.signatures.len());
    
    // Serialize the transaction message for signing
    let message_bytes = transaction.message.serialize();
    println!("📝 Message to sign: {} bytes", message_bytes.len());
    
    // Sign the message
    let signature_bytes = match signer.sign_message(&message_bytes).await {
        Ok(sig) => sig,
        Err(e) => return Err(format!("Failed to sign message: {}", e)),
    };
    
    // Ensure we have exactly 64 bytes for the signature
    if signature_bytes.len() != 64 {
        return Err(format!("Invalid signature length: expected 64, got {}", signature_bytes.len()));
    }
    
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

// Jupiter Ultra API Types
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UltraOrderResponse {
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
pub struct UltraExecuteRequest {
    #[serde(rename = "signedTransaction")]
    pub signed_transaction: String,
    #[serde(rename = "requestId")]
    pub request_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UltraExecuteResponse {
    pub status: String, // "Success" or "Failed"
    pub signature: Option<String>,
    pub error: Option<String>,
}

// Keep original Jupiter v1 types for fallback
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QuoteResponse {
    #[serde(rename = "inputMint")]
    pub input_mint: String,
    #[serde(rename = "inAmount")]
    pub in_amount: String,
    #[serde(rename = "outputMint")]
    pub output_mint: String,
    #[serde(rename = "outAmount")]
    pub out_amount: String,
    #[serde(rename = "otherAmountThreshold")]
    pub other_amount_threshold: String,
    #[serde(rename = "swapMode")]
    pub swap_mode: String,
    #[serde(rename = "slippageBps")]
    pub slippage_bps: u16,
    #[serde(rename = "priceImpactPct")]
    pub price_impact_pct: String,
    #[serde(rename = "routePlan")]
    pub route_plan: Vec<serde_json::Value>,
}

// Token mint addresses
fn get_token_mint(symbol: &str) -> &'static str {
    match symbol {
        "SOL" => "So11111111111111111111111111111111111111112",
        "USDC" => "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
        "USDT" => "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB",
        "JUP" => "JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN",
        "BONK" => "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263",
        "JTO" => "jtojtomepa8beP8AuQc6eXt5FriJwfFMwQx2v2f9mCL",
        "JLP" => "27G8MtK7VtTcCHkpASjSDdkWWYfoqT6ggEuKidVJidD4",
        _ => "So11111111111111111111111111111111111111112",
    }
}

// Get token decimals for proper amount conversion
fn get_token_decimals(symbol: &str) -> u8 {
    match symbol {
        "USDC" | "USDT" => 6,  // USDC and USDT have 6 decimals
        "BONK" => 5,           // BONK has 5 decimals  
        _ => 9,                // SOL, JUP, JTO, JLP have 9 decimals
    }
}

// Convert human-readable amount to lamports/smallest unit
fn to_lamports(amount: f64, symbol: &str) -> u64 {
    let decimals = get_token_decimals(symbol);
    (amount * 10_f64.powi(decimals as i32)) as u64
}

// Convert lamports/smallest unit to human-readable amount  
fn from_lamports(lamports: u64, symbol: &str) -> f64 {
    let decimals = get_token_decimals(symbol);
    lamports as f64 / 10_f64.powi(decimals as i32)
}

// Token icons
const ICON_SOL: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/solana-mobile@main/assets/icons/solanaLogo.png";
const ICON_USDC: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/solana-mobile@main/assets/icons/usdcLogo.png";
const ICON_USDT: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/solana-mobile@main/assets/icons/usdtLogo.png";
const ICON_JTO: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/solana-mobile@main/assets/icons/jtoLogo.png";
const ICON_JUP: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/solana-mobile@main/assets/icons/jupLogo.png";
const ICON_JLP: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/solana-mobile@main/assets/icons/jlpLogo.png";
const ICON_BONK: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/solana-mobile@main/assets/icons/bonkLogo.png";
const ICON_32: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/solana-mobile@main/assets/icons/32x32.png";

fn get_token_icon(symbol: &str) -> &'static str {
    match symbol {
        "SOL" => ICON_SOL,
        "USDC" => ICON_USDC,
        "USDT" => ICON_USDT,
        "JTO" => ICON_JTO,
        "JUP" => ICON_JUP,
        "JLP" => ICON_JLP,
        "BONK" => ICON_BONK,
        _ => ICON_32,
    }
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
    // Explorer links for multiple explorers
    let solana_explorer_url = format!("https://explorer.solana.com/tx/{}", signature);
    let solscan_url = format!("https://solscan.io/tx/{}", signature);
    let solana_fm_url = format!("https://solana.fm/tx/{}", signature);
    
    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| onclose.call(()),
            
            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                
                h2 { class: "modal-title", "Swap Completed Successfully! 🎉" }
                
                div {
                    class: "tx-icon-container",
                    div {
                        class: "tx-success-icon",
                        "✓" // Checkmark icon
                    }
                }
                
                div {
                    class: "success-message",
                    "Your swap transaction was submitted to the Solana network."
                }
                
                div {
                    class: "swap-summary",
                    div {
                        class: "swap-summary-row",
                        span { "Sold:" }
                        span { "{selling_amount} {selling_token}" }
                    }
                    div {
                        class: "swap-summary-row",
                        span { "Received:" }
                        span { "~{buying_amount} {buying_token}" }
                    }
                }
                
                // Add hardware wallet reconnection notice if this was a hardware wallet transaction
                if was_hardware_wallet {
                    div {
                        class: "hardware-reconnect-notice",
                        "Your hardware wallet has been disconnected after the transaction. You'll need to reconnect it for future swaps."
                    }
                }
                
                div {
                    class: "transaction-details",
                    div {
                        class: "wallet-field",
                        label { "Transaction Signature:" }
                        div { 
                            class: "address-display", 
                            title: "Click to copy",
                            onclick: move |_| {
                                // We can't do actual clipboard operations in Dioxus yet
                                // This is just for UI indication
                                log::info!("Signature copied to clipboard: {}", signature);
                            },
                            "{signature}"
                        }
                        div { 
                            class: "copy-hint",
                            "Click to copy"
                        }
                    }
                    
                    div {
                        class: "explorer-links",
                        p { "View transaction in explorer:" }
                        
                        div {
                            class: "explorer-buttons",
                            a {
                                class: "explorer-button",
                                href: "{solana_explorer_url}",
                                target: "_blank",
                                rel: "noopener noreferrer",
                                "Solana Explorer"
                            }
                            a {
                                class: "explorer-button",
                                href: "{solscan_url}",
                                target: "_blank",
                                rel: "noopener noreferrer",
                                "Solscan"
                            }
                            a {
                                class: "explorer-button",
                                href: "{solana_fm_url}",
                                target: "_blank",
                                rel: "noopener noreferrer",
                                "Solana FM"
                            }
                        }
                    }
                }
                
                div { class: "modal-buttons",
                    button {
                        class: "modal-button primary",
                        onclick: move |_| onclose.call(()),
                        "Close"
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
    
    // State management
    let mut selling_token = use_signal(|| "SOL".to_string());
    let mut buying_token = use_signal(|| "USDC".to_string());
    let mut selling_amount = use_signal(|| "".to_string());
    let mut buying_amount = use_signal(|| "0.00".to_string());
    let mut swapping = use_signal(|| false);
    let mut error_message = use_signal(|| None as Option<String>);

    // State for transaction success modal
    let mut show_success_modal = use_signal(|| false);
    let mut transaction_signature = use_signal(|| "".to_string());
    let mut was_hardware_transaction = use_signal(|| false);
    let mut show_hardware_approval = use_signal(|| false);

    // Jupiter Ultra API state
    let mut current_order = use_signal(|| None as Option<UltraOrderResponse>);
    let mut fetching_order = use_signal(|| false);

    // Clone tokens for closures - need separate clones for each closure
    let tokens_clone = tokens.clone();
    let tokens_clone2 = tokens.clone();
    let tokens_clone3 = tokens.clone();
    let tokens_clone4 = tokens.clone(); // For handle_amount_change

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
    let hardware_wallet_clone2 = hardware_wallet.clone(); 
    let wallet_clone2 = wallet.clone();

    // Get user public key
    let get_user_pubkey = move || -> Option<String> {
        if let Some(wallet_info) = &wallet_clone {
            Some(wallet_info.address.clone())
        } else if let Some(_hw) = &hardware_wallet_clone {
            Some("HARDWARE_WALLET_PUBKEY".to_string()) // TODO: Get real hardware wallet pubkey
        } else {
            None
        }
    };

    // Jupiter Ultra API: Fetch order with better error handling
    let fetch_jupiter_ultra_order = move |input_mint: String, output_mint: String, amount_lamports: u64, user_pubkey: Option<String>| {
        spawn(async move {
            // Prevent multiple simultaneous requests
            if fetching_order() {
                return;
            }
            
            fetching_order.set(true);
            error_message.set(None);
            
            let client = reqwest::Client::new();
            
            // Build query parameters
            let mut url = format!(
                "https://lite-api.jup.ag/ultra/v1/order?inputMint={}&outputMint={}&amount={}",
                input_mint, output_mint, amount_lamports
            );
            
            // Add taker (user pubkey) if available for unsigned transaction
            if let Some(pubkey) = user_pubkey {
                url.push_str(&format!("&taker={}", pubkey));
            }
            
            println!("🚀 Fetching Jupiter Ultra order: {}", url);
            
            match client.get(&url).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        match response.json::<UltraOrderResponse>().await {
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
                                    // Log transaction details for debugging
                                    println!("📃 Request ID: {}", order.request_id);
                                    println!("📃 Router: {}", order.router);
                                    if let Some(price_impact) = order.price_impact {
                                        println!("📃 Price Impact: {:.4}%", price_impact);
                                    }
                                    println!("📃 Fee BPS: {}", order.fee_bps);
                                    
                                    if let Some(tx) = &order.transaction {
                                        if !tx.is_empty() {
                                            println!("📃 ✅ Transaction ready for signing ({} chars)", tx.len());
                                        } else {
                                            println!("📃 ⚠️ Empty transaction - may need balance or different route");
                                        }
                                    }
                                    
                                    current_order.set(Some(order));
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
            
            fetching_order.set(false);
        });
    };

    // Jupiter Ultra API: Execute transaction - REAL IMPLEMENTATION
    let execute_jupiter_ultra_swap = move |order: UltraOrderResponse, signed_transaction: String| {
        spawn(async move {
            let client = reqwest::Client::new();
            
            let execute_request = UltraExecuteRequest {
                signed_transaction,
                request_id: order.request_id.clone(),
            };
            
            println!("🚀 Executing Jupiter Ultra swap with requestId: {}", order.request_id);
            println!("🔄 Signed transaction length: {} chars", execute_request.signed_transaction.len());
            
            match client
                .post("https://lite-api.jup.ag/ultra/v1/execute")
                .json(&execute_request)
                .send()
                .await 
            {
                Ok(response) => {
                    let status_code = response.status();
                    println!("📡 Jupiter Ultra execute response status: {}", status_code);
                    
                    // Get response text first to debug
                    match response.text().await {
                        Ok(response_text) => {
                            println!("📄 Raw execute response: {}", response_text);
                            
                            if status_code.is_success() {
                                // Try to parse as the expected UltraExecuteResponse format
                                match serde_json::from_str::<UltraExecuteResponse>(&response_text) {
                                    Ok(execute_response) => {
                                        match execute_response.status.as_str() {
                                            "Success" => {
                                                if let Some(signature) = execute_response.signature {
                                                    println!("✅ Jupiter Ultra swap executed successfully! Signature: {}", signature);
                                                    transaction_signature.set(signature);
                                                    swapping.set(false);
                                                    show_success_modal.set(true);
                                                } else {
                                                    println!("⚠️ Swap completed but no signature returned");
                                                    swapping.set(false);
                                                    error_message.set(Some("Swap completed but no transaction signature received".to_string()));
                                                }
                                            }
                                            "Failed" => {
                                                let error_msg = execute_response.error.unwrap_or("Unknown error".to_string());
                                                println!("❌ Jupiter Ultra swap failed: {}", error_msg);
                                                swapping.set(false);
                                                error_message.set(Some(format!("Swap failed: {}", error_msg)));
                                            }
                                            _ => {
                                                println!("⚠️ Unknown swap status: {}", execute_response.status);
                                                swapping.set(false);
                                                error_message.set(Some(format!("Unknown swap status: {}", execute_response.status)));
                                            }
                                        }
                                    }
                                    Err(_) => {
                                        // If parsing as UltraExecuteResponse fails, try to parse as a generic response
                                        // Sometimes Jupiter might return just a transaction signature directly
                                        if response_text.len() == 64 || response_text.len() == 88 {
                                            // Looks like a transaction signature
                                            println!("✅ Received transaction signature: {}", response_text);
                                            transaction_signature.set(response_text);
                                            swapping.set(false);
                                            show_success_modal.set(true);
                                        } else {
                                            println!("❌ Failed to parse execute response format");
                                            println!("📄 Response was: {}", response_text);
                                            swapping.set(false);
                                            error_message.set(Some("Unexpected response format from Jupiter".to_string()));
                                        }
                                    }
                                }
                            } else {
                                // Handle error responses
                                println!("❌ Jupiter Ultra execute failed with status: {}", status_code);
                                
                                // Try to parse error details
                                if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&response_text) {
                                    if let Some(error_msg) = error_json.get("error").and_then(|e| e.as_str()) {
                                        println!("❌ Error details: {}", error_msg);
                                        swapping.set(false);
                                        error_message.set(Some(format!("Swap failed: {}", error_msg)));
                                    } else {
                                        swapping.set(false);
                                        error_message.set(Some(format!("Swap failed with status: {}", status_code)));
                                    }
                                } else {
                                    swapping.set(false);
                                    error_message.set(Some(format!("Swap failed with status: {}", status_code)));
                                }
                            }
                        }
                        Err(e) => {
                            println!("❌ Failed to read response text: {}", e);
                            swapping.set(false);
                            error_message.set(Some("Network error during swap execution".to_string()));
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Jupiter Ultra execute request failed: {}", e);
                    swapping.set(false);
                    error_message.set(Some("Network error during swap execution".to_string()));
                }
            }
        });
    };

    // Token price lookup for USD calculations
    let get_token_price_usd = move |symbol: &str| -> f64 {
        match symbol {
            "SOL" => 184.83,
            "USDC" => 1.0,
            "USDT" => 1.0,
            "JUP" => 0.85,
            "BONK" => 0.000025,
            "JTO" => 2.45,
            "JLP" => 3.12,
            _ => 1.0,
        }
    };

    // Calculate exchange rate for fallback display
    let exchange_rate = use_memo(move || {
        let selling_price = get_token_price_usd(&selling_token());
        let buying_price = get_token_price_usd(&buying_token());
        
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
        current_order.set(None); // Clear previous order immediately
        
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
                
                // Only fetch Jupiter order if we have sufficient balance
                if amount <= selling_balance && amount > 0.0 {
                    let amount_lamports = to_lamports(amount, &selling_token());
                    
                    let input_mint = get_token_mint(&selling_token()).to_string();
                    let output_mint = get_token_mint(&buying_token()).to_string();
                    let user_pubkey = get_user_pubkey();
                    
                    // Add small delay to prevent too many API calls
                    spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                        fetch_jupiter_ultra_order(input_mint, output_mint, amount_lamports, user_pubkey);
                    });
                }
            }
        } else {
            buying_amount.set("0.00".to_string());
            current_order.set(None);
        }
    };

    // Update buying amount when Jupiter Ultra order changes
    use_effect(move || {
        if let Some(order) = current_order() {
            let output_amount = order.out_amount.parse::<u64>().unwrap_or(0);
            let converted_amount = from_lamports(output_amount, &buying_token());
            
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

                // Check if we have a valid Jupiter Ultra order
                if let Some(order) = current_order() {
                    // Check for order-level errors
                    if let Some(error_msg) = &order.error_message {
                        error_message.set(Some(format!("Cannot swap: {}", error_msg)));
                        return;
                    }

                    if let Some(unsigned_tx) = &order.transaction {
                        if unsigned_tx.is_empty() {
                            error_message.set(Some("No transaction available - insufficient balance or liquidity".to_string()));
                            return;
                        }
                        
                        println!("✅ Using Jupiter Ultra order for swap");
                        println!("📄 Transaction to sign: {} chars", unsigned_tx.len());
                        swapping.set(true);
                        error_message.set(None);
                        
                        // Clone values for the async block
                        let order_clone = order.clone();
                        let unsigned_tx_clone = unsigned_tx.clone();
                        let hw_clone = hardware_wallet_clone2.clone();
                        let wallet_info_clone = wallet_clone2.clone();
                        
                        // Real transaction signing and execution
                        spawn(async move {
                            // Determine if this is a hardware wallet transaction
                            let is_hardware = hw_clone.is_some();
                            was_hardware_transaction.set(is_hardware);
                            
                            if is_hardware {
                                show_hardware_approval.set(true);
                            }
                            
                            println!("🔐 Starting real transaction signing...");
                            println!("📄 Unsigned transaction: {} chars", unsigned_tx_clone.len());
                            
                            // Create the appropriate signer
                            let signing_result = if let Some(hw) = hw_clone {
                                println!("💻 Using hardware wallet signer");
                                let hw_signer = HardwareSigner::from_wallet(hw);
                                sign_jupiter_transaction(&hw_signer, &unsigned_tx_clone).await
                            } else if let Some(wallet_info) = wallet_info_clone {
                                println!("🔑 Using software wallet signer");
                                match Wallet::from_wallet_info(&wallet_info) {
                                    Ok(wallet) => {
                                        let sw_signer = SoftwareSigner::new(wallet);
                                        sign_jupiter_transaction(&sw_signer, &unsigned_tx_clone).await
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
                                Ok(signed_transaction) => {
                                    println!("✅ Transaction signed successfully! Length: {} chars", signed_transaction.len());
                                    println!("🚀 Executing real signed transaction...");
                                    execute_jupiter_ultra_swap(order_clone, signed_transaction);
                                }
                                Err(e) => {
                                    println!("❌ Transaction signing failed: {}", e);
                                    swapping.set(false);
                                    error_message.set(Some(format!("Failed to sign transaction: {}", e)));
                                }
                            }
                        });
                    } else {
                        error_message.set(Some("No transaction data available".to_string()));
                        swapping.set(false);
                    }
                } else {
                    error_message.set(Some("No quote available - please wait or try a different amount".to_string()));
                }
            }
        }
    };

    // Handle token swap direction
    let handle_token_swap = move |_| {
        println!("🔄 Token swap direction clicked!");
        let current_selling = selling_token();
        let current_buying = buying_token();
        selling_token.set(current_buying);
        buying_token.set(current_selling);
        
        // Clear amounts when swapping tokens
        selling_amount.set("".to_string());
        buying_amount.set("0.00".to_string());
        error_message.set(None);
        current_order.set(None);
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
            let price = get_token_price_usd(&selling_token());
            amount * price
        } else {
            0.0
        }
    });

    let buying_usd_value = use_memo(move || {
        if let Ok(amount) = buying_amount().parse::<f64>() {
            let price = get_token_price_usd(&buying_token());
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
                    background: linear-gradient(135deg, #1e293b 0%, #0f172a 100%);
                    border-radius: 24px;
                    padding: 0;
                    width: 340px;
                    max-width: 95vw;
                    box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.25);
                    border: 1px solid rgba(148, 163, 184, 0.1);
                    overflow: hidden;
                    margin: 0 auto;
                ",
                
                // Modal header
                div { 
                    class: "swap-header-v2",
                    style: "
                        display: flex;
                        justify-content: space-between;
                        align-items: center;
                        padding: 20px 24px 16px;
                        border-bottom: 1px solid rgba(148, 163, 184, 0.1);
                    ",
                    h2 { 
                        class: "swap-title-v2",
                        style: "
                            color: #f8fafc;
                            font-size: 20px;
                            font-weight: 600;
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
                            color: #94a3b8;
                            font-size: 24px;
                            cursor: pointer;
                            padding: 4px;
                            border-radius: 8px;
                            transition: all 0.2s ease;
                            width: 32px;
                            height: 32px;
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
                            padding: 12px;
                            background-color: #7f1d1d;
                            border: 1px solid #dc2626;
                            color: #fca5a5;
                            border-radius: 8px;
                            margin: 16px 24px;
                            font-size: 14px;
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
                            margin-bottom: 12px;
                        ",
                        span { 
                            style: "color: #94a3b8; font-size: 14px;",
                            "You're selling" 
                        }
                        span { 
                            class: "swap-balance",
                            style: "color: #cbd5e1; font-size: 12px;",
                            "Balance: {selling_balance():.6} {selling_token()}"
                        }
                    }
                    
                    div {
                        class: "swap-trading-row",
                        style: "
                            display: flex;
                            justify-content: space-between;
                            align-items: center;
                            background: rgba(15, 23, 42, 0.6);
                            border: 1px solid rgba(148, 163, 184, 0.15);
                            border-radius: 12px;
                            padding: 18px 20px;
                            gap: 20px;
                        ",
                        
                        // Token selector
                        div {
                            class: "swap-token-side",
                            style: "display: flex; align-items: center; gap: 8px;",
                            img {
                                class: "swap-token-icon",
                                style: "width: 24px; height: 24px; border-radius: 50%;",
                                src: get_token_icon(&selling_token()),
                                alt: selling_token()
                            }
                            select {
                                class: "swap-token-picker",
                                style: "
                                    background: transparent;
                                    border: none;
                                    color: #ffffff;
                                    font-size: 16px;
                                    font-weight: 600;
                                    cursor: pointer;
                                    outline: none;
                                ",
                                value: selling_token(),
                                onchange: move |e| {
                                    selling_token.set(e.value());
                                    selling_amount.set("".to_string());
                                    buying_amount.set("0.00".to_string());
                                    current_order.set(None);
                                },
                                
                                option { value: "SOL", "SOL" }
                                option { value: "USDC", "USDC" }
                                option { value: "USDT", "USDT" }
                                option { value: "JUP", "JUP" }
                                option { value: "BONK", "BONK" }
                                option { value: "JTO", "JTO" }
                                option { value: "JLP", "JLP" }
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
                                text-align: right;
                            ",
                            input {
                                class: "swap-amount-field",
                                style: "
                                    background: transparent;
                                    border: none;
                                    color: #ffffff;
                                    font-size: 24px;
                                    font-weight: 700;
                                    text-align: right;
                                    width: 100%;
                                    outline: none;
                                    padding: 0;
                                    margin: 0;
                                ",
                                r#type: "text",
                                placeholder: "0.00",
                                value: selling_amount(),
                                oninput: move |e| handle_amount_change(e.value()),
                                disabled: swapping()
                            }
                            div {
                                class: "swap-amount-usd",
                                style: "
                                    color: #9ca3af;
                                    font-size: 12px;
                                    text-align: right;
                                    margin-top: 2px;
                                ",
                                "${selling_usd_value():.2}"
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
                        margin: 8px 0;
                        position: relative;
                        z-index: 10;
                    ",
                    button {
                        class: "swap-arrow-button",
                        style: "
                            background-color: #374151;
                            border: 2px solid #4b5563;
                            border-radius: 50%;
                            width: 44px;
                            height: 44px;
                            color: #ffffff;
                            font-size: 18px;
                            cursor: pointer;
                            transition: all 0.2s ease;
                            display: flex;
                            align-items: center;
                            justify-content: center;
                            font-weight: bold;
                        ",
                        onclick: handle_token_swap,
                        "↕"
                    }
                }
                
                // Buying section
                div {
                    class: "swap-section",
                    style: "padding: 16px 24px 12px;",
                    
                    div {
                        class: "swap-section-header",
                        style: "
                            display: flex;
                            justify-content: space-between;
                            align-items: center;
                            margin-bottom: 12px;
                        ",
                        span { 
                            style: "color: #94a3b8; font-size: 14px;",
                            "You're buying" 
                        }
                        span { 
                            class: "swap-balance",
                            style: "color: #cbd5e1; font-size: 12px;",
                            "Balance: {buying_balance():.6} {buying_token()}"
                        }
                    }
                    
                    div {
                        class: "swap-trading-row",
                        style: "
                            display: flex;
                            justify-content: space-between;
                            align-items: center;
                            background: rgba(15, 23, 42, 0.6);
                            border: 1px solid rgba(148, 163, 184, 0.15);
                            border-radius: 12px;
                            padding: 18px 20px;
                            gap: 20px;
                        ",
                        
                        // Token selector
                        div {
                            class: "swap-token-side",
                            style: "display: flex; align-items: center; gap: 8px;",
                            img {
                                class: "swap-token-icon",
                                style: "width: 24px; height: 24px; border-radius: 50%;",
                                src: get_token_icon(&buying_token()),
                                alt: buying_token()
                            }
                            select {
                                class: "swap-token-picker",
                                style: "
                                    background: transparent;
                                    border: none;
                                    color: #ffffff;
                                    font-size: 16px;
                                    font-weight: 600;
                                    cursor: pointer;
                                    outline: none;
                                ",
                                value: buying_token(),
                                onchange: move |e| {
                                    buying_token.set(e.value());
                                    buying_amount.set("0.00".to_string());
                                    current_order.set(None);
                                },
                                
                                option { value: "USDC", "USDC" }
                                option { value: "USDT", "USDT" }
                                option { value: "SOL", "SOL" }
                                option { value: "JUP", "JUP" }
                                option { value: "BONK", "BONK" }
                                option { value: "JTO", "JTO" }
                                option { value: "JLP", "JLP" }
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
                                text-align: right;
                            ",
                            input {
                                class: "swap-amount-field swap-amount-readonly",
                                style: "
                                    background: transparent;
                                    border: none;
                                    color: #9ca3af;
                                    font-size: 24px;
                                    font-weight: 700;
                                    text-align: right;
                                    width: 100%;
                                    outline: none;
                                    padding: 0;
                                    margin: 0;
                                    cursor: not-allowed;
                                ",
                                r#type: "text",
                                value: buying_amount(),
                                readonly: true
                            }
                            div {
                                class: "swap-amount-usd",
                                style: "
                                    color: #9ca3af;
                                    font-size: 12px;
                                    text-align: right;
                                    margin-top: 2px;
                                ",
                                "${buying_usd_value():.2}"
                            }
                        }
                    }
                }
                
                // Rate information
                div {
                    class: "swap-rate-section",
                    style: "
                        background-color: #0f1419;
                        border-radius: 8px;
                        border: 1px solid #2a2a2a;
                        padding: 12px 16px;
                        margin: 16px 24px;
                    ",
                    div {
                        class: "swap-rate-row",
                        style: "color: #9ca3af; font-size: 14px; margin-bottom: 4px;",
                        {
                            if fetching_order() {
                                "Getting best rate...".to_string()
                            } else if let Some(order) = current_order() {
                                let input_amount = order.in_amount.parse::<u64>().unwrap_or(0);
                                let output_amount = order.out_amount.parse::<u64>().unwrap_or(0);
                                
                                let input_converted = from_lamports(input_amount, &selling_token());
                                let output_converted = from_lamports(output_amount, &buying_token());
                                
                                let rate = if input_converted > 0.0 { output_converted / input_converted } else { 0.0 };
                                
                                let formatted_rate = if rate < 0.01 {
                                    format!("{:.6}", rate)
                                } else {
                                    format!("{:.4}", rate)
                                };
                                
                                format!("Rate: 1 {} = {} {} (Jupiter Ultra)", selling_token(), formatted_rate, buying_token())
                            } else {
                                let rate = exchange_rate();
                                let formatted_rate = if rate < 0.01 {
                                    format!("{:.6}", rate)
                                } else {
                                    format!("{:.4}", rate)
                                };
                                format!("Rate: 1 {} = {} {}", selling_token(), formatted_rate, buying_token())
                            }
                        }
                    }
                    
                    // Show additional Jupiter Ultra info if available
                    if let Some(order) = current_order() {
                        div {
                            class: "swap-rate-row",
                            style: "color: #9ca3af; font-size: 14px; margin-bottom: 4px;",
                            "Route: {order.router}"
                        }
                        if let Some(price_impact) = order.price_impact {
                            div {
                                class: "swap-rate-row",
                                style: "color: #9ca3af; font-size: 14px; margin-bottom: 4px;",
                                "Price Impact: {price_impact:.4}%"
                            }
                        }
                        div {
                            class: "swap-rate-row",
                            style: "color: #9ca3af; font-size: 14px; margin-bottom: 0;",
                            "Fee: {order.fee_bps} bps"
                        }
                    }
                }
                
                // Action buttons
                div {
                    class: "modal-buttons",
                    style: "
                        display: flex;
                        gap: 12px;
                        justify-content: space-between;
                        padding: 0 24px 24px;
                    ",
                    button {
                        class: "modal-button secondary",
                        style: "
                            flex: 1;
                            padding: 16px 24px;
                            border-radius: 12px;
                            border: none;
                            cursor: pointer;
                            font-size: 16px;
                            font-weight: 600;
                            transition: all 0.2s ease;
                            background-color: #dc2626;
                            color: white;
                        ",
                        onclick: move |_| onclose.call(()),
                        "Cancel"
                    }
                    button {
                        class: "modal-button primary",
                        style: "
                            flex: 1;
                            padding: 16px 24px;
                            border-radius: 12px;
                            border: none;
                            cursor: pointer;
                            font-size: 16px;
                            font-weight: 600;
                            transition: all 0.2s ease;
                            background-color: #6366f1;
                            color: white;
                        ",
                        disabled: swapping() || selling_amount().is_empty() || fetching_order(),
                        onclick: handle_swap,
                        
                        if fetching_order() {
                            "Getting Quote..."
                        } else if swapping() {
                            "Swapping..."
                        } else {
                            "Swap"
                        }
                    }
                }
                
                // Development notes - show transaction info when available
                if let Some(order) = current_order() {
                    div {
                        style: "
                            margin: 0 24px 24px;
                            padding: 12px;
                            background-color: #0f1419;
                            border-radius: 8px;
                            border: 1px solid #2a2a2a;
                        ",
                        div {
                            style: "color: #9ca3af; font-size: 12px; margin-bottom: 8px;",
                            "🚧 Jupiter Ultra Transaction Info:"
                        }
                        div {
                            style: "color: #22c55e; font-size: 11px; margin-bottom: 4px;",
                            "✅ Request ID: {order.request_id}"
                        }
                        if let Some(tx) = &order.transaction {
                            if !tx.is_empty() {
                                div {
                                    style: "color: #22c55e; font-size: 11px; margin-bottom: 4px;",
                                    {format!("✅ Unsigned TX: {}... ({} chars)", &tx[..20], tx.len())}
                                }
                            } else {
                                div {
                                    style: "color: #f59e0b; font-size: 11px; margin-bottom: 4px;",
                                    "⚠️ No transaction (taker not set)"
                                }
                            }
                        }
                        div {
                            style: "color: #f59e0b; font-size: 11px;",
                            "🔄 Transaction signing needed for execution"
                        }
                    }
                }
            }
        }
    }
}