use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use crate::components::common::Token;
use crate::wallet::WalletInfo;
use crate::hardware::HardwareWallet;
use crate::signing::hardware::HardwareSigner;
use crate::signing::software::SoftwareSigner;
use crate::signing::TransactionSigner;
use crate::wallet::Wallet;
use std::sync::Arc;
use reqwest::header;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer as SolanaSigner, Signature},
    transaction::VersionedTransaction,
};
use base64;
use bincode;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JupiterLendToken {
    pub id: i32,
    pub address: String,
    pub name: String,
    pub symbol: String,
    pub decimals: i32,
    #[serde(rename = "assetAddress")]
    pub asset_address: String,
    pub asset: serde_json::Value,
    #[serde(rename = "totalAssets")]
    pub total_assets: String,
    #[serde(rename = "totalSupply")]
    pub total_supply: String,
    #[serde(rename = "convertToShares")]
    pub convert_to_shares: String,
    #[serde(rename = "convertToAssets")]
    pub convert_to_assets: String,
    #[serde(rename = "rewardsRate")]
    pub rewards_rate: String,
    #[serde(rename = "supplyRate")]
    pub supply_rate: String,
    #[serde(rename = "totalRate")]
    pub total_rate: String,
    #[serde(rename = "rebalanceDifference")]
    pub rebalance_difference: String,
    #[serde(rename = "liquiditySupplyData")]
    pub liquidity_supply_data: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Position {
    pub token: JupiterLendToken,
    #[serde(rename = "ownerAddress")]
    pub owner_address: String,
    pub shares: String,
    #[serde(rename = "underlyingAssets")]
    pub underlying_assets: String,
    #[serde(rename = "underlyingBalance")]
    pub underlying_balance: String,
    pub allowance: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Earning {
    pub address: String,
    #[serde(rename = "ownerAddress")]
    pub owner_address: String,
    #[serde(rename = "totalDeposits")]
    pub total_deposits: String,
    #[serde(rename = "totalWithdraws")]
    pub total_withdraws: String,
    #[serde(rename = "totalBalance")]
    pub total_balance: String,
    #[serde(rename = "totalAssets")]
    pub total_assets: String,
    pub earnings: String,
}

fn get_fallback_icon(symbol: &str) -> String {
    match symbol {
        "USDC" => "assets/lendLogos/usdc.png".to_string(),
        "SOL" => "assets/lendLogos/sol.png".to_string(),
        "USDT" => "assets/lendLogos/usdt.png".to_string(),
        "EURC" => "assets/lendLogos/eurc.png".to_string(),
        "USDG" => "assets/lendLogos/usdg.png".to_string(),
        "USDS" => "assets/lendLogos/usds.png".to_string(),
        _ => "assets/default-token.png".to_string(), // Add a default fallback icon if needed
    }
}

async fn sign_jupiter_lend_transaction(
    signer: &dyn TransactionSigner,
    unsigned_transaction_b64: &str,
) -> Result<String, String> {
    // Decode the base64 unsigned transaction
    let unsigned_tx_bytes = match base64::decode(unsigned_transaction_b64) {
        Ok(bytes) => bytes,
        Err(e) => return Err(format!("Failed to decode base64 transaction: {}", e)),
    };
    
    // Deserialize the transaction
    let mut transaction: VersionedTransaction = match bincode::deserialize(&unsigned_tx_bytes) {
        Ok(tx) => tx,
        Err(e) => return Err(format!("Failed to deserialize transaction: {}", e)),
    };
    
    // Serialize the transaction message for signing
    let message_bytes = transaction.message.serialize();
    
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
    
    // Serialize the signed transaction
    let signed_tx_bytes = match bincode::serialize(&transaction) {
        Ok(bytes) => bytes,
        Err(e) => return Err(format!("Failed to serialize signed transaction: {}", e)),
    };
    
    // Encode back to base64
    let signed_transaction_b64 = base64::encode(&signed_tx_bytes);
    
    Ok(signed_transaction_b64)
}

async fn execute_jupiter_lend_transaction(
    signed_transaction_b64: String,
    rpc_url: String,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let send_body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "sendTransaction",
        "params": [signed_transaction_b64, { "encoding": "base64", "skipPreflight": true, "preflightCommitment": "finalized" }]
    });
    let response = client
        .post(&rpc_url)
        .json(&send_body)
        .send()
        .await;

    match response {
        Ok(res) if res.status().is_success() => {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                if let Some(sig) = json.get("result").and_then(|v| v.as_str()) {
                    Ok(sig.to_string())
                } else {
                    Err("Failed to get signature from RPC response".to_string())
                }
            } else {
                Err("Failed to parse RPC response".to_string())
            }
        }
        Ok(res) => Err(format!("RPC request failed with status: {}", res.status())),
        Err(e) => Err(format!("Failed to send to RPC: {}", e)),
    }
}

