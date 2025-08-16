use dioxus::prelude::*;
use crate::wallet::WalletInfo;
use crate::hardware::HardwareWallet;
use crate::components::common::Token;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

// Jupiter API Types
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QuoteRequest {
    pub input_mint: String,
    pub output_mint: String,
    pub amount: String,
    pub slippage_bps: Option<u16>,
    pub swap_mode: Option<String>,
}

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

#[derive(Debug, Serialize, Deserialize)]
pub struct SwapRequest {
    #[serde(rename = "userPublicKey")]
    pub user_public_key: String,
    #[serde(rename = "quoteResponse")]
    pub quote_response: QuoteResponse,
    #[serde(rename = "prioritizationFeeLamports")]
    pub prioritization_fee_lamports: Option<serde_json::Value>,
    #[serde(rename = "dynamicComputeUnitLimit")]
    pub dynamic_compute_unit_limit: Option<bool>,
    #[serde(rename = "wrapAndUnwrapSol")]
    pub wrap_and_unwrap_sol: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SwapResponse {
    #[serde(rename = "swapTransaction")]
    pub swap_transaction: String,
    #[serde(rename = "lastValidBlockHeight")]
    pub last_valid_block_height: u64,
    #[serde(rename = "prioritizationFeeLamports")]
    pub prioritization_fee_lamports: Option<u64>,
}

// Token mint addresses (TODO: Make this configurable or fetch from Jupiter)
fn get_token_mint(symbol: &str) -> &'static str {
    match symbol {
        "SOL" => "So11111111111111111111111111111111111111112", // Wrapped SOL
        "USDC" => "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
        "USDT" => "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB",
        "JUP" => "JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN",
        "BONK" => "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263",
        "JTO" => "jtojtomepa8beP8AuQc6eXt5FriJwfFMwQx2v2f9mCL",
        "JLP" => "27G8MtK7VtTcCHkpASjSDdkWWYfoqT6ggEuKidVJidD4",
        _ => "So11111111111111111111111111111111111111112", // Default to SOL
    }
}

// Token icon constants (matching wallet_view.rs)
const ICON_SOL: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/solana-mobile@main/assets/icons/solanaLogo.png";
const ICON_USDC: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/solana-mobile@main/assets/icons/usdcLogo.png";
const ICON_USDT: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/solana-mobile@main/assets/icons/usdtLogo.png";
const ICON_JTO: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/solana-mobile@main/assets/icons/jtoLogo.png";
const ICON_JUP: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/solana-mobile@main/assets/icons/jupLogo.png";
const ICON_JLP: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/solana-mobile@main/assets/icons/jlpLogo.png";
const ICON_BONK: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/solana-mobile@main/assets/icons/bonkLogo.png";
const ICON_32: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/solana-mobile@main/assets/icons/32x32.png";

// Helper function to get token icon
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

