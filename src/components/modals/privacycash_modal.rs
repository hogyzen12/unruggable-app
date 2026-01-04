use dioxus::prelude::*;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::cell::RefCell;
use std::rc::Rc;

use crate::privacycash;
use crate::signing::{SignerType, TransactionSigner};
use crate::transaction::TransactionClient;
use crate::wallet::{Wallet, WalletInfo};

const DEFAULT_RPC_URL: &str = "https://johna-k3cr1v-fast-mainnet.helius-rpc.com";
const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
const USDT_MINT: &str = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB";
const ORE_MINT: &str = "oreoU2P8bN6jkk3jbaiVxYnG1dCXcYxwhwyK9jSybcp";

struct PrivacyToken {
    symbol: &'static str,
    mint: Option<&'static str>,
    decimals: u8,
}

const PRIVACY_TOKENS: &[PrivacyToken] = &[
    PrivacyToken {
        symbol: "SOL",
        mint: None,
        decimals: 9,
    },
    PrivacyToken {
        symbol: "USDC",
        mint: Some(USDC_MINT),
        decimals: 6,
    },
    PrivacyToken {
        symbol: "USDT",
        mint: Some(USDT_MINT),
        decimals: 6,
    },
    PrivacyToken {
        symbol: "ORE",
        mint: Some(ORE_MINT),
        decimals: 11,
    },
];