#[component]
pub fn LendModal(
    tokens: Vec<Token>,
    wallet: Option<WalletInfo>,
    hardware_wallet: Option<Arc<HardwareWallet>>,
    custom_rpc: Option<String>,
    onclose: EventHandler<()>,
    onsuccess: EventHandler<String>,
) -> Element {
    println!(" LendModal component rendered with Jupiter Lend API!");

    let wallet_address = use_signal(|| wallet.as_ref().map(|w| w.address.clone()));

    // State management
    let mut selected_symbol = use_signal(|| None::<String>);
    let mut amount = use_signal(|| "".to_string());
    let mut mode = use_signal(|| "deposit".to_string());
    let mut processing = use_signal(|| false);
    let mut error_message = use_signal(|| None::<String>);

    // API states
    let mut available_lend_tokens = use_signal(|| Vec::<JupiterLendToken>::new());
    let mut positions = use_signal(|| Vec::<Position>::new());
    let mut earnings = use_signal(|| Vec::<Earning>::new());
    let mut fetching_tokens = use_signal(|| false);
    let mut fetching_positions = use_signal(|| false);
    let mut fetching_earnings = use_signal(|| false);
    let mut selected_lend_token = use_signal(|| None::<JupiterLendToken>);

    // Transaction success state
    let mut show_success_modal = use_signal(|| false);
    let mut transaction_signature = use_signal(|| "".to_string());
    let mut was_hardware_transaction = use_signal(|| false);
    let mut show_hardware_approval = use_signal(|| false);

    // Fetch available lending tokens on mount
    use_effect(move || {
        if available_lend_tokens().is_empty() && !fetching_tokens() {
            fetching_tokens.set(true);
            
            spawn(async move {
                let client = reqwest::Client::new();
                let response = client
                    .get("https://lite-api.jup.ag/lend/v1/earn/tokens")
                    .header("Accept", "application/json")
                    .send()
                    .await;

                match response {
                    Ok(res) if res.status().is_success() => {
                        if let Ok(text) = res.text().await {
                            if let Ok(tokens) = serde_json::from_str::<Vec<JupiterLendToken>>(&text) {
                                available_lend_tokens.set(tokens);
                            } else {
                                error_message.set(Some("Failed to parse lend tokens".to_string()));
                            }
                        } else {
                            error_message.set(Some("Failed to read response".to_string()));
                        }
                    }
                    _ => {
                        error_message.set(Some("Failed to fetch lend tokens".to_string()));
                    }
                }
                fetching_tokens.set(false);
            });
        }
    });

    // Fetch positions if wallet is connected
    use_effect(move || {
        if let Some(address) = wallet_address() {
            if positions().is_empty() && !fetching_positions() {
                fetching_positions.set(true);
                
                let address = address.clone();
                spawn(async move {
                    let client = reqwest::Client::new();
                    let response = client
                        .get(format!("https://lite-api.jup.ag/lend/v1/earn/positions?users={}", address))
                        .header("Accept", "application/json")
                        .send()
                        .await;

                    match response {
                        Ok(res) if res.status().is_success() => {
                            if let Ok(text) = res.text().await {
                                if let Ok(pos) = serde_json::from_str::<Vec<Position>>(&text) {
                                    positions.set(pos);
                                } else {
                                    error_message.set(Some("Failed to parse positions".to_string()));
                                }
                            } else {
                                error_message.set(Some("Failed to read positions response".to_string()));
                            }
                        }
                        _ => {
                            error_message.set(Some("Failed to fetch positions".to_string()));
                        }
                    }
                    fetching_positions.set(false);
                });
            }
        }
    });

    // Fetch earnings if positions available
    use_effect(move || {
        if !positions().is_empty() && earnings().is_empty() && !fetching_earnings() {
            if let Some(address) = wallet_address() {
                fetching_earnings.set(true);
                
                let address = address.clone();
                let position_addresses = positions().iter().map(|p| p.token.address.clone()).collect::<Vec<_>>().join(",");
                spawn(async move {
                    let client = reqwest::Client::new();
                    let response = client
                        .get(format!("https://lite-api.jup.ag/lend/v1/earn/earnings?user={}&positions={}", address, position_addresses))
                        .header("Accept", "application/json")
                        .send()
                        .await;

                    match response {
                        Ok(res) if res.status().is_success() => {
                            if let Ok(text) = res.text().await {
                                if let Ok(earn) = serde_json::from_str::<Vec<Earning>>(&text) {
                                    earnings.set(earn);
                                } else {
                                    error_message.set(Some("Failed to parse earnings".to_string()));
                                }
                            } else {
                                error_message.set(Some("Failed to read earnings response".to_string()));
                            }
                        }
                        _ => {
                            error_message.set(Some("Failed to fetch earnings".to_string()));
                        }
                    }
                    fetching_earnings.set(false);
                });
            }
        }
    });

    // Update selected lend token
    use_effect(move || {
        if let Some(sym) = selected_symbol() {
            if let Some(token) = available_lend_tokens().iter().find(|t| {
                t.asset.get("symbol").and_then(|v| v.as_str()) == Some(&sym)
            }) {
                selected_lend_token.set(Some(token.clone()));
            }
        }
    });

    let has_hardware = hardware_wallet.is_some();

    // Helper to format big numbers with decimals
    let format_balance = |value: &str, decimals: i32| -> f64 {
        value.parse::<f64>().unwrap_or(0.0) / 10.0f64.powi(decimals)
    };

    // Format APY
    let format_apy = |rate_str: &str| -> String {
        if let Ok(rate) = rate_str.parse::<f64>() {
            format!("{:.2}%", rate / 100.0)
        } else {
            "N/A".to_string()
        }
    };

    // Format TVL
    let format_tvl = |lend_token: &JupiterLendToken| -> String {
        if let Ok(val) = lend_token.total_assets.parse::<f64>() {
            let decimals = lend_token.decimals as i32;
            let asset_amount = val / 10.0f64.powi(decimals);
            let price_str = lend_token.asset.get("price").and_then(|v| v.as_str()).unwrap_or("0");
            let price = price_str.parse::<f64>().unwrap_or(0.0);
            let tvl_usd = asset_amount * price;
            if tvl_usd >= 1_000_000_000.0 {
                format!("${:.1}B", tvl_usd / 1_000_000_000.0)
            } else if tvl_usd >= 1_000_000.0 {
                format!("${:.1}M", tvl_usd / 1_000_000.0)
            } else {
                format!("${:.0}", tvl_usd)
            }
        } else {
            "N/A".to_string()
        }
    };

    let tokens_clone = tokens.clone();
    // Get current wallet balance for symbol
    let current_balance = use_memo(move || {
        if let Some(sym) = selected_symbol() {
            tokens_clone.iter().find(|t| t.symbol == sym).map(|t| t.balance).unwrap_or(0.0)
        } else {
            0.0
        }
    });

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
                    width: min(480px, calc(100vw - 32px));
                    max-width: 480px;
                    max-height: calc(100vh - 64px);
                    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.8);
                    border: 1px solid rgba(255, 255, 255, 0.1);
                    overflow: hidden;
                    margin: 16px auto;
                    display: flex;
                    flex-direction: column;
                ",

                // Modal header
                if selected_symbol().is_some() {
                    div {
                        class: "lend-header",
                        style: "
                            display: flex;
                            justify-content: space-between;
                            align-items: center;
                            padding: 24px;
                            border-bottom: 1px solid rgba(255, 255, 255, 0.1);
                            background: transparent;
                        ",
                        button {
                            style: "
                                background: #3a3a3a;
                                border: 1px solid #5a5a5a;
                                color: #ffffff;
                                padding: 8px 16px;
                                border-radius: 8px;
                                cursor: pointer;
                                font-size: 14px;
                                font-weight: 500;
                                transition: all 0.2s ease;
                            ",
                            onclick: move |_| {
                                selected_symbol.set(None);
                                amount.set("".to_string());
                                error_message.set(None);
                            },
                            "← Back"
                        }
                        h2 {
                            style: "
                                color: #f8fafc;
                                font-size: 20px;
                                font-weight: 700;
                                margin: 0;
                                letter-spacing: -0.025em;
                            ",
                            "{mode.read().to_uppercase()} {selected_symbol().unwrap_or_default()}"
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
                } else {
                    div {
                        class: "lend-header",
                        style: "
                            display: flex;
                            justify-content: space-between;
                            align-items: center;
                            padding: 24px;
                            border-bottom: 1px solid rgba(255, 255, 255, 0.1);
                            background: transparent;
                        ",
                        h2 {
                            style: "
                                color: #f8fafc;
                                font-size: 22px;
                                font-weight: 700;
                                margin: 0;
                                letter-spacing: -0.025em;
                            ",
                            "Lend Tokens"
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

                div {
                    style: "
                        padding: 20px 24px;
                        overflow-y: auto;
                        flex: 1;
                    ",

                    if fetching_tokens() || fetching_positions() || fetching_earnings() {
                        div {
                            style: "
                                display: flex;
                                justify-content: center;
                                align-items: center;
                                padding: 40px;
                                color: #94a3b8;
                                font-size: 15px;
                            ",
                            "Loading data..."
                        }
                    } else if selected_symbol().is_none() {
                        div {
                            style: "display: flex; flex-direction: column; gap: 12px;",
                            {
                                available_lend_tokens().into_iter().map(move |lend_token| {
                                    let lend_token_clone_deposit = lend_token.clone();
                                    let lend_token_clone_withdraw = lend_token.clone();
                                    let symbol = lend_token.asset.get("symbol").and_then(|v| v.as_str()).unwrap_or(&lend_token.symbol).to_string();
                                    let symbol_deposit = symbol.clone();
                                    let symbol_withdraw = symbol.clone();
                                    let symbol_buy = symbol.clone();
                                    let logo_uri = lend_token.asset.get("logoUrl").and_then(|v| v.as_str()).unwrap_or("");
                                    let final_logo = if logo_uri.is_empty() {
                                        get_fallback_icon(&symbol)
                                    } else {
                                        logo_uri.to_string()
                                    };
                                    let apy = format_apy(&lend_token.total_rate);
                                    let tvl = format_tvl(&lend_token);
                                    let wallet_balance = tokens.iter().find(|t| t.symbol == symbol).map(|t| t.balance).unwrap_or(0.0);
                                    let position_opt = positions().iter().find(|p| p.token.address == lend_token.address).cloned();
                                    let earning_opt = earnings().iter().find(|e| e.address == lend_token.address).cloned();

                                    let position_balance = if let Some(pos) = &position_opt {
                                        format_balance(&pos.underlying_balance, lend_token.decimals)
                                    } else {
                                        0.0
                                    };
                                    let earnings_amount = if let Some(earn) = &earning_opt {
                                        format_balance(&earn.earnings, lend_token.decimals)
                                    } else {
                                        0.0
                                    };

                                    rsx! {
                                        div {
                                            style: "
                                                background: #1a1a1a;
                                                border: 1.5px solid #4a4a4a;
                                                border-radius: 12px;
                                                padding: 16px;
                                                transition: all 0.2s ease;
                                            ",
                                            div {
                                                style: "
                                                    display: flex;
                                                    align-items: center;
                                                    gap: 12px;
                                                    margin-bottom: 12px;
                                                ",
                                                img {
                                                    src: "{final_logo}",
                                                    alt: "{symbol}",
                                                    style: "width: 40px; height: 40px; border-radius: 50%;"
                                                }
                                                div {
                                                    style: "flex: 1;",
                                                    div {
                                                        style: "
                                                            color: #f8fafc;
                                                            font-size: 16px;
                                                            font-weight: 700;
                                                            margin-bottom: 4px;
                                                        ",
                                                        "{symbol}"
                                                    }
                                                    div {
                                                        style: "
                                                            color: #94a3b8;
                                                            font-size: 13px;
                                                        ",
                                                        "Wallet Balance: {wallet_balance:.2}"
                                                    }
                                                }
                                            }
                                            div {
                                                style: "
                                                    display: flex;
                                                    justify-content: space-between;
                                                    margin-bottom: 12px;
                                                    padding: 8px 0;
                                                ",
                                                span {
                                                    style: "
                                                        color: #10b981;
                                                        font-size: 15px;
                                                        font-weight: 600;
                                                    ",
                                                    "{apy} APY"
                                                }
                                                span {
                                                    style: "
                                                        color: #cbd5e1;
                                                        font-size: 13px;
                                                    ",
                                                    "TVL: {tvl}"
                                                }
                                            }
                                            if position_balance > 0.0 {
                                                div {
                                                    style: "
                                                        background: rgba(16, 185, 129, 0.1);
                                                        border: 1px solid rgba(16, 185, 129, 0.2);
                                                        border-radius: 8px;
                                                        padding: 10px 12px;
                                                        margin-bottom: 12px;
                                                    ",
                                                    div {
                                                        style: "
                                                            color: #cbd5e1;
                                                            font-size: 13px;
                                                            margin-bottom: 4px;
                                                        ",
                                                        "Your Position: {position_balance:.2} {symbol}"
                                                    }
                                                    div {
                                                        style: "
                                                            color: #10b981;
                                                            font-size: 13px;
                                                            font-weight: 600;
                                                        ",
                                                        "Earnings: {earnings_amount:.6} {symbol}"
                                                    }
                                                }
                                            }
                                            div {
                                                style: "
                                                    display: flex;
                                                    gap: 8px;
                                                ",
                                                button {
                                                    style: "
                                                        flex: 1;
                                                        background: white;
                                                        color: #1a1a1a;
                                                        border: none;
                                                        border-radius: 8px;
                                                        padding: 10px 16px;
                                                        font-size: 14px;
                                                        font-weight: 700;
                                                        cursor: pointer;
                                                        transition: all 0.2s ease;
                                                    ",
                                                    onclick: move |_| {
                                                        mode.set("deposit".to_string());
                                                        selected_symbol.set(Some(symbol_deposit.clone()));
                                                        selected_lend_token.set(Some(lend_token_clone_deposit.clone()));
                                                    },
                                                    "Deposit"
                                                }
                                                if position_balance > 0.0 {
                                                    button {
                                                        style: "
                                                            flex: 1;
                                                            background: #3a3a3a;
                                                            color: #ffffff;
                                                            border: 1px solid #5a5a5a;
                                                            border-radius: 8px;
                                                            padding: 10px 16px;
                                                            font-size: 14px;
                                                            font-weight: 700;
                                                            cursor: pointer;
                                                            transition: all 0.2s ease;
                                                        ",
                                                        onclick: move |_| {
                                                            mode.set("withdraw".to_string());
                                                            selected_symbol.set(Some(symbol_withdraw.clone()));
                                                            selected_lend_token.set(Some(lend_token_clone_withdraw.clone()));
                                                        },
                                                        "Withdraw"
                                                    }
                                                }
                                                if wallet_balance == 0.0 && position_balance == 0.0 {
                                                    button {
                                                        style: "
                                                            flex: 1;
                                                            background: #3a3a3a;
                                                            color: #ffffff;
                                                            border: 1px solid #5a5a5a;
                                                            border-radius: 8px;
                                                            padding: 10px 16px;
                                                            font-size: 14px;
                                                            font-weight: 700;
                                                            cursor: pointer;
                                                            transition: all 0.2s ease;
                                                        ",
                                                        onclick: move |_| {
                                                            println!("Buy {} clicked", symbol_buy);
                                                        },
                                                        "Buy"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                })
                            }
                            if available_lend_tokens().is_empty() {
                                div {
                                    style: "
                                        text-align: center;
                                        padding: 40px;
                                        color: #94a3b8;
                                        font-size: 15px;
                                    ",
                                    "No lending options available"
                                }
                            }
                        }
                    } else {
                        div {
                            style: "display: flex; flex-direction: column; gap: 16px;",

                            if let Some(lend_token) = selected_lend_token() {
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
                                            justify-content: space-between;
                                            margin-bottom: 10px;
                                        ",
                                        span {
                                            style: "color: #94a3b8; font-size: 14px;",
                                            "Current APY:"
                                        }
                                        span {
                                            style: "color: #10b981; font-size: 14px; font-weight: 600;",
                                            "{format_apy(&lend_token.total_rate)}"
                                        }
                                    }
                                    div {
                                        style: "
                                            display: flex;
                                            justify-content: space-between;
                                            margin-bottom: 10px;
                                        ",
                                        span {
                                            style: "color: #94a3b8; font-size: 14px;",
                                            "Supply Rate:"
                                        }
                                        span {
                                            style: "color: #cbd5e1; font-size: 14px;",
                                            "{format_apy(&lend_token.supply_rate)}"
                                        }
                                    }
                                    if !lend_token.rewards_rate.is_empty() && lend_token.rewards_rate != "0" {
                                        div {
                                            style: "
                                                display: flex;
                                                justify-content: space-between;
                                            ",
                                            span {
                                                style: "color: #94a3b8; font-size: 14px;",
                                                "Rewards Rate:"
                                            }
                                            span {
                                                style: "color: #cbd5e1; font-size: 14px;",
                                                "{format_apy(&lend_token.rewards_rate)}"
                                            }
                                        }
                                    }
                                }
                            }


                            div {
                                style: "display: flex; flex-direction: column; gap: 8px;",
                                div {
                                    style: "
                                        display: flex;
                                        justify-content: space-between;
                                        align-items: center;
                                        margin-bottom: 8px;
                                    ",
                                    label {
                                        style: "color: #94a3b8; font-size: 15px; font-weight: 500;",
                                        "Amount to {mode.read().to_uppercase()}"
                                    }
                                    div {
                                        style: "color: #cbd5e1; font-size: 13px;",
                                        if *mode.read() == "deposit" {
                                            "Balance: {tokens.iter().find(|t| t.symbol == selected_symbol().unwrap_or_default()).map(|t| t.balance).unwrap_or(0.0):.6} {selected_symbol().unwrap_or_default()}"
                                        } else {
                                            if let Some(pos) = positions().iter().find(|p| p.token.asset.get("symbol").and_then(|v| v.as_str()) == Some(&selected_symbol().unwrap_or_default())) {
                                                "Position: {format_balance(&pos.underlying_balance, pos.token.decimals):.6} {selected_symbol().unwrap_or_default()}"
                                            } else {
                                                "Position: 0.0 {selected_symbol().unwrap_or_default()}"
                                            }
                                        }
                                    }
                                }
                                div {
                                    style: "
                                        display: flex;
                                        align-items: center;
                                        background: #1a1a1a;
                                        border: 1.5px solid #4a4a4a;
                                        border-radius: 12px;
                                        padding: 16px;
                                        gap: 12px;
                                    ",
                                    input {
                                        r#type: "text",
                                        placeholder: "0.0",
                                        value: amount(),
                                        oninput: move |evt| amount.set(evt.value()),
                                        style: "
                                            background: transparent;
                                            border: none;
                                            color: #ffffff;
                                            font-size: 24px;
                                            font-weight: 700;
                                            width: 100%;
                                            outline: none;
                                        "
                                    }
                                    button {
                                        onclick: move |_| {
                                            let max = if *mode.read() == "deposit" {
                                                let bal = tokens.iter().find(|t| t.symbol == selected_symbol().unwrap_or_default()).map(|t| t.balance).unwrap_or(0.0);
                                                if selected_symbol().unwrap_or_default() == "SOL" && bal > 0.01 { bal - 0.01 } else { bal }
                                            } else {
                                                if let Some(pos) = positions().iter().find(|p| p.token.asset.get("symbol").and_then(|v| v.as_str()) == Some(&selected_symbol().unwrap_or_default())) {
                                                    format_balance(&pos.underlying_balance, selected_lend_token().as_ref().map(|t| t.decimals).unwrap_or(6))
                                                } else {
                                                    0.0
                                                }
                                            };
                                            amount.set(format!("{:.6}", max).trim_end_matches('0').trim_end_matches('.').to_string());
                                        },
                                        style: "
                                            background: #3a3a3a;
                                            border: 1px solid #5a5a5a;
                                            color: #ffffff;
                                            padding: 8px 16px;
                                            border-radius: 8px;
                                            cursor: pointer;
                                            font-size: 13px;
                                            font-weight: 700;
                                            white-space: nowrap;
                                        ",
                                        "MAX"
                                    }
                                }
                            }


                            if let Some(lend_token) = selected_lend_token() {
                                if let Some(earning) = earnings().iter().find(|e| e.address == lend_token.address) {
                                    div {
                                        style: "
                                            background: rgba(16, 185, 129, 0.1);
                                            border: 1px solid rgba(16, 185, 129, 0.2);
                                            border-radius: 10px;
                                            padding: 12px 16px;
                                            color: #10b981;
                                            font-size: 14px;
                                            font-weight: 600;
                                            text-align: center;
                                        ",
                                        "Total Earnings: {format_balance(&earning.earnings, lend_token.decimals):.6} {selected_symbol().unwrap_or_default()}"
                                    }
                                }
                            }

                            // Summary
                            {
                                let show_summary = !amount().is_empty() && amount().parse::<f64>().unwrap_or(0.0) > 0.0;
                                let lend_token_opt = selected_lend_token();

                                if show_summary && lend_token_opt.is_some() {
                                    let lend_token = lend_token_opt.unwrap();

                                    let yearly = if *mode.read() == "deposit" {
                                        let amt = amount().parse::<f64>().unwrap_or(0.0);
                                        let rate = lend_token.total_rate.parse::<f64>().unwrap_or(0.0) / 10000.0;
                                        amt * rate
                                    } else {
                                        0.0 // For withdraw, maybe show remaining earnings or something
                                    };

                                    rsx! {
                                        div {
                                            style: "
                                                background: #1a1a1a;
                                                border: 1.5px solid #4a4a4a;
                                                border-radius: 12px;
                                                padding: 16px;
                                            ",
                                            h4 {
                                                style: "
                                                    color: #f8fafc;
                                                    font-size: 16px;
                                                    font-weight: 700;
                                                    margin: 0 0 12px 0;
                                                ",
                                                "{mode.read().to_uppercase()} Summary"
                                            }
                                            div {
                                                style: "
                                                    display: flex;
                                                    justify-content: space-between;
                                                    margin-bottom: 8px;
                                                ",
                                                span {
                                                    style: "color: #94a3b8; font-size: 14px;",
                                                    "Amount:"
                                                }
                                                span {
                                                    style: "color: #cbd5e1; font-size: 14px;",
                                                    "{amount()} {selected_symbol().unwrap_or_default()}"
                                                }
                                            }
                                            div {
                                                style: "
                                                    display: flex;
                                                    justify-content: space-between;
                                                    margin-bottom: 8px;
                                                ",
                                                span {
                                                    style: "color: #94a3b8; font-size: 14px;",
                                                    "APY:"
                                                }
                                                span {
                                                    style: "color: #10b981; font-size: 14px; font-weight: 600;",
                                                    "{format_apy(&lend_token.total_rate)}"
                                                }
                                            }
                                            if *mode.read() == "deposit" {
                                                div {
                                                    style: "
                                                        display: flex;
                                                        justify-content: space-between;
                                                    ",
                                                    span {
                                                        style: "color: #94a3b8; font-size: 14px;",
                                                        "Est. yearly earnings:"
                                                    }
                                                    span {
                                                        style: "color: #10b981; font-size: 14px; font-weight: 600;",
                                                        "{yearly:.6} {selected_symbol().unwrap_or_default()}"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    rsx! { div { style: "display: none;" } }
                                }
                            }
                        }
                    }
                }


                if selected_symbol().is_some() {
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
                            onclick: move |_| {
                                selected_symbol.set(None);
                                amount.set("".to_string());
                                error_message.set(None);
                            },
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
                            disabled: {
                                let amt = amount().parse::<f64>().unwrap_or(0.0);
                                processing() || amount().is_empty() || amt <= 0.0 || amt > if *mode.read() == "deposit" { current_balance() } else {
                                    positions().iter().find(|p| p.token.asset.get("symbol").and_then(|v| v.as_str()) == Some(&selected_symbol().unwrap_or_default())).map(|p| format_balance(&p.underlying_balance, p.token.decimals)).unwrap_or(0.0)
                                } || selected_lend_token().is_none()
                            },
                            onclick: move |_| {
                                error_message.set(None);
                                
                                let amt_f64 = match amount().parse::<f64>() {
                                    Ok(a) if a > 0.0 => a,
                                    _ => {
                                        error_message.set(Some("Invalid amount".to_string()));
                                        return;
                                    }
                                };
                                
                                let max_available = if *mode.read() == "deposit" { current_balance() } else { 
                                    positions().iter().find(|p| p.token.asset.get("symbol").and_then(|v| v.as_str()) == Some(&selected_symbol().unwrap_or_default())).map(|p| format_balance(&p.underlying_balance, p.token.decimals)).unwrap_or(0.0) 
                                };
                                if amt_f64 > max_available {
                                    error_message.set(Some("Insufficient balance".to_string()));
                                    return;
                                };
                                
                                if selected_lend_token().is_none() {
                                    error_message.set(Some("No token selected".to_string()));
                                    return;
                                };
                                
                                processing.set(true);
                                if has_hardware {
                                    show_hardware_approval.set(true);
                                }
                                
                                let wallet_clone = wallet.clone();
                                let hardware_wallet_clone = hardware_wallet.clone();
                                let custom_rpc_clone = custom_rpc.clone();
                                let mode_clone = mode();
                                let amount_clone = amount();
                                let selected_lend_token_clone = selected_lend_token();
                                let wallet_address_clone = wallet_address();
                                
                                spawn(async move {
                                    let sig: String = match mode_clone.as_str() {
                                        "deposit" => {
                                            if let Some(lend_token) = selected_lend_token_clone {
                                                if let Some(signer_str) = wallet_address_clone {
                                                    let decimals = lend_token.decimals;
                                                    let amount_raw = ((amt_f64 * 10.0f64.powi(decimals)) as u64).to_string();
                                                    let asset = lend_token.asset_address.clone();
                                                    
                                                    let client = reqwest::Client::new();
                                                    let body = serde_json::json!({
                                                        "asset": asset,
                                                        "signer": signer_str,
                                                        "amount": amount_raw
                                                    });
                                                    let response = client
                                                        .post("https://lite-api.jup.ag/lend/v1/earn/deposit")
                                                        .header(header::CONTENT_TYPE, "application/json")
                                                        .header(header::ACCEPT, "application/json")
                                                        .json(&body)
                                                        .send()
                                                        .await;
                                                    
                                                    let tx_base64 = match response {
                                                        Ok(res) if res.status().is_success() => {
                                                            if let Ok(json) = res.json::<serde_json::Value>().await {
                                                                json.get("transaction").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_default()
                                                            } else {
                                                                String::new()
                                                            }
                                                        }
                                                        Ok(res) => {
                                                            format!("Request failed with status: {}", res.status())
                                                        }
                                                        Err(e) => {
                                                            format!("Failed to get response: {}", e)
                                                        }
                                                    };
                                                    
                                                    if tx_base64.is_empty() {
                                                        "No transaction received".to_string()
                                                    } else {
                                                        let is_hardware = hardware_wallet_clone.is_some();
                                                        was_hardware_transaction.set(is_hardware);
                                                        
                                                        let signer_result = if is_hardware {
                                                            if let Some(hw) = hardware_wallet_clone {
                                                                let hw_signer = HardwareSigner::from_wallet(hw);
                                                                sign_jupiter_lend_transaction(&hw_signer, &tx_base64).await
                                                            } else {
                                                                Err("No hardware wallet".to_string())
                                                            }
                                                        } else if let Some(w) = wallet_clone {
                                                            match Wallet::from_wallet_info(&w) {
                                                                Ok(wallet) => {
                                                                    let sw_signer = SoftwareSigner::new(wallet);
                                                                    sign_jupiter_lend_transaction(&sw_signer, &tx_base64).await
                                                                }
                                                                Err(e) => Err(format!("Failed to load wallet: {}", e))
                                                            }
                                                        } else {
                                                            Err("No wallet available".to_string())
                                                        };
                                                        
                                                        match signer_result {
                                                            Ok(signed_b64) => {
                                                                let rpc_url = custom_rpc_clone.unwrap_or("https://johna-k3cr1v-fast-mainnet.helius-rpc.com".to_string());
                                                                match execute_jupiter_lend_transaction(signed_b64, rpc_url).await {
                                                                    Ok(sig) => sig,
                                                                    Err(e) => e
                                                                }
                                                            }
                                                            Err(e) => e
                                                        }
                                                    }
                                                } else {
                                                    "No wallet address".to_string()
                                                }
                                            } else {
                                                "No selected token".to_string()
                                            }
                                        }
                                        "withdraw" => {
                                            if let Some(lend_token) = selected_lend_token_clone {
                                                if let Some(signer_str) = wallet_address_clone {
                                                    let decimals = lend_token.decimals;
                                                    let amount_raw = ((amt_f64 * 10.0f64.powi(decimals)) as u64).to_string();
                                                    let asset = lend_token.asset_address.clone();
                                                    
                                                    let client = reqwest::Client::new();
                                                    let body = serde_json::json!({
                                                        "asset": asset,
                                                        "signer": signer_str,
                                                        "amount": amount_raw
                                                    });
                                                    let response = client
                                                        .post("https://lite-api.jup.ag/lend/v1/earn/withdraw")
                                                        .header(header::CONTENT_TYPE, "application/json")
                                                        .header(header::ACCEPT, "application/json")
                                                        .json(&body)
                                                        .send()
                                                        .await;
                                                    
                                                    let tx_base64 = match response {
                                                        Ok(res) if res.status().is_success() => {
                                                            if let Ok(json) = res.json::<serde_json::Value>().await {
                                                                json.get("transaction").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_default()
                                                            } else {
                                                                String::new()
                                                            }
                                                        }
                                                        Ok(res) => {
                                                            format!("Request failed with status: {}", res.status())
                                                        }
                                                        Err(e) => {
                                                            format!("Failed to get response: {}", e)
                                                        }
                                                    };
                                                    
                                                    if tx_base64.is_empty() {
                                                        "No transaction received".to_string()
                                                    } else {
                                                        let is_hardware = hardware_wallet_clone.is_some();
                                                        was_hardware_transaction.set(is_hardware);
                                                        
                                                        let signer_result = if is_hardware {
                                                            if let Some(hw) = hardware_wallet_clone {
                                                                let hw_signer = HardwareSigner::from_wallet(hw);
                                                                sign_jupiter_lend_transaction(&hw_signer, &tx_base64).await
                                                            } else {
                                                                Err("No hardware wallet".to_string())
                                                            }
                                                        } else if let Some(w) = wallet_clone {
                                                            match Wallet::from_wallet_info(&w) {
                                                                Ok(wallet) => {
                                                                    let sw_signer = SoftwareSigner::new(wallet);
                                                                    sign_jupiter_lend_transaction(&sw_signer, &tx_base64).await
                                                                }
                                                                Err(e) => Err(format!("Failed to load wallet: {}", e))
                                                            }
                                                        } else {
                                                            Err("No wallet available".to_string())
                                                        };
                                                        
                                                        match signer_result {
                                                            Ok(signed_b64) => {
                                                                let rpc_url = custom_rpc_clone.unwrap_or("https://johna-k3cr1v-fast-mainnet.helius-rpc.com".to_string());
                                                                match execute_jupiter_lend_transaction(signed_b64, rpc_url).await {
                                                                    Ok(sig) => sig,
                                                                    Err(e) => e
                                                                }
                                                            }
                                                            Err(e) => e
                                                        }
                                                    }
                                                } else {
                                                    "No wallet address".to_string()
                                                }
                                            } else {
                                                "No selected token".to_string()
                                            }
                                        }
                                        _ => {
                                            // Mock for other modes if any
                                            #[cfg(target_arch = "wasm32")]
                                            gloo_timers::future::TimeoutFuture::new(2000).await;
                                            "mock_tx_sig".to_string()
                                        }
                                    };
                                    
                                    transaction_signature.set(sig);
                                    processing.set(false);
                                    show_hardware_approval.set(false);
                                    show_success_modal.set(true);
                                });
                            },
                            if processing() { if *mode.read() == "deposit" { "Depositing..." } else { "Withdrawing..." } } else { if *mode.read() == "deposit" { "Deposit" } else { "Withdraw" } }
                        }
                    }
                }
            }
        }
        if show_success_modal() {
            LendTransactionSuccessModal {
                signature: transaction_signature(),
                lending_token: selected_symbol().unwrap_or_default(),
                lending_amount: amount(),
                apy: selected_lend_token().map(|t| format_apy(&t.total_rate)).unwrap_or("N/A".to_string()),
                was_hardware_wallet: was_hardware_transaction(),
                onclose: move |_| {
                    show_success_modal.set(false);
                    onsuccess.call(transaction_signature());
                },
            }
        }
    }
}

#[component]
pub fn LendTransactionSuccessModal(
    signature: String,
    lending_token: String,
    lending_amount: String,
    apy: String,
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
                    "Transaction Completed Successfully! 🎉"
                }

                div {
                    style: "
                        padding: 0 24px 20px;
                        text-align: center;
                    ",
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
                    p {
                        style: "
                            color: #cbd5e1;
                            font-size: 15px;
                            margin: 0 0 8px 0;
                        ",
                        "Your {lending_amount} {lending_token} has been processed at {apy} APY."
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
                            style: "
                                display: flex;
                                flex-direction: column;
                                gap: 8px;
                            ",
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
                    style: "
                        padding: 0 24px 24px;
                    ",
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