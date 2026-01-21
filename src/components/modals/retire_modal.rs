use dioxus::prelude::*;
use crate::components::address_input::AddressInput;
use crate::components::common::Token;
use crate::hardware::HardwareWallet;
use crate::rpc;
use crate::wallet::{Wallet, WalletInfo};
use crate::signing::{SignerType, TransactionSigner};
use crate::transaction::TransactionClient;
use base64::Engine;
use solana_sdk::{
    message::Message,
    pubkey::Pubkey,
    signature::Signature,
    transaction::{Transaction, VersionedTransaction},
};
use solana_system_interface::instruction as system_instruction;
use spl_token::instruction as token_instruction;
use spl_associated_token_account::get_associated_token_address_with_program_id;
use std::str::FromStr;
use std::sync::Arc;

const SOL_MINT: &str = "So11111111111111111111111111111111111111112";
const TOKEN_2022_PROGRAM_ID: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";

#[derive(Clone, Debug, PartialEq)]
pub struct RetireResult {
    pub signature: String,
    pub remove_from_storage: bool,
    pub residual_balance: f64,
}

#[derive(Debug, Clone, PartialEq)]
enum RetireTokenStatus {
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

impl RetireTokenStatus {
    fn is_complete(&self) -> bool {
        matches!(
            self,
            RetireTokenStatus::Complete { .. }
                | RetireTokenStatus::Failed { .. }
                | RetireTokenStatus::CloseFailed { .. }
        )
    }

    fn status_text(&self) -> String {
        match self {
            RetireTokenStatus::Pending => "Waiting...".to_string(),
            RetireTokenStatus::FetchingQuote => "Fetching swap quote...".to_string(),
            RetireTokenStatus::SwappingToSol => "Swapping to SOL...".to_string(),
            RetireTokenStatus::SwapSuccess { sol_received } => format!("Swapped -> {:.4} SOL", sol_received),
            RetireTokenStatus::SwapFailed { reason } => format!("Swap failed: {}", reason),
            RetireTokenStatus::ClosingAccount => "Closing token account...".to_string(),
            RetireTokenStatus::AccountClosed { rent_reclaimed } => format!("Closed -> {:.6} SOL rent", rent_reclaimed),
            RetireTokenStatus::CloseFailed { reason } => format!("Close failed: {}", reason),
            RetireTokenStatus::Complete { sol_received, rent_reclaimed } => {
                format!("Complete: {:.4} SOL + {:.6} rent", sol_received, rent_reclaimed)
            }
            RetireTokenStatus::Failed { reason } => format!("Failed: {}", reason),
        }
    }