#[component]
pub fn PrivacyCashModal(
    wallet: Option<WalletInfo>,
    custom_rpc: Option<String>,
    onclose: EventHandler<()>,
) -> Element {
    let mut amount = use_signal(|| "".to_string());
    let mut recipient = use_signal(|| "".to_string());
    let mut error = use_signal(|| None as Option<String>);
    let mut status = use_signal(|| None as Option<String>);
    let mut busy = use_signal(|| false);
    let mut private_balance = use_signal(|| None as Option<u64>);
    let mut balance_loading = use_signal(|| false);
    let mut selected_token = use_signal(|| 0usize);

    let wallet_info = wallet.clone();
    let rpc_url = custom_rpc.clone();

    let on_refresh_balance = move |_| {
        let wallet_info = wallet_info.clone();
        let rpc_url = rpc_url.clone().unwrap_or_else(|| DEFAULT_RPC_URL.to_string());
        let token_index = selected_token();
        let token = &PRIVACY_TOKENS[token_index];
        error.set(None);
        status.set(None);
        balance_loading.set(true);

        spawn(async move {
            let Some(wallet_info) = wallet_info else {
                error.set(Some("No wallet selected".to_string()));
                balance_loading.set(false);
                return;
            };

            let Ok(wallet) = Wallet::from_wallet_info(&wallet_info) else {
                error.set(Some("Failed to load wallet".to_string()));
                balance_loading.set(false);
                return;
            };

            let signer = SignerType::from_wallet(wallet);
            let Ok(authority) = signer.get_public_key().await else {
                error.set(Some("Failed to get public key".to_string()));
                balance_loading.set(false);
                return;
            };

            let Ok(signature) = privacycash::sign_auth_message(&signer).await else {
                error.set(Some("Failed to sign auth message".to_string()));
                balance_loading.set(false);
                return;
            };

            println!(
                "[PrivacyCash] Fetching private balance for {} ({})",
                authority, token.symbol
            );
            let balance_res = match token.mint {
                Some(mint) => {
                    privacycash::get_private_balance_spl(
                        &authority,
                        &signature,
                        mint,
                        Some(rpc_url.as_str()),
                    )
                    .await
                }
                None => privacycash::get_private_balance(&authority, &signature, Some(rpc_url.as_str())).await,
            };
            match balance_res {
                Ok(balance) => {
                    println!("[PrivacyCash] Private balance {}", balance);
                    private_balance.set(Some(balance));
                }
                Err(err) => {
                    println!("[PrivacyCash] Balance fetch failed: {}", err);
                    error.set(Some(format!("Balance fetch failed: {err}")));
                }
            }

            balance_loading.set(false);
        });
    };

    let wallet_for_deposit = wallet.clone();
    let rpc_for_deposit = custom_rpc.clone();
    let on_deposit = move |_| {
        let wallet_info = wallet_for_deposit.clone();
        let rpc_url = rpc_for_deposit.clone().unwrap_or_else(|| DEFAULT_RPC_URL.to_string());
        let amount_value = amount();
        let token_index = selected_token();
        let token = &PRIVACY_TOKENS[token_index];
        error.set(None);
        status.set(None);
        busy.set(true);

        spawn(async move {
            let Some(wallet_info) = wallet_info else {
                error.set(Some("No wallet selected".to_string()));
                busy.set(false);
                return;
            };

            let Ok(wallet) = Wallet::from_wallet_info(&wallet_info) else {
                error.set(Some("Failed to load wallet".to_string()));
                busy.set(false);
                return;
            };

            let amount_f64 = match amount_value.parse::<f64>() {
                Ok(value) if value > 0.0 => value,
                _ => {
                    error.set(Some("Invalid amount".to_string()));
                    busy.set(false);
                    return;
                }
            };

            let signer = SignerType::from_wallet(wallet);
            let Ok(authority) = signer.get_public_key().await else {
                error.set(Some("Failed to get public key".to_string()));
                busy.set(false);
                return;
            };

            let Ok(signature) = privacycash::sign_auth_message(&signer).await else {
                error.set(Some("Failed to sign auth message".to_string()));
                busy.set(false);
                return;
            };

            let scale = 10_f64.powi(token.decimals as i32);
            let base_units = (amount_f64 * scale).round() as u64;
            println!("[PrivacyCash] Building deposit tx for {} ({})", authority, token.symbol);
            let mut tx = match token.mint {
                Some(mint) => {
                    privacycash::build_deposit_spl_tx(
                        &authority,
                        &signature,
                        base_units,
                        mint,
                        Some(rpc_url.as_str()),
                    )
                    .await
                }
                None => {
                    privacycash::build_deposit_tx(
                        &authority,
                        &signature,
                        base_units,
                        Some(rpc_url.as_str()),
                    )
                    .await
                }
            };
            let mut tx = match tx {
                Ok(tx) => tx,
                Err(err) => {
                    error.set(Some(format!("Failed to build deposit tx: {err}")));
                    busy.set(false);
                    return;
                }
            };

            let tx_client = TransactionClient::new(Some(rpc_url.as_str()));
            let recent_blockhash = match tx_client.get_recent_blockhash().await {
                Ok(hash) => hash,
                Err(err) => {
                    error.set(Some(format!("Failed to get blockhash: {err}")));
                    busy.set(false);
                    return;
                }
            };

            if let Err(err) = privacycash::sign_transaction(&signer, &mut tx, recent_blockhash).await {
                error.set(Some(format!("Failed to sign tx: {err}")));
                busy.set(false);
                return;
            }

            println!("[PrivacyCash] Submitting deposit");
            match privacycash::submit_deposit(&authority, &tx).await {
                Ok(sig) => {
                    println!("[PrivacyCash] Deposit submitted {}", sig);
                    status.set(Some(format!("Deposit submitted: {sig}")));
                }
                Err(err) => {
                    println!("[PrivacyCash] Deposit failed: {}", err);
                    error.set(Some(format!("Deposit failed: {err}")));
                }
            }

            busy.set(false);
        });
    };

    let wallet_for_withdraw = wallet.clone();
    let rpc_for_withdraw = custom_rpc.clone();
    let selected_token_for_withdraw = selected_token.clone();
    let on_withdraw: Rc<RefCell<dyn FnMut(Option<String>)>> = Rc::new(RefCell::new(
        move |recipient_override: Option<String>| {
        let wallet_info = wallet_for_withdraw.clone();
        let amount_value = amount();
        let recipient_value = recipient();
        let rpc_url = rpc_for_withdraw.clone().unwrap_or_else(|| DEFAULT_RPC_URL.to_string());
        let token = &PRIVACY_TOKENS[selected_token_for_withdraw()];
            error.set(None);
            status.set(None);
            busy.set(true);

        spawn(async move {
            let Some(wallet_info) = wallet_info else {
                error.set(Some("No wallet selected".to_string()));
                busy.set(false);
                return;
            };

            let Ok(wallet) = Wallet::from_wallet_info(&wallet_info) else {
                error.set(Some("Failed to load wallet".to_string()));
                busy.set(false);
                return;
            };

            let amount_f64 = match amount_value.parse::<f64>() {
                Ok(value) if value > 0.0 => value,
                _ => {
                    error.set(Some("Invalid amount".to_string()));
                    busy.set(false);
                    return;
                }
            };

            let signer = SignerType::from_wallet(wallet);
            let Ok(authority) = signer.get_public_key().await else {
                error.set(Some("Failed to get public key".to_string()));
                busy.set(false);
                return;
            };

            let recipient = recipient_override.unwrap_or(recipient_value);
            let recipient = if recipient.trim().is_empty() {
                authority.clone()
            } else {
                recipient
            };

            if Pubkey::from_str(&recipient).is_err() {
                error.set(Some("Invalid recipient address".to_string()));
                busy.set(false);
                return;
            }

            let Ok(signature) = privacycash::sign_auth_message(&signer).await else {
                error.set(Some("Failed to sign auth message".to_string()));
                busy.set(false);
                return;
            };

            let scale = 10_f64.powi(token.decimals as i32);
            let base_units = (amount_f64 * scale).round() as u64;
            println!("[PrivacyCash] Building withdraw request {} ({})", recipient, token.symbol);
            let req = match token.mint {
                Some(mint) => {
                    privacycash::build_withdraw_spl_request(
                        &authority,
                        &signature,
                        base_units,
                        &recipient,
                        mint,
                        Some(rpc_url.as_str()),
                    )
                    .await
                }
                None => {
                    privacycash::build_withdraw_request(
                        &authority,
                        &signature,
                        base_units,
                        &recipient,
                        Some(rpc_url.as_str()),
                    )
                    .await
                }
            };
            let req = match req {
                Ok(req) => req,
                Err(err) => {
                    error.set(Some(format!("Failed to build withdraw request: {err}")));
                    busy.set(false);
                    return;
                }
            };

            println!("[PrivacyCash] Submitting withdraw request");
            match privacycash::submit_withdraw(&req).await {
                Ok(sig) => {
                    println!("[PrivacyCash] Withdraw submitted {}", sig);
                    status.set(Some(format!("Withdraw submitted: {sig}")));
                }
                Err(err) => {
                    println!("[PrivacyCash] Withdraw failed: {}", err);
                    error.set(Some(format!("Withdraw failed: {err}")));
                }
            }

            busy.set(false);
        });
        }
    ));

    let withdraw_to_self = {
        let on_withdraw = Rc::clone(&on_withdraw);
        move |_| {
            on_withdraw.borrow_mut()(None);
        }
    };
    let send_privately = {
        let on_withdraw = Rc::clone(&on_withdraw);
        move |_| {
            on_withdraw.borrow_mut()(Some(recipient()));
        }
    };

    let selected_index = selected_token();
    let selected = &PRIVACY_TOKENS[selected_index];
    let selected_symbol = selected.symbol;
    let selected_decimals = selected.decimals;

    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| onclose.call(()),

            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),

                h2 { class: "modal-title", "Privacy Cash" }

                div {
                    class: "wallet-field",
                    label { "Asset:" }
                    select {
                        value: "{selected_index}",
                        onchange: move |e| {
                            if let Ok(idx) = e.value().parse::<usize>() {
                                selected_token.set(idx);
                                private_balance.set(None);
                                error.set(None);
                                status.set(None);
                            }
                        },
                        for (idx, token) in PRIVACY_TOKENS.iter().enumerate() {
                            option { value: "{idx}", "{token.symbol}" }
                        }
                    }
                }

                div {
                    class: "wallet-field",
                    label { "Private Balance ({selected_symbol}):" }
                    div { class: "address-display",
                        if balance_loading() {
                            "Loading..."
                        } else if let Some(balance) = private_balance() {
                            {
                                let display = balance as f64 / 10_f64.powi(selected_decimals as i32);
                                rsx! { "{display:.6}" }
                            }
                        } else {
                            "-"
                        }
                    }
                    button {
                        class: "modal-button secondary",
                        onclick: on_refresh_balance,
                        disabled: balance_loading(),
                        "Refresh"
                    }
                }

                div {
                    class: "wallet-field",
                    label { "Amount ({selected_symbol}):" }
                    input {
                        r#type: "number",
                        value: "{amount}",
                        oninput: move |e| amount.set(e.value()),
                        placeholder: "0.0",
                        step: "0.0001",
                        min: "0"
                    }
                }

                div {
                    class: "wallet-field",
                    label { "Recipient (optional for withdraw):" }
                    input {
                        r#type: "text",
                        value: "{recipient}",
                        oninput: move |e| recipient.set(e.value()),
                        placeholder: "Solana address"
                    }
                }

                if let Some(err) = error() {
                    div { class: "error-message", "{err}" }
                }

                if let Some(msg) = status() {
                    div { class: "success-message", "{msg}" }
                }

                div { class: "modal-buttons",
                    button {
                        class: "modal-button primary",
                        onclick: on_deposit,
                        disabled: busy(),
                        "Deposit"
                    }
                    button {
                        class: "modal-button primary",
                        onclick: send_privately,
                        disabled: busy(),
                        "Send Privately"
                    }
                    button {
                        class: "modal-button secondary",
                        onclick: withdraw_to_self,
                        disabled: busy(),
                        "Withdraw to Self"
                    }
                    button {
                        class: "modal-button",
                        onclick: move |_| onclose.call(()),
                        "Close"
                    }
                }
            }
        }
    }
}
