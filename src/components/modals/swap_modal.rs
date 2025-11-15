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
use solana_sdk::{
    transaction::VersionedTransaction,
    pubkey::Pubkey as SolanaPubkey,
    hash::Hash as SolanaHash,
};
use crate::titan::{TitanClient, build_transaction_from_route};
use crate::titan::SwapRoute as TitanSwapRoute;

const ICON_SWITCH: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/icons/SWITCH.svg";

/// Sign a Jupiter Ultra transaction using the provided signer
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
    println!("⏳ Waiting for hardware wallet signature...");
    let signature_bytes = match signer.sign_message(&message_bytes).await {
        Ok(sig) => {
            println!("✅ Hardware wallet returned signature: {} bytes", sig.len());
            sig
        }
        Err(e) => {
            println!("❌ Hardware wallet signing failed: {}", e);
            return Err(format!("Failed to sign message: {}", e));
        }
    };
    
    // Ensure we have exactly 64 bytes for the signature
    if signature_bytes.len() != 64 {
        println!("❌ Invalid signature length from hardware wallet");
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

// Get token mint address from actual token data
fn get_token_mint<'a>(symbol: &str, tokens: &'a [Token]) -> &'a str {
    tokens.iter()
        .find(|t| t.symbol == symbol)
        .map(|t| t.mint.as_str())
        .unwrap_or("So11111111111111111111111111111111111111112") // Default to SOL if not found
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
                                class: "button-standard ghost",
                                href: "{solscan_url}",
                                target: "_blank",
                                rel: "noopener noreferrer",
                                "Solscan"
                            }
                            a {
                                class: "button-standard ghost",
                                href: "{orb_url}",
                                target: "_blank",
                                rel: "noopener noreferrer",
                                "Orb"
                            }
                        }
                    }
                }
                
                div { class: "modal-buttons",
                    button {
                        class: "button-standard primary",
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
    
    // Store hardware wallet address (fetched async)
    let mut hw_address = use_signal(|| None as Option<String>);

    // Clone tokens for closures - need separate clones for each closure
    let tokens_clone = tokens.clone();
    let tokens_clone2 = tokens.clone();
    let tokens_clone3 = tokens.clone();
    let tokens_clone4 = tokens.clone(); // For handle_amount_change
    let tokens_clone5 = tokens.clone(); // For quote comparison use_effect
    let tokens_clone6 = tokens.clone(); // For UI rendering

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
        let client = titan_client();
        spawn(async move {
            // Prevent multiple simultaneous requests
            if fetching_titan() {
                return;
            }
            
            fetching_titan.set(true);
            
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
            
            // Lock and use the client
            let mut client_lock = client.lock().await;
            
            // Connect if not connected
            if let Err(e) = client_lock.connect().await {
                println!("❌ Failed to connect to Titan: {}", e);
                fetching_titan.set(false);
                return;
            }
            
            // Request swap quotes
            match client_lock.request_swap_quotes(
                &input_mint,
                &output_mint,
                amount_lamports,
                &user_pk,
                Some(50), // 0.5% slippage
            ).await {
                Ok((provider_name, route)) => {
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
                Err(e) => {
                    println!("❌ Failed to get Titan quote: {}", e);
                    titan_quote.set(None);
                }
            }
            
            // Close connection
            let _ = client_lock.close().await;
            
            fetching_titan.set(false);
        });
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
                                    
                                    if let Some(ref tx) = order.transaction {
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

    // Titan Exchange: Execute transaction via direct Solana RPC submission
    let execute_titan_swap = move |signed_transaction_b64: String, custom_rpc: Option<String>| {
        spawn(async move {
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
            
            // Create transaction client with custom RPC if provided
            let rpc_url = custom_rpc.as_deref();
            let transaction_client = TransactionClient::new(rpc_url);
            
            // Submit directly to Solana RPC
            match transaction_client.send_transaction(&signed_tx_b58).await {
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
        current_order.set(None); // Clear previous Jupiter order
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
                    
                    let input_mint = get_token_mint(&selling_token(), &tokens_clone4).to_string();
                    let output_mint = get_token_mint(&buying_token(), &tokens_clone4).to_string();
                    let user_pubkey = get_user_pubkey();
                    
                    // Clone for each async call
                    let input_mint_jup = input_mint.clone();
                    let output_mint_jup = output_mint.clone();
                    let user_pubkey_jup = user_pubkey.clone();
                    
                    let input_mint_titan = input_mint.clone();
                    let output_mint_titan = output_mint.clone();
                    let user_pubkey_titan = user_pubkey.clone();
                    
                    // Add small delay to prevent too many API calls
                    spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                        
                        // Spawn both quote requests in parallel
                        println!("🔄 Fetching quotes from both Jupiter and Titan...");
                        
                        // Jupiter request
                        fetch_jupiter_ultra_order(input_mint_jup, output_mint_jup, amount_lamports, user_pubkey_jup);
                        
                        // Titan request (runs in parallel)
                        fetch_titan_quotes(input_mint_titan, output_mint_titan, amount_lamports, user_pubkey_titan);
                    });
                }
            }
        } else {
            buying_amount.set("0.00".to_string());
            current_order.set(None);
            titan_quote.set(None);
        }
    };

    // Quote comparison logic: Compare Jupiter and Titan quotes and select the best
    use_effect(move || {
        let jupiter_quote = current_order();
        let titan_q = titan_quote();
        
        // Compare quotes if both are available
        if let (Some(jupiter_order), Some((provider_name, titan_route))) = (jupiter_quote.clone(), titan_q.clone()) {
            // Parse output amounts
            let jupiter_output = jupiter_order.out_amount.parse::<u64>().unwrap_or(0);
            let titan_output = titan_route.out_amount;
            
            println!("📊 Quote Comparison:");
            println!("   Jupiter: {} lamports", jupiter_output);
            println!("   Titan ({}): {} lamports", provider_name, titan_output);
            
            // Select the provider with higher output (better for user)
            if titan_output > jupiter_output {
                println!("🏆 Titan wins with {} more lamports", titan_output - jupiter_output);
                selected_provider.set(Some("Titan".to_string()));
                
                // Update buying amount with Titan quote
                let converted_amount = from_lamports(titan_output, &buying_token(), &tokens_clone5);
                let formatted = if converted_amount < 0.01 && converted_amount > 0.0 {
                    format!("{:.6}", converted_amount)
                } else {
                    format!("{:.2}", converted_amount)
                };
                buying_amount.set(formatted);
            } else {
                println!("🏆 Jupiter wins with {} more lamports", jupiter_output - titan_output);
                selected_provider.set(Some("Jupiter".to_string()));
                
                // Update buying amount with Jupiter quote
                let converted_amount = from_lamports(jupiter_output, &buying_token(), &tokens_clone5);
                let formatted = if converted_amount < 0.01 && converted_amount > 0.0 {
                    format!("{:.6}", converted_amount)
                } else {
                    format!("{:.2}", converted_amount)
                };
                buying_amount.set(formatted);
            }
        } else if let Some(order) = jupiter_quote {
            // Only Jupiter quote available
            println!("📊 Only Jupiter quote available");
            selected_provider.set(Some("Jupiter".to_string()));
            
            let output_amount = order.out_amount.parse::<u64>().unwrap_or(0);
            let converted_amount = from_lamports(output_amount, &buying_token(), &tokens_clone5);
            
            let formatted = if converted_amount < 0.01 && converted_amount > 0.0 {
                format!("{:.6}", converted_amount)
            } else {
                format!("{:.2}", converted_amount)
            };
            
            buying_amount.set(formatted);
        } else if let Some((provider_name, titan_route)) = titan_q {
            // Only Titan quote available
            println!("📊 Only Titan quote available from {}", provider_name);
            selected_provider.set(Some("Titan".to_string()));
            
            let converted_amount = from_lamports(titan_route.out_amount, &buying_token(), &tokens_clone5);
            
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
                        
                        // Build transaction from Titan's instructions
                        spawn(async move {
                            println!("🔧 Fetching recent blockhash...");
                            
                            // Create RPC client to fetch recent blockhash
                            let rpc_client = TransactionClient::new(custom_rpc_titan.as_deref());
                            
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
                            let unsigned_tx_bytes = match build_transaction_from_route(
                                &titan_route,
                                user_pubkey,
                                recent_blockhash,
                                rpc_url,
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
                                    execute_titan_swap(signed_transaction_b64, custom_rpc_for_titan);
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
                } else if provider == Some("Jupiter".to_string()) || current_order().is_some() {
                    // Jupiter won OR fallback to Jupiter if available
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
                        error_message.set(Some("No transaction data available".to_string()));
                        swapping.set(false);
                    }
                } else {
                    error_message.set(Some("No quote available - please wait for quotes".to_string()));
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
                    style: "padding: 20px 24px 16px;",
                    
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
                            "Balance: {selling_balance():.6} {selling_token()}"
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
                            select {
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
                                    -webkit-appearance: none;
                                    -moz-appearance: none;
                                    appearance: none;
                                ",
                                value: selling_token(),
                                onchange: move |e| {
                                    selling_token.set(e.value());
                                    selling_amount.set("".to_string());
                                    buying_amount.set("0.00".to_string());
                                    current_order.set(None);
                                },
                                
                                // Dynamically generate options from user's tokens
                                for token in tokens_clone6.iter() {
                                    option { 
                                        value: "{token.symbol}",
                                        "{token.symbol}"
                                    }
                                }
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
                            "Balance: {buying_balance():.6} {buying_token()}"
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
                        
                        // Token selector
                        div {
                            class: "swap-token-side",
                            style: "display: flex; align-items: center; gap: 12px; flex-shrink: 0;",
                            img {
                                class: "swap-token-icon",
                                style: "width: 32px; height: 32px; border-radius: 50%;",
                                src: get_token_icon(&buying_token(), &tokens_clone6),
                                alt: buying_token()
                            }
                            select {
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
                                    -webkit-appearance: none;
                                    -moz-appearance: none;
                                    appearance: none;
                                ",
                                value: buying_token(),
                                onchange: move |e| {
                                    buying_token.set(e.value());
                                    buying_amount.set("0.00".to_string());
                                    current_order.set(None);
                                },
                                
                                // Dynamically generate options from user's tokens
                                for token in tokens_clone6.iter() {
                                    option { 
                                        value: "{token.symbol}",
                                        "{token.symbol}"
                                    }
                                }
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
                
                // Rate information
                div {
                    class: "swap-rate-section",
                    style: "
                        background: #1a1a1a;
                        border-radius: 12px;
                        border: 1.5px solid #4a4a4a;
                        padding: 16px;
                        margin: 0 24px 20px;
                    ",
                    
                    // Show provider badge if a provider has been selected
                    if let Some(provider) = selected_provider() {
                        div {
                            class: "provider-badge",
                            style: "
                                display: inline-block;
                                padding: 6px 14px;
                                border-radius: 8px;
                                font-size: 13px;
                                font-weight: 600;
                                margin-bottom: 12px;
                                background: #3a3a3a;
                                color: #ffffff;
                                border: 1px solid #5a5a5a;
                            ",
                            "Best rate from {provider}"
                        }
                    }
                    
                    div {
                        class: "swap-rate-row",
                        style: "color: #cbd5e1; font-size: 14px; margin-bottom: 8px; font-weight: 500;",
                        {
                            if fetching_order() || fetching_titan() {
                                "Getting best rate...".to_string()
                            } else if let Some(order) = current_order() {
                                let input_amount = order.in_amount.parse::<u64>().unwrap_or(0);
                                let output_amount = order.out_amount.parse::<u64>().unwrap_or(0);
                                
                                let input_converted = from_lamports(input_amount, &selling_token(), &tokens);
                                let output_converted = from_lamports(output_amount, &buying_token(), &tokens);
                                
                                let rate = if input_converted > 0.0 { output_converted / input_converted } else { 0.0 };
                                
                                let formatted_rate = if rate < 0.01 {
                                    format!("{:.6}", rate)
                                } else {
                                    format!("{:.4}", rate)
                                };
                                
                                format!("Rate: 1 {} = {} {}", selling_token(), formatted_rate, buying_token())
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
                    
                    // Show additional info if available
                    if let Some(order) = current_order() {
                        div {
                            class: "swap-rate-row",
                            style: "color: #94a3b8; font-size: 13px; margin-bottom: 4px;",
                            "Route: {order.router}"
                        }
                        if let Some(price_impact) = order.price_impact {
                            div {
                                class: "swap-rate-row",
                                style: "color: #94a3b8; font-size: 13px; margin-bottom: 4px;",
                                "Price Impact: {price_impact:.4}%"
                            }
                        }
                        div {
                            class: "swap-rate-row",
                            style: "color: #94a3b8; font-size: 13px; margin-bottom: 0;",
                            "Fee: {order.fee_bps} bps"
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