    fn status_color(&self) -> &str {
        match self {
            RetireTokenStatus::Pending => "#94a3b8",
            RetireTokenStatus::FetchingQuote
            | RetireTokenStatus::SwappingToSol
            | RetireTokenStatus::ClosingAccount => "#3b82f6",
            RetireTokenStatus::SwapSuccess { .. } | RetireTokenStatus::AccountClosed { .. } => "#10b981",
            RetireTokenStatus::Complete { .. } => "#10b981",
            RetireTokenStatus::SwapFailed { .. }
            | RetireTokenStatus::CloseFailed { .. }
            | RetireTokenStatus::Failed { .. } => "#ef4444",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct RetireTokenItem {
    token: Token,
    status: RetireTokenStatus,
}

#[derive(Debug, serde::Deserialize)]
struct JupiterOrderResponse {
    #[serde(rename = "outAmount")]
    out_amount: String,
    transaction: Option<String>,
}

async fn try_swap_to_sol(
    token_mint: &str,
    amount_lamports: u64,
    user_pubkey: &str,
) -> Result<(String, f64), String> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://lite-api.jup.ag/ultra/v1/order?inputMint={}&outputMint={}&amount={}&taker={}",
        token_mint, SOL_MINT, amount_lamports, user_pubkey
    );

    match client.get(&url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<JupiterOrderResponse>().await {
                    Ok(order) => {
                        if let Some(tx) = order.transaction {
                            let sol_out: f64 = order.out_amount.parse().unwrap_or(0.0) / 1_000_000_000.0;
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

async fn get_mint_program_id(
    mint_pubkey: &Pubkey,
    rpc_url: Option<&str>,
) -> Result<Pubkey, String> {
    let client = reqwest::Client::new();
    let url = rpc_url.unwrap_or("https://johna-k3cr1v-fast-mainnet.helius-rpc.com");
    let request = serde_json::json!({
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

    let response = client
        .post(url)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("RPC error: {}", e))?;
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("RPC parse error: {}", e))?;

    if let Some(owner_str) = json["result"]["value"]["owner"].as_str() {
        let owner = Pubkey::from_str(owner_str).map_err(|e| format!("Invalid owner: {}", e))?;
        let token_2022 = Pubkey::from_str(TOKEN_2022_PROGRAM_ID)
            .map_err(|e| format!("Invalid token-2022 ID: {}", e))?;
        if owner == token_2022 {
            Ok(token_2022)
        } else {
            Ok(spl_token::id())
        }
    } else {
        Ok(spl_token::id())
    }
}

async fn sign_and_execute_transaction(
    unsigned_tx_b64: &str,
    signer: &dyn TransactionSigner,
    tx_client: &TransactionClient,
) -> Result<String, String> {
    let unsigned_tx_bytes = base64::decode(unsigned_tx_b64)
        .map_err(|e| format!("Failed to decode transaction: {}", e))?;
    let mut transaction: VersionedTransaction = bincode::deserialize(&unsigned_tx_bytes)
        .map_err(|e| format!("Failed to deserialize transaction: {}", e))?;

    let message_bytes = transaction.message.serialize();
    let signature_bytes = signer.sign_message(&message_bytes).await
        .map_err(|e| format!("Failed to sign: {}", e))?;

    let mut sig_array = [0u8; 64];
    sig_array.copy_from_slice(&signature_bytes);
    transaction.signatures[0] = Signature::from(sig_array);

    let signed_tx_bytes = bincode::serialize(&transaction)
        .map_err(|e| format!("Failed to serialize signed transaction: {}", e))?;
    let signed_tx_b58 = bs58::encode(&signed_tx_bytes).into_string();
    tx_client.send_transaction(&signed_tx_b58).await
        .map_err(|e| format!("{:?}", e))
}

async fn close_token_account(
    token: &Token,
    owner: &Pubkey,
    signer: &dyn TransactionSigner,
    tx_client: &TransactionClient,
    rpc_url: Option<&str>,
) -> Result<(String, f64), String> {
    let token_mint = Pubkey::from_str(&token.mint)
        .map_err(|e| format!("Invalid mint: {}", e))?;
    let token_program_id = get_mint_program_id(&token_mint, rpc_url).await?;
    let token_account = get_associated_token_address_with_program_id(
        owner,
        &token_mint,
        &token_program_id,
    );

    let close_instruction = token_instruction::close_account(
        &token_program_id,
        &token_account,
        owner,
        owner,
        &[],
    )
    .map_err(|e| format!("Failed to create close instruction: {}", e))?;

    let recent_blockhash = tx_client.get_recent_blockhash().await
        .map_err(|e| format!("Failed to get blockhash: {:?}", e))?;
    let message = Message::new(&[close_instruction], Some(owner));
    let mut transaction = Transaction::new_unsigned(message);
    transaction.message.recent_blockhash = recent_blockhash;

    let message_bytes = bincode::serialize(&transaction.message)
        .map_err(|e| format!("Failed to serialize message: {}", e))?;
    let signature_bytes = signer.sign_message(&message_bytes).await
        .map_err(|e| format!("Failed to sign: {}", e))?;
    let mut sig_array = [0u8; 64];
    sig_array.copy_from_slice(&signature_bytes);
    transaction.signatures = vec![Signature::from(sig_array)];

    let signed_tx_bytes = bincode::serialize(&transaction)
        .map_err(|e| format!("Failed to serialize transaction: {}", e))?;
    let signed_tx_b58 = bs58::encode(&signed_tx_bytes).into_string();
    let signature = tx_client.send_transaction(&signed_tx_b58).await
        .map_err(|e| format!("{:?}", e))?;

    let rent_reclaimed = 0.00203928;
    Ok((signature, rent_reclaimed))
}

async fn send_sol_to_recipient(
    sender: &Pubkey,
    recipient: &Pubkey,
    amount_sol: f64,
    signer: &dyn TransactionSigner,
    tx_client: &TransactionClient,
    fee_buffer: f64,
) -> Result<String, String> {
    let amount_to_send = amount_sol - fee_buffer;

    if amount_to_send <= 0.0 {
        return Err("Insufficient SOL to send after accounting for fees".to_string());
    }

    let lamports = (amount_to_send * 1_000_000_000.0).floor() as u64;
    let transfer_instruction = system_instruction::transfer(
        sender,
        recipient,
        lamports,
    );

    let recent_blockhash = tx_client.get_recent_blockhash().await
        .map_err(|e| format!("Failed to get blockhash: {:?}", e))?;
    let message = Message::new(&[transfer_instruction], Some(sender));
    let mut transaction = Transaction::new_unsigned(message);
    transaction.message.recent_blockhash = recent_blockhash;

    let message_bytes = bincode::serialize(&transaction.message)
        .map_err(|e| format!("Failed to serialize message: {}", e))?;
    let signature_bytes = signer.sign_message(&message_bytes).await
        .map_err(|e| format!("Failed to sign: {}", e))?;
    let mut sig_array = [0u8; 64];
    sig_array.copy_from_slice(&signature_bytes);
    transaction.signatures = vec![Signature::from(sig_array)];

    let signed_tx_bytes = bincode::serialize(&transaction)
        .map_err(|e| format!("Failed to serialize transaction: {}", e))?;
    let signed_tx_b58 = bs58::encode(&signed_tx_bytes).into_string();
    tx_client.send_transaction(&signed_tx_b58).await
        .map_err(|e| format!("{:?}", e))
}

async fn get_balance_lamports(address: &str, rpc_url: Option<&str>) -> Result<u64, String> {
    let client = reqwest::Client::new();
    let url = rpc_url.unwrap_or("https://johna-k3cr1v-fast-mainnet.helius-rpc.com");
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getBalance",
        "params": [
            address,
            { "commitment": "finalized" }
        ]
    });

    let response = client
        .post(url)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("RPC error: {}", e))?;
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("RPC parse error: {}", e))?;

    if let Some(error) = json.get("error") {
        return Err(format!("RPC error: {:?}", error));
    }

    if let Some(value) = json["result"]["value"].as_u64() {
        Ok(value)
    } else {
        Err(format!("Failed to parse balance from response: {:?}", json))
    }
}

async fn get_fee_for_message(message: &Message, rpc_url: Option<&str>) -> Result<u64, String> {
    let client = reqwest::Client::new();
    let url = rpc_url.unwrap_or("https://johna-k3cr1v-fast-mainnet.helius-rpc.com");
    let message_bytes = message.serialize();
    let message_b64 = base64::engine::general_purpose::STANDARD.encode(message_bytes);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getFeeForMessage",
        "params": [
            message_b64,
            { "commitment": "processed" }
        ]
    });