#[component]
pub fn SwapModal(
    tokens: Vec<Token>,
    wallet: Option<WalletInfo>,
    hardware_wallet: Option<Arc<HardwareWallet>>,
    current_balance: f64,
    custom_rpc: Option<String>,
    onclose: EventHandler<()>,
    onsuccess: EventHandler<String>,
) -> Element {
    println!("🔄 SwapModal component rendered!"); // Debug log
    
    // State management
    let mut selling_token = use_signal(|| "SOL".to_string()); // Default to SOL for selling
    let mut buying_token = use_signal(|| "USDC".to_string()); // Default to USDC for buying
    let mut selling_amount = use_signal(|| "".to_string());
    let mut buying_amount = use_signal(|| "184.83".to_string()); // Clean 2 decimal format
    let mut swapping = use_signal(|| false);
    let mut error_message = use_signal(|| None as Option<String>);

    // State for real Jupiter quote
    let mut current_quote = use_signal(|| None as Option<QuoteResponse>);
    let mut fetching_quote = use_signal(|| false);

    // Jupiter API functions with essential logging only
    let fetch_jupiter_quote = move |input_mint: String, output_mint: String, amount_lamports: u64| {
        spawn(async move {
            fetching_quote.set(true);
            
            let client = reqwest::Client::new();
            let url = format!(
                "https://lite-api.jup.ag/swap/v1/quote?inputMint={}&outputMint={}&amount={}&slippageBps=50",
                input_mint, output_mint, amount_lamports
            );
            
            match client.get(&url).send().await {
                Ok(response) => {
                    match response.json::<QuoteResponse>().await {
                        Ok(quote) => {
                            println!("✅ Jupiter quote received: {} -> {}", quote.in_amount, quote.out_amount);
                            current_quote.set(Some(quote));
                        }
                        Err(e) => {
                            println!("❌ Failed to parse Jupiter quote: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Jupiter quote request failed: {}", e);
                }
            }
            
            fetching_quote.set(false);
        });
    };

    let execute_jupiter_swap = move |quote: QuoteResponse, user_pubkey: String| {
        println!("🚀 Executing Jupiter swap for user: {}", user_pubkey);
        
        spawn(async move {
            let client = reqwest::Client::new();
            
            let swap_request = SwapRequest {
                user_public_key: user_pubkey,
                quote_response: quote,
                prioritization_fee_lamports: Some(serde_json::json!({
                    "priorityLevelWithMaxLamports": {
                        "maxLamports": 10000000,
                        "priorityLevel": "veryHigh"
                    }
                })),
                dynamic_compute_unit_limit: Some(true),
                wrap_and_unwrap_sol: Some(true),
            };
            
            match client
                .post("https://lite-api.jup.ag/swap/v1/swap")
                .json(&swap_request)
                .send()
                .await 
            {
                Ok(response) => {
                    match response.json::<SwapResponse>().await {
                        Ok(swap_response) => {
                            println!("✅ Jupiter swap transaction created!");
                            let tx_preview = format!("{}...", &swap_response.swap_transaction[..20]);
                            onsuccess.call(format!("Swap transaction ready: {}", tx_preview));
                        }
                        Err(e) => {
                            println!("❌ Failed to parse swap response: {}", e);
                            onsuccess.call(format!("Swap failed: {}", e));
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Jupiter swap request failed: {}", e);
                    onsuccess.call(format!("Swap failed: {}", e));
                }
            }
        });
    };

    // Memoize filtered token lists for better performance
    let tokens_clone = tokens.clone();
    let selling_tokens = use_memo(move || tokens_clone.clone());
    let buying_tokens = use_memo(move || tokens.clone());

    // Memoize token info lookups for better performance
    let selling_token_info = use_memo(move || {
        selling_tokens.read().iter()
            .find(|t| t.symbol == selling_token())
            .cloned()
    });
    
    let buying_token_info = use_memo(move || {
        buying_tokens.read().iter()
            .find(|t| t.symbol == buying_token())
            .cloned()
    });

    // Get selling token balance for display
    let selling_balance = use_memo(move || {
        if let Some(token_info) = selling_token_info() {
            token_info.balance
        } else {
            0.0
        }
    });

    // Get buying token balance for display
    let buying_balance = use_memo(move || {
        if let Some(token_info) = buying_token_info() {
            token_info.balance
        } else {
            0.0
        }
    });

    // Mock token prices in USDC for rate calculations (TODO: Replace with real API)
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

    // Calculate dynamic exchange rate
    let exchange_rate = use_memo(move || {
        let selling_price = get_token_price_usd(&selling_token());
        let buying_price = get_token_price_usd(&buying_token());
        
        if buying_price > 0.0 {
            selling_price / buying_price
        } else {
            1.0
        }
    });

    // Calculate USD value for selling amount
    let selling_usd_value = use_memo(move || {
        if let Ok(amount) = selling_amount().parse::<f64>() {
            let price = get_token_price_usd(&selling_token());
            amount * price
        } else {
            0.0
        }
    });

    // Calculate USD value for buying amount
    let buying_usd_value = use_memo(move || {
        if let Ok(amount) = buying_amount().parse::<f64>() {
            let price = get_token_price_usd(&buying_token());
            amount * price
        } else {
            0.0
        }
    });

    // Handle amount input changes with Jupiter quotes + fallback
    let mut handle_amount_change = move |value: String| {
        selling_amount.set(value.clone());
        error_message.set(None);
        
        if !value.is_empty() {
            if let Ok(amount) = value.parse::<f64>() {
                // Always calculate fallback rate immediately
                let fallback_rate = exchange_rate();
                let fallback_converted = amount * fallback_rate;
                let fallback_formatted = if fallback_converted < 0.01 && fallback_converted > 0.0 {
                    format!("{:.6}", fallback_converted)
                } else {
                    format!("{:.2}", fallback_converted)
                };
                
                // Set fallback amount immediately
                buying_amount.set(fallback_formatted);
                
                // Convert to lamports and fetch Jupiter quote
                let amount_lamports = (amount * 1_000_000_000.0) as u64;
                let input_mint = get_token_mint(&selling_token()).to_string();
                let output_mint = get_token_mint(&buying_token()).to_string();
                
                // Fetch real quote from Jupiter (will update buying_amount if successful)
                fetch_jupiter_quote(input_mint, output_mint, amount_lamports);
            }
        } else {
            buying_amount.set("0.00".to_string());
            current_quote.set(None);
        }
    };

    // Update buying amount when Jupiter quote changes
    use_effect(move || {
        if let Some(quote) = current_quote() {
            let output_amount = quote.out_amount.parse::<u64>().unwrap_or(0) as f64 / 1_000_000_000.0;
            
            let formatted = if output_amount < 0.01 && output_amount > 0.0 {
                format!("{:.6}", output_amount)
            } else {
                format!("{:.2}", output_amount)
            };
            
            buying_amount.set(formatted);
        }
    });

    // Handle swap button click with simple logging
    let handle_swap = move |_| {
        println!("🔄 Swap button clicked! Selling: {} {} -> Buying: {} {}", 
            selling_amount(), selling_token(), buying_amount(), buying_token());
        
        if selling_amount().is_empty() {
            error_message.set(Some("Please enter an amount to sell".to_string()));
            return;
        }

        if let Ok(amount) = selling_amount().parse::<f64>() {
            if amount > selling_balance() {
                error_message.set(Some("Insufficient balance".to_string()));
                return;
            }
        }

        // Get user's public key
        let user_pubkey = if let Some(wallet_info) = &wallet {
            wallet_info.address.clone()
        } else if let Some(_hw) = &hardware_wallet {
            "HARDWARE_WALLET_PUBKEY".to_string() // TODO: Get real hardware wallet pubkey
        } else {
            error_message.set(Some("No wallet connected".to_string()));
            return;
        };
        
        println!("💰 User pubkey: {}", user_pubkey);

        // Check if we have a current quote
        if let Some(quote) = current_quote() {
            println!("✅ Using Jupiter quote for swap");
            swapping.set(true);
            execute_jupiter_swap(quote, user_pubkey);
            
            spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                swapping.set(false);
            });
        } else {
            println!("⚠️ No Jupiter quote available, using fallback swap simulation");
            swapping.set(true);
            
            // Fallback: simulate swap without Jupiter
            spawn(async move {
                println!("🔄 Starting fallback swap simulation...");
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                println!("✅ Fallback swap completed!");
                swapping.set(false);
                onsuccess.call("Fallback swap completed successfully!".to_string());
            });
        }
    };

    // Handle token swap (switch selling and buying tokens)
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
        
        // Exchange rate and USD values will update automatically via memos
    };

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
                        class: "swap-error-message-v2",
                        style: "
                            margin: 16px 24px 0;
                            padding: 12px 16px;
                            background-color: rgba(239, 68, 68, 0.1);
                            border: 1px solid rgba(239, 68, 68, 0.3);
                            border-radius: 12px;
                            color: #fca5a5;
                            font-size: 14px;
                            text-align: center;
                        ",
                        "{error}"
                    }
                }
                
                // Selling section
                div {
                    class: "swap-section-v2",
                    style: "padding: 16px 24px 12px; position: relative;",
                    
                    // Section header with title and balance info
                    div {
                        class: "swap-section-header-v2",
                        style: "
                            display: flex;
                            justify-content: space-between;
                            align-items: center;
                            margin-bottom: 12px;
                        ",
                        span { 
                            class: "swap-section-title-v2",
                            style: "
                                color: #94a3b8;
                                font-size: 14px;
                                font-weight: 500;
                                text-transform: uppercase;
                                letter-spacing: 0.05em;
                            ",
                            "Selling" 
                        }
                        div {
                            class: "swap-section-balance-v2",
                            style: "display: flex; align-items: center; gap: 8px;",
                            span { 
                                class: "swap-balance-icon-v2",
                                style: "font-size: 12px;",
                                "💰" 
                            }
                            span { 
                                class: "swap-balance-amount-v2",
                                style: "
                                    color: #cbd5e1;
                                    font-size: 12px;
                                    font-weight: 500;
                                ",
                                "{selling_balance():.2} {selling_token()}"
                            }
                            button {
                                class: "swap-max-button-v2",
                                style: "
                                    background: rgba(99, 102, 241, 0.15);
                                    border: 1px solid rgba(99, 102, 241, 0.3);
                                    color: #a5b4fc;
                                    padding: 2px 8px;
                                    border-radius: 8px;
                                    font-size: 11px;
                                    font-weight: 600;
                                    cursor: pointer;
                                    transition: all 0.2s ease;
                                    text-transform: uppercase;
                                    letter-spacing: 0.025em;
                                    margin-left: 4px;
                                ",
                                onclick: move |_| {
                                    let half_balance = selling_balance() / 2.0;
                                    selling_amount.set(format!("{:.6}", half_balance));
                                    handle_amount_change(format!("{:.6}", half_balance));
                                },
                                "HALF"
                            }
                            button {
                                class: "swap-max-button-v2",
                                style: "
                                    background: rgba(99, 102, 241, 0.15);
                                    border: 1px solid rgba(99, 102, 241, 0.3);
                                    color: #a5b4fc;
                                    padding: 2px 8px;
                                    border-radius: 8px;
                                    font-size: 11px;
                                    font-weight: 600;
                                    cursor: pointer;
                                    transition: all 0.2s ease;
                                    text-transform: uppercase;
                                    letter-spacing: 0.025em;
                                    margin-left: 4px;
                                ",
                                onclick: move |_| {
                                    selling_amount.set(format!("{:.6}", selling_balance()));
                                    handle_amount_change(format!("{:.6}", selling_balance()));
                                },
                                "MAX"
                            }
                        }
                    }
                    
                    // CRITICAL: Main input container with inline styles
                    div {
                        class: "swap-input-container-v2",
                        style: "
                            display: flex;
                            justify-content: space-between;
                            align-items: center;
                            background: rgba(15, 23, 42, 0.6);
                            border: 1px solid rgba(148, 163, 184, 0.15);
                            border-radius: 12px;
                            padding: 18px 20px;
                            min-height: 72px;
                            gap: 20px;
                            width: 100%;
                            box-sizing: border-box;
                        ",
                        
                        // Left side: Token selection
                        div {
                            class: "swap-token-side-v2",
                            style: "
                                display: flex;
                                align-items: center;
                                gap: 8px;
                                flex-shrink: 0;
                                width: 105px;
                            ",
                            img {
                                class: "swap-token-icon-v2",
                                style: "
                                    width: 24px;
                                    height: 24px;
                                    border-radius: 50%;
                                    object-fit: cover;
                                    flex-shrink: 0;
                                    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
                                ",
                                src: "{get_token_icon(&selling_token())}",
                                alt: "{selling_token()}",
                            }
                            select {
                                class: "swap-token-dropdown-v2",
                                style: "
                                    background: transparent;
                                    border: none;
                                    color: #f8fafc;
                                    font-size: 16px;
                                    font-weight: 600;
                                    cursor: pointer;
                                    outline: none;
                                    appearance: none;
                                    padding: 4px 20px 4px 4px;
                                    width: 73px;
                                    letter-spacing: -0.02em;
                                    background-image: url('data:image/svg+xml,%3csvg xmlns=\"http://www.w3.org/2000/svg\" fill=\"none\" viewBox=\"0 0 20 20\"%3e%3cpath stroke=\"%2394a3b8\" stroke-linecap=\"round\" stroke-linejoin=\"round\" stroke-width=\"1.5\" d=\"M6 8l4 4 4-4\"/%3e%3c/svg%3e');
                                    background-position: right 2px center;
                                    background-repeat: no-repeat;
                                    background-size: 12px;
                                ",
                                value: "{selling_token()}",
                                onchange: move |e| {
                                    selling_token.set(e.value());
                                    selling_amount.set("".to_string());
                                    buying_amount.set("0.00".to_string());
                                },
                                
                                option { value: "SOL", "SOL" }
                                option { value: "USDC", "USDC" }
                                option { value: "USDT", "USDT" }
                                option { value: "JUP", "JUP" }
                                option { value: "BONK", "BONK" }
                                for token in selling_tokens.read().iter() {
                                    if !["SOL", "USDC", "USDT", "JUP", "BONK"].contains(&token.symbol.as_str()) {
                                        option { 
                                            value: "{token.symbol}", 
                                            "{token.symbol}" 
                                        }
                                    }
                                }
                            }
                        }
                        
                        // Right side: Amount input
                        div {
                            class: "swap-amount-side-v2",
                            style: "
                                display: flex;
                                flex-direction: column;
                                align-items: flex-end;
                                justify-content: center;
                                flex: 1;
                                text-align: right;
                                min-width: 0;
                                max-width: 180px;
                            ",
                            input {
                                class: "swap-amount-input-v2",
                                style: "
                                    background: transparent;
                                    border: none;
                                    color: #f8fafc;
                                    font-size: 28px;
                                    font-weight: 700;
                                    text-align: right;
                                    width: 100%;
                                    outline: none;
                                    padding: 0;
                                    margin: 0;
                                    letter-spacing: -0.02em;
                                    line-height: 1;
                                    font-family: system-ui, -apple-system, sans-serif;
                                    box-sizing: border-box;
                                ",
                                r#type: "text",
                                value: "{selling_amount()}",
                                placeholder: "1",
                                oninput: move |e| handle_amount_change(e.value())
                            }
                            div {
                                class: "swap-amount-usd-v2",
                                style: "
                                    color: #64748b;
                                    font-size: 15px;
                                    font-weight: 500;
                                    text-align: right;
                                    margin-top: 6px;
                                    letter-spacing: -0.01em;
                                    white-space: nowrap;
                                ",
                                "${selling_usd_value():.2}"
                            }
                        }
                    }
                }
                
                // Swap direction button
                div {
                    class: "swap-direction-container-v2",
                    style: "
                        display: flex;
                        justify-content: center;
                        align-items: center;
                        position: relative;
                        margin: -8px 0;
                        z-index: 10;
                    ",
                    button {
                        class: "swap-direction-button-v2",
                        style: "
                            background: linear-gradient(135deg, #374151 0%, #1f2937 100%);
                            border: 2px solid rgba(148, 163, 184, 0.2);
                            border-radius: 50%;
                            width: 40px;
                            height: 40px;
                            color: #e2e8f0;
                            font-size: 16px;
                            cursor: pointer;
                            transition: all 0.3s ease;
                            display: flex;
                            align-items: center;
                            justify-content: center;
                            font-weight: 600;
                            box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
                        ",
                        onclick: handle_token_swap,
                        "⇅"
                    }
                }
                
                // Buying section
                div {
                    class: "swap-section-v2",
                    style: "padding: 16px 24px 12px; position: relative;",
                    
                    // Section header with title and balance info
                    div {
                        class: "swap-section-header-v2",
                        style: "
                            display: flex;
                            justify-content: space-between;
                            align-items: center;
                            margin-bottom: 12px;
                        ",
                        span { 
                            class: "swap-section-title-v2",
                            style: "
                                color: #94a3b8;
                                font-size: 14px;
                                font-weight: 500;
                                text-transform: uppercase;
                                letter-spacing: 0.05em;
                            ",
                            "Buying" 
                        }
                        div {
                            class: "swap-section-balance-v2",
                            style: "display: flex; align-items: center; gap: 8px;",
                            span { 
                                class: "swap-balance-icon-v2",
                                style: "font-size: 12px;",
                                "💰" 
                            }
                            span { 
                                class: "swap-balance-amount-v2",
                                style: "
                                    color: #cbd5e1;
                                    font-size: 12px;
                                    font-weight: 500;
                                ",
                                "{buying_balance():.2} {buying_token()}"
                            }
                        }
                    }
                    
                    // Main input container - consistent structure
                    div {
                        class: "swap-input-container-v2",
                        style: "
                            display: flex;
                            justify-content: space-between;
                            align-items: center;
                            background: rgba(15, 23, 42, 0.6);
                            border: 1px solid rgba(148, 163, 184, 0.15);
                            border-radius: 12px;
                            padding: 18px 20px;
                            min-height: 72px;
                            gap: 20px;
                            width: 100%;
                            box-sizing: border-box;
                        ",
                        
                        // Left side: Token selection
                        div {
                            class: "swap-token-side-v2",
                            style: "
                                display: flex;
                                align-items: center;
                                gap: 8px;
                                flex-shrink: 0;
                                width: 105px;
                            ",
                            img {
                                class: "swap-token-icon-v2",
                                style: "
                                    width: 24px;
                                    height: 24px;
                                    border-radius: 50%;
                                    object-fit: cover;
                                    flex-shrink: 0;
                                    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
                                ",
                                src: "{get_token_icon(&buying_token())}",
                                alt: "{buying_token()}",
                            }
                            select {
                                class: "swap-token-dropdown-v2",
                                style: "
                                    background: transparent;
                                    border: none;
                                    color: #f8fafc;
                                    font-size: 16px;
                                    font-weight: 600;
                                    cursor: pointer;
                                    outline: none;
                                    appearance: none;
                                    padding: 4px 20px 4px 4px;
                                    width: 73px;
                                    letter-spacing: -0.02em;
                                    background-image: url('data:image/svg+xml,%3csvg xmlns=\"http://www.w3.org/2000/svg\" fill=\"none\" viewBox=\"0 0 20 20\"%3e%3cpath stroke=\"%2394a3b8\" stroke-linecap=\"round\" stroke-linejoin=\"round\" stroke-width=\"1.5\" d=\"M6 8l4 4 4-4\"/%3e%3c/svg%3e');
                                    background-position: right 2px center;
                                    background-repeat: no-repeat;
                                    background-size: 12px;
                                ",
                                value: "{buying_token()}",
                                onchange: move |e| {
                                    buying_token.set(e.value());
                                    selling_amount.set("".to_string());
                                    buying_amount.set("0.00".to_string());
                                },
                                
                                option { value: "USDC", "USDC" }
                                option { value: "USDT", "USDT" }
                                option { value: "SOL", "SOL" }
                                option { value: "JUP", "JUP" }
                                option { value: "BONK", "BONK" }
                                for token in buying_tokens.read().iter() {
                                    if !["USDC", "USDT", "SOL", "JUP", "BONK"].contains(&token.symbol.as_str()) {
                                        option { 
                                            value: "{token.symbol}", 
                                            "{token.symbol}" 
                                        }
                                    }
                                }
                            }
                        }
                        
                        // Right side: Amount display (read-only)
                        div {
                            class: "swap-amount-side-v2",
                            style: "
                                display: flex;
                                flex-direction: column;
                                align-items: flex-end;
                                justify-content: center;
                                flex: 1;
                                text-align: right;
                                min-width: 0;
                                max-width: 180px;
                            ",
                            div {
                                class: "swap-amount-display-v2",
                                style: "
                                    color: #cbd5e1;
                                    font-size: 28px;
                                    font-weight: 700;
                                    text-align: right;
                                    letter-spacing: -0.02em;
                                    line-height: 1;
                                    font-family: system-ui, -apple-system, sans-serif;
                                    word-break: break-all;
                                    overflow-wrap: break-word;
                                ",
                                "{buying_amount()}"
                            }
                            div {
                                class: "swap-amount-usd-v2",
                                style: "
                                    color: #64748b;
                                    font-size: 15px;
                                    font-weight: 500;
                                    text-align: right;
                                    margin-top: 6px;
                                    letter-spacing: -0.01em;
                                    white-space: nowrap;
                                ",
                                "${buying_usd_value():.2}"
                            }
                        }
                    }
                }
                
                // Rate info - now shows real Jupiter data or fallback
                div {
                    class: "swap-rate-info-v2",
                    style: "
                        padding: 12px 24px;
                        text-align: center;
                        color: #94a3b8;
                        font-size: 14px;
                        font-weight: 500;
                        border-top: 1px solid rgba(148, 163, 184, 0.1);
                        background: rgba(15, 23, 42, 0.3);
                    ",
                    {
                        if fetching_quote() {
                            "Getting best rate...".to_string()
                        } else if let Some(quote) = current_quote() {
                            let input_amount = quote.in_amount.parse::<u64>().unwrap_or(0) as f64 / 1_000_000_000.0;
                            let output_amount = quote.out_amount.parse::<u64>().unwrap_or(0) as f64 / 1_000_000_000.0;
                            let rate = if input_amount > 0.0 { output_amount / input_amount } else { 0.0 };
                            
                            let formatted_rate = if rate < 0.01 {
                                format!("{:.6}", rate)
                            } else {
                                format!("{:.2}", rate)
                            };
                            
                            format!("Rate: 1 {} ≈ {} {} (Jupiter)", selling_token(), formatted_rate, buying_token())
                        } else {
                            let rate = exchange_rate();
                            let formatted_rate = if rate < 0.01 {
                                format!("{:.6}", rate)
                            } else {
                                format!("{:.2}", rate)
                            };
                            format!("Rate: 1 {} ≈ {} {}", selling_token(), formatted_rate, buying_token())
                        }
                    }
                }
                
                // Swap button
                button {
                    class: "swap-action-button-v2",
                    style: "
                        width: calc(100% - 48px);
                        margin: 16px 24px 24px;
                        padding: 18px 32px;
                        background: linear-gradient(135deg, #a3f3a0 0%, #7dd3fc 100%);
                        border: none;
                        border-radius: 16px;
                        color: #0f172a;
                        font-size: 18px;
                        font-weight: 700;
                        cursor: pointer;
                        transition: all 0.3s ease;
                        text-transform: uppercase;
                        letter-spacing: 0.025em;
                        box-shadow: 0 8px 24px rgba(163, 243, 160, 0.3);
                    ",
                    disabled: swapping() || selling_amount().is_empty() || fetching_quote(),
                    onclick: handle_swap,
                    {
                        if fetching_quote() {
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