    let response = client
        .post(url)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("RPC error: {}", e))?;
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("RPC parse error: {}", e))?;

    if let Some(error) = json.get("error") {
        return Err(format!("RPC error: {:?}", error));
    }

    if let Some(value) = json["result"]["value"].as_u64() {
        Ok(value)
    } else {
        Err(format!("Failed to parse fee from response: {:?}", json))
    }
}

async fn send_all_sol_to_recipient(
    sender: &Pubkey,
    recipient: &Pubkey,
    signer: &dyn TransactionSigner,
    tx_client: &TransactionClient,
    rpc_url: Option<&str>,
) -> Result<(String, f64), String> {
    let mut last_signature = "RETIRE_COMPLETED".to_string();
    let mut total_sent_lamports: u64 = 0;

    for _ in 0..3 {
        let balance_lamports = get_balance_lamports(&sender.to_string(), rpc_url).await?;
        if balance_lamports == 0 {
            return Ok((last_signature, total_sent_lamports as f64 / 1_000_000_000.0));
        }

        let recent_blockhash = tx_client.get_recent_blockhash().await
            .map_err(|e| format!("Failed to get blockhash: {:?}", e))?;
        let fee_message = {
            let ix = system_instruction::transfer(sender, recipient, 0);
            let mut message = Message::new(&[ix], Some(sender));
            message.recent_blockhash = recent_blockhash;
            message
        };

        let fee_lamports = get_fee_for_message(&fee_message, rpc_url).await?;
        if balance_lamports <= fee_lamports {
            return Err("Insufficient SOL to cover transfer fee".to_string());
        }

        let lamports_to_send = balance_lamports - fee_lamports;
        let transfer_instruction = system_instruction::transfer(
            sender,
            recipient,
            lamports_to_send,
        );

        let mut message = Message::new(&[transfer_instruction], Some(sender));
        message.recent_blockhash = recent_blockhash;
        let message_bytes = message.serialize();

        let signature_bytes = signer.sign_message(&message_bytes).await
            .map_err(|e| format!("Failed to sign: {}", e))?;
        let mut sig_array = [0u8; 64];
        sig_array.copy_from_slice(&signature_bytes);

        let mut transaction = Transaction::new_unsigned(message);
        transaction.signatures = vec![Signature::from(sig_array)];

        let signed_tx_bytes = bincode::serialize(&transaction)
            .map_err(|e| format!("Failed to serialize transaction: {}", e))?;
        let signed_tx_b58 = bs58::encode(&signed_tx_bytes).into_string();
        let signature = tx_client.send_transaction(&signed_tx_b58).await
            .map_err(|e| format!("{:?}", e))?;

        last_signature = signature;
        total_sent_lamports = total_sent_lamports.saturating_add(lamports_to_send);

        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    }

    Ok((last_signature, total_sent_lamports as f64 / 1_000_000_000.0))
}

async fn execute_retire<F>(
    tokens: Vec<Token>,
    wallet: Option<WalletInfo>,
    custom_rpc: Option<String>,
    recipient: Pubkey,
    mut status_callback: F,
) -> Result<(String, f64, f64), String>
where
    F: FnMut(usize, RetireTokenStatus),
{
    let wallet_info = wallet.ok_or_else(|| "No wallet available".to_string())?;
    let signer = SignerType::from_wallet(
        Wallet::from_wallet_info(&wallet_info)
            .map_err(|e| format!("Failed to load wallet: {}", e))?
    );
    let signer: Box<dyn TransactionSigner> = Box::new(signer);

    let user_pubkey_str = signer.get_public_key().await
        .map_err(|e| format!("Failed to get public key: {}", e))?;
    let user_pubkey = Pubkey::from_str(&user_pubkey_str)
        .map_err(|e| format!("Invalid public key: {}", e))?;

    let tx_client = TransactionClient::new(custom_rpc.as_deref());
    let mut last_signature = String::new();

    for (index, token) in tokens.iter().enumerate() {
        let amount_lamports = (token.balance * 10_f64.powi(token.decimals as i32)).round() as u64;

        if amount_lamports > 0 {
            status_callback(index, RetireTokenStatus::FetchingQuote);
            match try_swap_to_sol(&token.mint, amount_lamports, &user_pubkey_str).await {
                Ok((unsigned_tx_b64, sol_out)) => {
                    status_callback(index, RetireTokenStatus::SwappingToSol);
                    match sign_and_execute_transaction(&unsigned_tx_b64, &*signer, &tx_client).await {
                        Ok(signature) => {
                            last_signature = signature;
                            status_callback(index, RetireTokenStatus::SwapSuccess { sol_received: sol_out });
                            status_callback(index, RetireTokenStatus::ClosingAccount);
                            match close_token_account(token, &user_pubkey, &*signer, &tx_client, custom_rpc.as_deref()).await {
                                Ok((close_sig, rent)) => {
                                    last_signature = close_sig;
                                    status_callback(index, RetireTokenStatus::Complete {
                                        sol_received: sol_out,
                                        rent_reclaimed: rent,
                                    });
                                }
                                Err(e) => {
                                    status_callback(index, RetireTokenStatus::CloseFailed { reason: e });
                                }
                            }
                        }
                        Err(e) => {
                            status_callback(index, RetireTokenStatus::SwapFailed { reason: e.clone() });
                            status_callback(index, RetireTokenStatus::ClosingAccount);
                            match close_token_account(token, &user_pubkey, &*signer, &tx_client, custom_rpc.as_deref()).await {
                                Ok((close_sig, rent)) => {
                                    last_signature = close_sig;
                                    status_callback(index, RetireTokenStatus::Complete {
                                        sol_received: 0.0,
                                        rent_reclaimed: rent,
                                    });
                                }
                                Err(e) => {
                                    status_callback(index, RetireTokenStatus::Failed { reason: e });
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    status_callback(index, RetireTokenStatus::SwapFailed { reason: e });
                    status_callback(index, RetireTokenStatus::ClosingAccount);
                    match close_token_account(token, &user_pubkey, &*signer, &tx_client, custom_rpc.as_deref()).await {
                        Ok((close_sig, rent)) => {
                            last_signature = close_sig;
                            status_callback(index, RetireTokenStatus::Complete {
                                sol_received: 0.0,
                                rent_reclaimed: rent,
                            });
                        }
                        Err(e) => {
                            status_callback(index, RetireTokenStatus::Failed { reason: e });
                        }
                    }
                }
            }
        } else {
            status_callback(index, RetireTokenStatus::ClosingAccount);
            match close_token_account(token, &user_pubkey, &*signer, &tx_client, custom_rpc.as_deref()).await {
                Ok((close_sig, rent)) => {
                    last_signature = close_sig;
                    status_callback(index, RetireTokenStatus::Complete {
                        sol_received: 0.0,
                        rent_reclaimed: rent,
                    });
                }
                Err(e) => {
                    status_callback(index, RetireTokenStatus::Failed { reason: e });
                }
            }
        }
    }

    let (send_signature, total_sent) = send_all_sol_to_recipient(
        &user_pubkey,
        &recipient,
        &*signer,
        &tx_client,
        custom_rpc.as_deref(),
    ).await?;
    last_signature = send_signature.clone();

    let mut final_balance = None;
    for _ in 0..3 {
        if let Ok(balance) = rpc::get_balance(&user_pubkey_str, custom_rpc.as_deref()).await {
            final_balance = Some(balance);
            if balance <= 0.00001 {
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    }

    Ok((last_signature, total_sent, final_balance.unwrap_or(0.0)))
}

#[component]
fn RetireProcessingModal(
    tokens: Vec<RetireTokenItem>,
    current_step: String,
    is_complete: bool,
    total_sol_sent: f64,
    final_signature: String,
    residual_balance: Option<f64>,
    onhide: EventHandler<()>,
    onclose: EventHandler<()>,
) -> Element {
    let show_success = is_complete;

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
                        if show_success { "RETIRE Complete" } else { "Processing RETIRE..." }
                    }
                    div {
                        style: "
                            color: #94a3b8;
                            font-size: 14px;
                        ",
                        if show_success {
                            "Wallet retired and SOL swept"
                        } else {
                            "{current_step}"
                        }
                    }
                }

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
                            img {
                                src: "{item.token.icon_type}",
                                alt: "{item.token.symbol}",
                                style: "width: 32px; height: 32px; border-radius: 50%;"
                            }
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
                                    if matches!(item.status, RetireTokenStatus::Complete { .. }) {
                                        "OK"
                                    } else {
                                        "ERR"
                                    }
                                }
                            }
                        }
                    }
                }

                if show_success {
                    div {
                        style: "
                            padding: 20px 24px;
                            border-top: 1px solid rgba(255, 255, 255, 0.1);
                            background: #1a1a1a;
                        ",
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
                                "Total SOL Sent"
                            }
                            div {
                                style: "
                                    color: #10b981;
                                    font-size: 24px;
                                    font-weight: 700;
                                ",
                                "{total_sol_sent:.6} SOL"
                            }
                        }

                        if !final_signature.is_empty() {
                            div {
                                style: "margin-bottom: 16px;",
                                div {
                                    style: "
                                        color: #94a3b8;
                                        font-size: 12px;
                                        margin-bottom: 6px;
                                    ",
                                    "Final Transaction Signature:"
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
                        }

                        if let Some(balance) = residual_balance {
                            div {
                                style: "
                                    margin-top: 12px;
                                    color: #fbbf24;
                                    font-size: 12px;
                                ",
                                "Residual balance after RETIRE: {balance:.8} SOL"
                            }
                        }
                    }
                }

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
                            onclick: move |_| onhide.call(()),
                            "Hide"
                        }
                    }
                }

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

#[component]
pub fn RetireModal(
    all_tokens: Vec<Token>,
    wallet: Option<WalletInfo>,
    hardware_wallet: Option<Arc<HardwareWallet>>,
    current_balance: f64,
    sol_price: f64,
    custom_rpc: Option<String>,
    onclose: EventHandler<()>,
    onsuccess: EventHandler<RetireResult>,
) -> Element {
    let mut processing = use_signal(|| false);
    let mut error_message = use_signal(|| None as Option<String>);
    let mut retire_items = use_signal(|| Vec::<RetireTokenItem>::new());
    let mut current_step_text = use_signal(|| "Preparing...".to_string());
    let mut recipient = use_signal(|| "".to_string());
    let mut resolved_recipient = use_signal(|| Option::<Pubkey>::None);
    let mut show_processing_modal = use_signal(|| false);
    let mut processing_complete = use_signal(|| false);
    let mut transaction_signature = use_signal(|| "".to_string());
    let mut total_sol_sent = use_signal(|| 0.0);
    let mut residual_balance = use_signal(|| None as Option<f64>);
    let mut remove_from_storage = use_signal(|| true);
    let mut confirm_text = use_signal(|| "".to_string());

    let selected_tokens = use_memo(move || {
        all_tokens.iter()
            .filter(|token| token.mint != SOL_MINT && token.balance > 0.0)
            .cloned()
            .collect::<Vec<Token>>()
    });

    let tokens_without_price = use_memo(move || {
        selected_tokens()
            .iter()
            .filter(|token| token.value_usd <= 0.0)
            .count()
    });

    use_effect(move || {
        if retire_items().is_empty() && !selected_tokens().is_empty() {
            let items: Vec<RetireTokenItem> = selected_tokens()
                .iter()
                .map(|token| RetireTokenItem {
                    token: token.clone(),
                    status: RetireTokenStatus::Pending,
                })
                .collect();
            retire_items.set(items);
        }
    });

    let estimated_rent_reclaim = use_memo(move || {
        selected_tokens().len() as f64 * 0.00203928
    });

    let estimated_swap_sol = use_memo(move || {
        let total_value_usd = selected_tokens().iter().fold(0.0, |acc, token| acc + token.value_usd);
        if sol_price > 0.0 {
            total_value_usd / sol_price
        } else {
            0.0
        }
    });

    let estimated_total_sol = use_memo(move || {
        current_balance + estimated_swap_sol() + estimated_rent_reclaim()
    });

    if show_processing_modal() {
        return rsx! {
            RetireProcessingModal {
                tokens: retire_items(),
                current_step: current_step_text(),
                is_complete: processing_complete(),
                total_sol_sent: total_sol_sent(),
                final_signature: transaction_signature(),
                residual_balance: residual_balance(),
                onhide: move |_| {
                    show_processing_modal.set(false);
                },
                onclose: move |_| {
                    show_processing_modal.set(false);
                    processing_complete.set(false);
                    onsuccess.call(RetireResult {
                        signature: transaction_signature(),
                        remove_from_storage: remove_from_storage(),
                        residual_balance: residual_balance().unwrap_or(0.0),
                    });
                }
            }
        };
    }

    let hardware_blocked = hardware_wallet.is_some();
    let wallet_for_confirm = wallet.clone();
    let confirm_phrase = use_memo(move || {
        if let Some(info) = wallet_for_confirm.as_ref() {
            info.name.clone()
        } else {
            "RETIRE".to_string()
        }
    });
    let confirm_ok = use_memo(move || {
        let binding = confirm_text();
        let text = binding.trim();
        if text.eq_ignore_ascii_case("RETIRE") {
            return true;
        }
        text.eq_ignore_ascii_case(&confirm_phrase())
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
                    width: min(560px, calc(100vw - 32px));
                    max-width: 560px;
                    max-height: calc(100vh - 64px);
                    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.8);
                    border: 1px solid rgba(255, 255, 255, 0.1);
                    overflow: hidden;
                    margin: 16px auto;
                    display: flex;
                    flex-direction: column;
                ",

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
                        "RETIRE Wallet"
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
                        "x"
                    }
                }

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

                if hardware_blocked {
                    div {
                        style: "
                            padding: 12px 16px;
                            background-color: rgba(245, 158, 11, 0.1);
                            border: 1px solid #f59e0b;
                            color: #fbbf24;
                            border-radius: 10px;
                            margin: 16px 24px;
                            font-size: 13px;
                            text-align: center;
                        ",
                        "RETIRE is software-wallet only. Disconnect hardware wallet to proceed."
                    }
                }

                div {
                    style: "
                        padding: 20px 24px;
                        overflow-y: auto;
                        flex: 1;
                    ",
                    div {
                        style: "
                            background: rgba(244, 63, 94, 0.1);
                            border: 1px solid rgba(244, 63, 94, 0.2);
                            border-radius: 12px;
                            padding: 16px;
                            margin-bottom: 20px;
                        ",
                        div {
                            style: "color: #fecaca; font-size: 14px; font-weight: 600; margin-bottom: 8px;",
                            "This will retire your wallet"
                        }
                        div {
                            style: "color: #e2e8f0; font-size: 13px; line-height: 1.5;",
                            "All tokens will be swapped to SOL when possible, token accounts closed, and all SOL sent out. This is irreversible."
                        }
                    }

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
                            span { style: "color: #94a3b8; font-size: 14px;", "Tokens to retire:" }
                            span { style: "color: #cbd5e1; font-size: 14px; font-weight: 600;", "{selected_tokens().len()}" }
                        }
                        div {
                            style: "
                                display: flex;
                                justify-content: space-between;
                                margin-bottom: 12px;
                            ",
                            span { style: "color: #94a3b8; font-size: 14px;", "Estimated rent reclaim:" }
                            span { style: "color: #10b981; font-size: 14px; font-weight: 600;", "{estimated_rent_reclaim():.6} SOL" }
                        }
                        div {
                            style: "
                                display: flex;
                                justify-content: space-between;
                                margin-bottom: 12px;
                            ",
                            span { style: "color: #94a3b8; font-size: 14px;", "Estimated SOL after swaps:" }
                            span { style: "color: #cbd5e1; font-size: 14px;", "{estimated_swap_sol():.6} SOL" }
                        }
                        div {
                            style: "
                                display: flex;
                                justify-content: space-between;
                            ",
                            span { style: "color: #94a3b8; font-size: 14px;", "Estimated total SOL sent:" }
                            span { style: "color: #f8fafc; font-size: 14px; font-weight: 600;", "{estimated_total_sol():.6} SOL" }
                        }
                        if tokens_without_price() > 0 {
                            div {
                                style: "
                                    margin-top: 10px;
                                    color: #fbbf24;
                                    font-size: 12px;
                                ",
                                "{tokens_without_price()} tokens lack price data; estimate may be low."
                            }
                        }
                    }

                    div {
                        style: "
                            background: #1a1a1a;
                            border: 1.5px solid #4a4a4a;
                            border-radius: 12px;
                            padding: 16px;
                            margin-bottom: 20px;
                        ",
                        div {
                            style: "color: #f8fafc; font-size: 15px; font-weight: 700; margin-bottom: 12px;",
                            "Tokens"
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
                                            style: "color: #f8fafc; font-size: 14px; font-weight: 600;",
                                            "{token.symbol}"
                                        }
                                        div {
                                            style: "color: #94a3b8; font-size: 12px;",
                                            "Balance: {token.balance:.6}"
                                        }
                                    }
                                }
                            }
                        }
                    }

                    div {
                        style: "
                            background: #1a1a1a;
                            border: 1.5px solid #4a4a4a;
                            border-radius: 12px;
                            padding: 16px;
                        ",
                        div {
                            style: "margin-bottom: 12px;",
                            label {
                                style: "color: #94a3b8; font-size: 14px; display: block; margin-bottom: 8px;",
                                "Recipient Address (required)"
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
                        div {
                            style: "margin-top: 12px;",
                            label {
                                style: "color: #94a3b8; font-size: 14px; display: block; margin-bottom: 8px;",
                                "Type RETIRE or \"{confirm_phrase()}\" to confirm"
                            }
                            input {
                                r#type: "text",
                                value: "{confirm_text()}",
                                oninput: move |e| confirm_text.set(e.value()),
                                placeholder: "RETIRE",
                                style: "
                                    width: 100%;
                                    background: #2a2a2a;
                                    border: 1px solid #4a4a4a;
                                    border-radius: 8px;
                                    padding: 10px 12px;
                                    color: #e2e8f0;
                                    font-size: 14px;
                                "
                            }
                        }
                        div {
                            style: "display: flex; align-items: center; gap: 10px; margin-top: 8px;",
                            input {
                                r#type: "checkbox",
                                checked: remove_from_storage(),
                                onchange: move |e| remove_from_storage.set(e.checked()),
                                style: "width: 18px; height: 18px; cursor: pointer;",
                            }
                            label {
                                style: "color: #e2e8f0; font-size: 13px; cursor: pointer;",
                                onclick: move |_| remove_from_storage.set(!remove_from_storage()),
                                "Remove wallet from this device after RETIRE"
                            }
                        }
                    }
                }

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
                            text-transform: uppercase;
                            letter-spacing: 0.5px;
                            box-shadow: 0 4px 12px rgba(255, 255, 255, 0.2);
                        ",
                        disabled: processing() || resolved_recipient().is_none() || hardware_blocked || !confirm_ok(),
                        onclick: move |_| {
                            processing.set(true);
                            error_message.set(None);

                            let mut initial_items: Vec<RetireTokenItem> = selected_tokens()
                                .iter()
                                .map(|token| RetireTokenItem {
                                    token: token.clone(),
                                    status: RetireTokenStatus::Pending,
                                })
                                .collect();

                            if initial_items.is_empty() {
                                let placeholder = Token {
                                    mint: "RETIRE_SOL".to_string(),
                                    symbol: "SOL".to_string(),
                                    name: "SOL Sweep".to_string(),
                                    balance: 0.0,
                                    decimals: 9,
                                    value_usd: 0.0,
                                    price: 0.0,
                                    price_change: 0.0,
                                    price_change_1d: 0.0,
                                    price_change_3d: 0.0,
                                    price_change_7d: 0.0,
                                    icon_type: "data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' width='32' height='32' viewBox='0 0 32 32'><rect width='32' height='32' rx='16' fill='%2394a3b8'/><text x='16' y='22' text-anchor='middle' fill='white' font-family='Arial' font-size='14'>SOL</text></svg>".to_string(),
                                };
                                initial_items.push(RetireTokenItem {
                                    token: placeholder,
                                    status: RetireTokenStatus::Pending,
                                });
                            }

                            retire_items.set(initial_items);
                            show_processing_modal.set(true);
                            current_step_text.set("Starting RETIRE process...".to_string());

                            let wallet_clone = wallet.clone();
                            let rpc_clone = custom_rpc.clone();
                            let recipient_clone = resolved_recipient();
                            let tokens_to_retire = selected_tokens();

                            spawn(async move {
                                let status_callback = move |index: usize, status: RetireTokenStatus| {
                                    let mut items = retire_items();
                                    if index < items.len() {
                                        items[index].status = status.clone();
                                        retire_items.set(items);

                                        let step_text = match &status {
                                            RetireTokenStatus::FetchingQuote => format!("Fetching swap quote for token {}...", index + 1),
                                            RetireTokenStatus::SwappingToSol => format!("Swapping token {} to SOL...", index + 1),
                                            RetireTokenStatus::SwapSuccess { sol_received } => format!("Swap successful: {:.4} SOL", sol_received),
                                            RetireTokenStatus::ClosingAccount => format!("Closing token account {}...", index + 1),
                                            RetireTokenStatus::Complete { .. } => format!("Token {} complete", index + 1),
                                            RetireTokenStatus::Failed { reason } => format!("Token {} failed: {}", index + 1, reason),
                                            _ => format!("Processing token {}...", index + 1),
                                        };
                                        current_step_text.set(step_text);
                                    }
                                };

                                let recipient_pubkey = match recipient_clone {
                                    Some(pubkey) => pubkey,
                                    None => {
                                        error_message.set(Some("Recipient address required".to_string()));
                                        show_processing_modal.set(false);
                                        processing.set(false);
                                        return;
                                    }
                                };

                                match execute_retire(
                                    tokens_to_retire,
                                    wallet_clone,
                                    rpc_clone,
                                    recipient_pubkey,
                                    status_callback,
                                ).await {
                                    Ok((signature, total_sent, final_balance)) => {
                                        transaction_signature.set(signature);
                                        total_sol_sent.set(total_sent);
                                        residual_balance.set(Some(final_balance));
                                        current_step_text.set("RETIRE completed successfully".to_string());
                                        processing_complete.set(true);
                                        processing.set(false);
                                    }
                                    Err(e) => {
                                        error_message.set(Some(format!("RETIRE failed: {}", e)));
                                        show_processing_modal.set(false);
                                        processing.set(false);
                                    }
                                }
                            });
                        },
                        if processing() { "RETIRING..." } else { "RETIRE" }
                    }
                }
            }
        }
    }
}
