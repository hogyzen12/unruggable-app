// src/components/modals/send_token_modal.rs
use crate::components::address_input::AddressInput; // ← ADD THIS IMPORT
use crate::hardware::AuthMode;
use crate::hardware::HardwareWallet;
use crate::privacycash;
use crate::rpc;
use crate::signing::hardware::HardwareSigner;
use crate::signing::{SignerType, TransactionSigner};
use crate::storage::{get_address_book_label, get_send_count, increment_send_count};
use crate::transaction::TransactionClient;
use crate::wallet::{Wallet, WalletInfo};
use dioxus::prelude::*;
use solana_sdk::pubkey::Pubkey; // ← ADD THIS IMPORT
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

// Import HardwareWalletEvent from send_modal instead of defining it again
use crate::components::modals::send_modal::HardwareWalletEvent;

const DEFAULT_RPC_URL: &str = "https://johna-k3cr1v-fast-mainnet.helius-rpc.com";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnlockMode {
    Pin,
    Otp,
}

fn extract_hardware_error_code(message: &str) -> Option<String> {
    const MARKER: &str = "Hardware wallet error: ";
    let idx = message.find(MARKER)?;
    let code = &message[idx + MARKER.len()..];
    let code = code
        .split(|c| c == '\n' || c == '\r')
        .next()
        .unwrap_or(code)
        .trim();
    if code.is_empty() {
        None
    } else {
        Some(code.to_string())
    }
}

fn format_unlock_error_message(err: &str) -> String {
    if let Some(code) = extract_hardware_error_code(err) {
        return match code.as_str() {
            "AUTH_FAILED" | "OTP_BAD_CODE" => "Incorrect code. Please try again.".to_string(),
            "AUTH_LOCKED" => {
                "Too many failed attempts. Device auth is locked. Use physical factory wipe to recover."
                    .to_string()
            }
            "BAD_PIN_FORMAT" | "BAD_OTP_FORMAT" => {
                "Code must be exactly 6 digits.".to_string()
            }
            "BUTTON_TIMEOUT" => {
                "No button press detected. Submit the code again, then press the device button within 8 seconds."
                    .to_string()
            }
            "AUTH_MODE_MISMATCH" => {
                "Unlock method does not match the device mode. Open hardware connect and try again."
                    .to_string()
            }
            "TIME_NOT_SET" => {
                "Device time is not set yet. Try the unlock again.".to_string()
            }
            other => format!("Unlock failed: {other}"),
        };
    }

    format!("Unlock failed: {err}")
}

async fn resolve_unlock_mode(wallet: &HardwareWallet) -> Option<UnlockMode> {
    if let Ok(Some(info)) = wallet.refresh_esp32_info().await {
        return match info.auth_mode {
            AuthMode::Pin => Some(UnlockMode::Pin),
            AuthMode::Otp => Some(UnlockMode::Otp),
            _ => None,
        };
    }

    match wallet.get_cached_esp32_info().await {
        Some(info) => match info.auth_mode {
            AuthMode::Pin => Some(UnlockMode::Pin),
            AuthMode::Otp => Some(UnlockMode::Otp),
            _ => None,
        },
        None => None,
    }
}

/// Modal component to display transaction success details for tokens
#[component]
pub fn TokenTransactionSuccessModal(
    signature: String,
    token_symbol: String,
    was_hardware_wallet: bool,
    onclose: EventHandler<()>,
) -> Element {
    // Explorer links - Solscan and Orb
    let solscan_url = format!("https://solscan.io/tx/{}", signature);
    let orb_url = format!(
        "https://orb.helius.dev/tx/{}?cluster=mainnet-beta&tab=summary",
        signature
    );

    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| onclose.call(()),

            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),

                h2 { class: "modal-title", "{token_symbol} Transaction Sent Successfully!" }

                div {
                    class: "tx-icon-container",
                    div {
                        class: "tx-success-icon",
                        "✓" // Checkmark icon
                    }
                }

                div {
                    class: "success-message",
                    "Your {token_symbol} transaction was submitted to the Solana network."
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
                                href: "{solscan_url}",
                                target: "_blank",
                                rel: "noopener noreferrer",
                                "Solscan"
                            }
                            a {
                                class: "explorer-button",
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
                        class: "modal-button primary",
                        onclick: move |_| onclose.call(()),
                        "Close"
                    }
                }
            }
        }
    }
}

/// Hardware wallet approval overlay component shown during token transaction signing
#[component]
fn TokenHardwareApprovalOverlay(token_symbol: String, oncancel: EventHandler<()>) -> Element {
    rsx! {
        div {
            class: "hardware-approval-overlay",

            div {
                class: "hardware-approval-content",

                h3 {
                    class: "hardware-approval-title",
                    "Confirm {token_symbol} Transaction"
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
                    "Please check your hardware wallet and confirm the {token_symbol} transaction details."
                }

                div {
                    class: "hardware-steps",
                    div {
                        class: "hardware-step",
                        div { class: "step-number", "1" }
                        span { "Review the transaction details on your device" }
                    }
                    div {
                        class: "hardware-step",
                        div { class: "step-number", "2" }
                        span { "Press the hardware button once to confirm" }
                    }
                }

                button {
                    class: "hardware-cancel-button",
                    onclick: move |_| oncancel.call(()),
                    "Cancel Transaction"
                }
            }
        }
    }
}

#[component]
pub fn SendTokenModal(
    wallet: Option<WalletInfo>,
    hardware_wallet: Option<Arc<HardwareWallet>>,
    token_symbol: String,
    token_mint: String,
    token_balance: f64,
    token_decimals: Option<u8>, // Token decimals for proper amount calculation
    custom_rpc: Option<String>,
    #[props(default = true)] enable_privacy: bool,
    onclose: EventHandler<()>,
    onsuccess: EventHandler<String>,
    #[props(!optional)] onhardware: EventHandler<HardwareWalletEvent>,
) -> Element {
    // Always declare all hooks at the top of the component - never conditionally
    let mut recipient = use_signal(|| "".to_string());
    let mut resolved_recipient = use_signal(|| Option::<Pubkey>::None); // ← ADD THIS LINE
    let mut amount = use_signal(|| "".to_string());
    let mut sending = use_signal(|| false);
    let error_message = use_signal(|| None as Option<String>);
    let mut recipient_balance = use_signal(|| None as Option<f64>);
    let mut checking_balance = use_signal(|| false);
    let mut recipient_label = use_signal(|| None as Option<String>);
    let mut recipient_send_count = use_signal(|| None as Option<u64>);
    let privacy_supported =
        enable_privacy && matches!(token_symbol.as_str(), "USDC" | "USDT" | "ORE");
    let mut privacy_enabled = use_signal(|| false);
    let private_balance = use_signal(|| None as Option<u64>);
    let private_balance_loading = use_signal(|| false);
    let privacy_progress = use_signal(|| None as Option<String>);
    let private_balance_error = use_signal(|| None as Option<String>);

    // Add state for transaction success modal - always declared
    let mut show_success_modal = use_signal(|| false);
    let transaction_signature = use_signal(|| "".to_string());
    let was_hardware_transaction = use_signal(|| false);

    // Add state for hardware wallet approval overlay - always declared
    let mut show_hardware_approval = use_signal(|| false);
    let mut show_unlock_modal = use_signal(|| false);
    let unlock_mode = use_signal(|| None as Option<UnlockMode>);
    let mut unlock_code = use_signal(|| "".to_string());
    let mut unlock_error = use_signal(|| None as Option<String>);
    let mut unlock_in_progress = use_signal(|| false);
    let _ = &onhardware;

    // Use decimals or default to 6 for most SPL tokens
    let decimals = token_decimals.unwrap_or(6);

    // Update recipient balance checking effect to use resolved recipient
    let custom_rpc_for_effect = custom_rpc.clone();
    use_effect(move || {
        if let Some(resolved_pubkey) = *resolved_recipient.read() {
            let recipient_addr = resolved_pubkey.to_string();
            let rpc_url = custom_rpc_for_effect.clone();

            checking_balance.set(true);
            recipient_balance.set(None);

            spawn(async move {
                match rpc::get_balance(&recipient_addr, rpc_url.as_deref()).await {
                    Ok(balance) => {
                        recipient_balance.set(Some(balance));
                    }
                    Err(_) => {
                        recipient_balance.set(None);
                    }
                }
                checking_balance.set(false);
            });
        } else {
            recipient_balance.set(None);
            checking_balance.set(false);
        }
    });

    use_effect(move || {
        if let Some(resolved_pubkey) = *resolved_recipient.read() {
            let address = resolved_pubkey.to_string();
            recipient_label.set(get_address_book_label(&address));
            let count = get_send_count(&address);
            if count > 0 {
                recipient_send_count.set(Some(count));
            } else {
                recipient_send_count.set(None);
            }
        } else {
            recipient_label.set(None);
            recipient_send_count.set(None);
        }
    });

    let refresh_private_balance: Rc<RefCell<dyn FnMut()>> = {
        let wallet_info = wallet.clone();
        let rpc_url = custom_rpc.clone();
        let hw_for_refresh = hardware_wallet.clone();
        let mint = token_mint.clone();
        let mut private_balance = private_balance.clone();
        let mut private_balance_loading = private_balance_loading.clone();
        let mut private_balance_error = private_balance_error.clone();
        let privacy_supported = privacy_supported;
        Rc::new(RefCell::new(move || {
            if !privacy_supported {
                private_balance.set(None);
                return;
            }
            private_balance_loading.set(true);
            private_balance_error.set(None);
            let rpc_url = rpc_url
                .clone()
                .unwrap_or_else(|| DEFAULT_RPC_URL.to_string());
            let wallet_info = wallet_info.clone();
            let hw_for_refresh = hw_for_refresh.clone();
            let mint = mint.clone();
            let mut private_balance = private_balance.clone();
            let mut private_balance_loading = private_balance_loading.clone();
            let mut private_balance_error = private_balance_error.clone();
            spawn(async move {
                let signer = if let Some(hw) = hw_for_refresh {
                    SignerType::Hardware(HardwareSigner::from_wallet(hw))
                } else {
                    let Some(wallet_info) = wallet_info else {
                        private_balance_loading.set(false);
                        return;
                    };
                    let Ok(wallet) = Wallet::from_wallet_info(&wallet_info) else {
                        private_balance_loading.set(false);
                        return;
                    };
                    SignerType::from_wallet(wallet)
                };
                let Ok(authority) = signer.get_public_key().await else {
                    private_balance_loading.set(false);
                    return;
                };
                let Ok(signature) = privacycash::sign_auth_message(&signer).await else {
                    private_balance_loading.set(false);
                    return;
                };
                match privacycash::get_private_balance_spl(
                    &authority,
                    &signature,
                    &mint,
                    Some(rpc_url.as_str()),
                )
                .await
                {
                    Ok(balance) => {
                        private_balance.set(Some(balance));
                    }
                    Err(err) => {
                        private_balance.set(None);
                        private_balance_error.set(Some(err));
                    }
                }
                private_balance_loading.set(false);
            });
        }))
    };

    {
        let refresh_private_balance = Rc::clone(&refresh_private_balance);
        use_effect(move || {
            if privacy_supported
                && privacy_enabled()
                && private_balance().is_none()
                && private_balance_error().is_none()
                && !private_balance_loading()
            {
                refresh_private_balance.borrow_mut()();
            }
        });
    }

    let execute_send: Rc<RefCell<dyn FnMut()>> = Rc::new(RefCell::new({
        let hardware_wallet = hardware_wallet.clone();
        let wallet = wallet.clone();
        let custom_rpc = custom_rpc.clone();
        let token_mint = token_mint.clone();
        let token_symbol = token_symbol.clone();
        let recipient_send_count = recipient_send_count.clone();
        let privacy_enabled = privacy_enabled.clone();
        let mut private_balance = private_balance.clone();
        let mut privacy_progress = privacy_progress.clone();
        let mut show_hardware_approval = show_hardware_approval.clone();
        let mut was_hardware_transaction = was_hardware_transaction.clone();
        let mut error_message = error_message.clone();
        let mut sending = sending.clone();
        let resolved_recipient = resolved_recipient.clone();
        let amount = amount.clone();
        let mut transaction_signature = transaction_signature.clone();
        let mut show_success_modal = show_success_modal.clone();
        let mut show_unlock_modal = show_unlock_modal.clone();
        let mut unlock_mode = unlock_mode.clone();
        let mut unlock_code = unlock_code.clone();
        let mut unlock_error = unlock_error.clone();
        let mut unlock_in_progress = unlock_in_progress.clone();
        let privacy_supported = privacy_supported;
        let decimals = decimals;
        let token_balance = token_balance;
        move || {
            let recipient_pubkey = match resolved_recipient.read().as_ref() {
                Some(pubkey) => *pubkey,
                None => {
                    error_message.set(Some(
                        "Please enter a valid recipient address or domain".to_string(),
                    ));
                    return;
                }
            };

            error_message.set(None);
            sending.set(true);

            if hardware_wallet.is_some() {
                show_hardware_approval.set(true);
                was_hardware_transaction.set(true);
            } else {
                was_hardware_transaction.set(false);
            }

            let hardware_wallet_clone = hardware_wallet.clone();
            let wallet_info = wallet.clone();
            let recipient_address = recipient_pubkey.to_string();
            let amount_str = amount();
            let rpc_url = custom_rpc.clone();
            let token_mint_clone = token_mint.clone();
            let token_symbol_clone = token_symbol.clone();
            let mut recipient_send_count = recipient_send_count.clone();

            spawn(async move {
                let amount_value = match amount_str.parse::<f64>() {
                    Ok(amt) if amt > 0.0 => amt,
                    _ => {
                        error_message.set(Some("Invalid amount".to_string()));
                        sending.set(false);
                        show_hardware_approval.set(false);
                        return;
                    }
                };

                let client = TransactionClient::new(rpc_url.as_deref());

                if privacy_enabled() && privacy_supported {
                    let signer = if let Some(hw) = hardware_wallet_clone.clone() {
                        SignerType::Hardware(HardwareSigner::from_wallet(hw))
                    } else {
                        let Some(ref wallet_info) = wallet_info else {
                            error_message.set(Some("No wallet available".to_string()));
                            sending.set(false);
                            return;
                        };

                        let Ok(wallet) = Wallet::from_wallet_info(wallet_info) else {
                            error_message.set(Some("Failed to load wallet".to_string()));
                            sending.set(false);
                            return;
                        };

                        SignerType::from_wallet(wallet)
                    };

                    let should_clear_hw = signer.is_hardware();
                    let Ok(authority) = signer.get_public_key().await else {
                        error_message.set(Some("Failed to get public key".to_string()));
                        sending.set(false);
                        if should_clear_hw {
                            show_hardware_approval.set(false);
                        }
                        return;
                    };

                    let signature = match privacycash::sign_auth_message(&signer).await {
                        Ok(signature) => signature,
                        Err(err) => {
                            let err_text = err.to_string();
                            if should_clear_hw {
                                if let Some(code) = extract_hardware_error_code(&err_text) {
                                    if code == "LOCKED" {
                                        if let Some(hw) = hardware_wallet_clone.clone() {
                                            if let Some(mode) =
                                                resolve_unlock_mode(hw.as_ref()).await
                                            {
                                                unlock_mode.set(Some(mode));
                                                unlock_code.set(String::new());
                                                unlock_error.set(None);
                                                unlock_in_progress.set(false);
                                                show_unlock_modal.set(true);
                                                sending.set(false);
                                                show_hardware_approval.set(false);
                                                return;
                                            }
                                        }
                                        error_message.set(Some(
                                            "Device is locked. Unlock it and try again."
                                                .to_string(),
                                        ));
                                        sending.set(false);
                                        show_hardware_approval.set(false);
                                        return;
                                    } else if code == "MODE_UNSET" {
                                        error_message.set(Some(
                                            "Device setup is required. Complete hardware setup before signing."
                                                .to_string(),
                                        ));
                                        sending.set(false);
                                        show_hardware_approval.set(false);
                                        return;
                                    } else if code == "AUTH_LOCKED" {
                                        error_message.set(Some(
                                            "Device auth is locked. Use physical factory wipe to recover."
                                                .to_string(),
                                        ));
                                        sending.set(false);
                                        show_hardware_approval.set(false);
                                        return;
                                    }
                                }
                            }

                            error_message.set(Some("Failed to sign auth message".to_string()));
                            sending.set(false);
                            if should_clear_hw {
                                show_hardware_approval.set(false);
                            }
                            return;
                        }
                    };

                    let rpc_url = rpc_url.unwrap_or_else(|| DEFAULT_RPC_URL.to_string());
                    let scale = 10_f64.powi(decimals as i32);
                    let base_units = (amount_value * scale).round() as u64;
                    privacy_progress.set(Some("Checking private balance…".to_string()));
                    let private_balance_value = match privacycash::get_private_balance_spl(
                        &authority,
                        &signature,
                        &token_mint_clone,
                        Some(rpc_url.as_str()),
                    )
                    .await
                    {
                        Ok(balance) => {
                            private_balance.set(Some(balance));
                            balance
                        }
                        Err(err) => {
                            private_balance.set(None);
                            error_message
                                .set(Some(format!("Failed to fetch private balance: {err}")));
                            sending.set(false);
                            if should_clear_hw {
                                show_hardware_approval.set(false);
                            }
                            return;
                        }
                    };
                    privacy_progress.set(Some("Preparing private send…".to_string()));

                    if private_balance_value < base_units {
                        let topup = base_units - private_balance_value;
                        let topup_amount = topup as f64 / scale;
                        if topup_amount > token_balance {
                            error_message.set(Some(format!(
                                "Insufficient public {} to top up private balance (need {:.4} {})",
                                token_symbol_clone, topup_amount, token_symbol_clone
                            )));
                            sending.set(false);
                            return;
                        }
                        privacy_progress
                            .set(Some("Step 1/2: Depositing to private balance…".to_string()));
                        let mut tx = match privacycash::build_deposit_spl_tx(
                            &authority,
                            &signature,
                            topup,
                            &token_mint_clone,
                            Some(rpc_url.as_str()),
                        )
                        .await
                        {
                            Ok(tx) => tx,
                            Err(err) => {
                                error_message
                                    .set(Some(format!("Failed to build deposit tx: {err}")));
                                sending.set(false);
                                if should_clear_hw {
                                    show_hardware_approval.set(false);
                                }
                                return;
                            }
                        };

                        let tx_client = TransactionClient::new(Some(rpc_url.as_str()));
                        let recent_blockhash = match tx_client.get_recent_blockhash().await {
                            Ok(hash) => hash,
                            Err(err) => {
                                error_message.set(Some(format!("Failed to get blockhash: {err}")));
                                sending.set(false);
                                if should_clear_hw {
                                    show_hardware_approval.set(false);
                                }
                                return;
                            }
                        };

                        if let Err(err) =
                            privacycash::sign_transaction(&signer, &mut tx, recent_blockhash).await
                        {
                            error_message.set(Some(format!("Failed to sign deposit tx: {err}")));
                            sending.set(false);
                            if should_clear_hw {
                                show_hardware_approval.set(false);
                            }
                            return;
                        }

                        if let Err(err) = privacycash::submit_deposit(&authority, &tx).await {
                            error_message.set(Some(format!("Deposit failed: {err}")));
                            sending.set(false);
                            if should_clear_hw {
                                show_hardware_approval.set(false);
                            }
                            return;
                        }

                        sleep(Duration::from_secs(4)).await;
                        if let Ok(balance) = privacycash::get_private_balance_spl(
                            &authority,
                            &signature,
                            &token_mint_clone,
                            Some(rpc_url.as_str()),
                        )
                        .await
                        {
                            private_balance.set(Some(balance));
                        }
                        privacy_progress.set(Some(format!(
                            "Step 1/2 complete: Deposited {:.4} {}",
                            topup_amount, token_symbol_clone
                        )));
                    }

                    privacy_progress.set(Some("Step 2/2: Sending privately…".to_string()));
                    let req = match privacycash::build_withdraw_spl_request(
                        &authority,
                        &signature,
                        base_units,
                        &recipient_address,
                        &token_mint_clone,
                        Some(rpc_url.as_str()),
                    )
                    .await
                    {
                        Ok(req) => req,
                        Err(err) => {
                            error_message
                                .set(Some(format!("Failed to build withdraw request: {err}")));
                            sending.set(false);
                            if should_clear_hw {
                                show_hardware_approval.set(false);
                            }
                            return;
                        }
                    };

                    match privacycash::submit_withdraw(&req).await {
                        Ok(signature) => {
                            privacy_progress.set(None);
                            transaction_signature.set(signature);
                            let new_count = increment_send_count(&recipient_address);
                            recipient_send_count.set(Some(new_count));
                            sending.set(false);
                            if should_clear_hw {
                                show_hardware_approval.set(false);
                            }
                            show_success_modal.set(true);
                        }
                        Err(err) => {
                            privacy_progress.set(None);
                            error_message.set(Some(format!("Withdraw failed: {err}")));
                            sending.set(false);
                            if should_clear_hw {
                                show_hardware_approval.set(false);
                            }
                        }
                    }
                } else {
                    if amount_value > token_balance {
                        error_message
                            .set(Some(format!("Insufficient {} balance", token_symbol_clone)));
                        sending.set(false);
                        show_hardware_approval.set(false);
                        return;
                    }
                }

                if let Some(hw) = hardware_wallet_clone {
                    let hw_signer = HardwareSigner::from_wallet(hw.clone());
                    match client
                        .send_spl_token_with_signer(
                            &hw_signer,
                            &recipient_address,
                            amount_value,
                            &token_mint_clone,
                        )
                        .await
                    {
                        Ok(signature) => {
                            println!("Token transaction sent with hardware wallet: {}", signature);
                            show_hardware_approval.set(false);
                            transaction_signature.set(signature);
                            let new_count = increment_send_count(&recipient_address);
                            recipient_send_count.set(Some(new_count));
                            sending.set(false);
                            show_success_modal.set(true);
                        }
                        Err(e) => {
                            let err_text = e.to_string();
                            if let Some(code) = extract_hardware_error_code(&err_text) {
                                if code == "LOCKED" {
                                    if let Some(mode) = resolve_unlock_mode(hw.as_ref()).await {
                                        unlock_mode.set(Some(mode));
                                        unlock_code.set(String::new());
                                        unlock_error.set(None);
                                        unlock_in_progress.set(false);
                                        show_unlock_modal.set(true);
                                        sending.set(false);
                                        show_hardware_approval.set(false);
                                        return;
                                    }
                                    error_message.set(Some(
                                        "Device is locked. Unlock it and try again.".to_string(),
                                    ));
                                    sending.set(false);
                                    show_hardware_approval.set(false);
                                    return;
                                } else if code == "MODE_UNSET" {
                                    error_message.set(Some(
                                        "Device setup is required. Complete hardware setup before signing."
                                            .to_string(),
                                    ));
                                    sending.set(false);
                                    show_hardware_approval.set(false);
                                    return;
                                } else if code == "AUTH_LOCKED" {
                                    error_message.set(Some(
                                        "Device auth is locked. Use physical factory wipe to recover."
                                            .to_string(),
                                    ));
                                    sending.set(false);
                                    show_hardware_approval.set(false);
                                    return;
                                }
                            }

                            error_message.set(Some(format!("Transaction failed: {}", e)));
                            sending.set(false);
                            show_hardware_approval.set(false);
                        }
                    }
                } else if let Some(wallet_info) = wallet_info {
                    match Wallet::from_wallet_info(&wallet_info) {
                        Ok(wallet) => {
                            match client
                                .send_spl_token(
                                    &wallet,
                                    &recipient_address,
                                    amount_value,
                                    &token_mint_clone,
                                )
                                .await
                            {
                                Ok(signature) => {
                                    println!("Token transaction sent: {}", signature);
                                    transaction_signature.set(signature);
                                    let new_count = increment_send_count(&recipient_address);
                                    recipient_send_count.set(Some(new_count));
                                    sending.set(false);
                                    show_success_modal.set(true);
                                }
                                Err(e) => {
                                    error_message.set(Some(format!("Transaction failed: {}", e)));
                                    sending.set(false);
                                }
                            }
                        }
                        Err(e) => {
                            error_message.set(Some(format!("Failed to load wallet: {}", e)));
                            sending.set(false);
                        }
                    }
                } else {
                    error_message.set(Some("No wallet available".to_string()));
                    sending.set(false);
                    show_hardware_approval.set(false);
                }
            });
        }
    }));
    let execute_send_for_unlock = Rc::clone(&execute_send);

    // Return success modal if transaction completed
    if show_success_modal() {
        return rsx! {
            TokenTransactionSuccessModal {
                signature: transaction_signature(),
                token_symbol: token_symbol.clone(),
                was_hardware_wallet: was_hardware_transaction(),
                onclose: move |_| {
                    show_success_modal.set(false);
                    // Call onsuccess when the user closes the modal
                    onsuccess.call(transaction_signature());
                }
            }
        };
    }

    // Determine which address to show based on wallet type
    let _display_address = if hardware_wallet.is_some() {
        let mut hw_address = use_signal(|| None as Option<String>);

        // Clone hardware_wallet for the effect
        let hw_clone = hardware_wallet.clone();
        use_effect(move || {
            if let Some(hw) = &hw_clone {
                let hw = hw.clone();
                spawn(async move {
                    if let Ok(pubkey) = hw.get_public_key().await {
                        hw_address.set(Some(pubkey));
                    }
                });
            }
        });
        hw_address().unwrap_or_else(|| "Hardware Wallet".to_string())
    } else if let Some(w) = &wallet {
        w.address.clone()
    } else {
        "No Wallet".to_string()
    };
    let execute_send_for_button = Rc::clone(&execute_send);

    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| onclose.call(()),

            div {
                class: "modal-content send-token-modal",
                onclick: move |e| e.stop_propagation(),
                style: "position: relative;", // Needed for absolute positioning of overlay

                // Hardware approval overlay - shown when waiting for hardware confirmation
                if show_hardware_approval() {
                    TokenHardwareApprovalOverlay {
                        token_symbol: token_symbol.clone(),
                        oncancel: move |_| {
                            show_hardware_approval.set(false);
                            sending.set(false);
                        }
                    }
                }

                if show_unlock_modal() {
                    div {
                        class: "modal-backdrop",
                        onclick: move |_| {},
                        div {
                            class: "modal-content",
                            onclick: move |e| e.stop_propagation(),
                            style: "
                                max-width: 420px;
                                margin: 0 auto;
                                text-align: left;
                            ",
                            h2 { class: "modal-title", "Unlock Hardware Device" }
                            p { class: "success-message",
                                match (unlock_mode(), unlock_in_progress()) {
                                    (Some(UnlockMode::Pin), true) => "Unlocking device...",
                                    (Some(UnlockMode::Pin), false) => "Enter your 6-digit Device PIN to continue signing.",
                                    (Some(UnlockMode::Otp), true) => "Code accepted. Press the hardware button once within 8 seconds to continue signing.",
                                    (Some(UnlockMode::Otp), false) => "Enter your 6-digit authenticator code. After you submit it, press the hardware button once to continue signing.",
                                    (None, _) => "Enter device unlock code to continue signing.",
                                }
                            }
                            div {
                                class: "wallet-field",
                                label {
                                    match unlock_mode() {
                                        Some(UnlockMode::Pin) => "Device PIN",
                                        Some(UnlockMode::Otp) => "Authenticator Code",
                                        None => "Unlock Code",
                                    }
                                }
                                input {
                                    r#type: "password",
                                    value: "{unlock_code}",
                                    oninput: move |e| {
                                        unlock_code.set(
                                            e.value()
                                                .chars()
                                                .filter(|c| c.is_ascii_digit())
                                                .take(6)
                                                .collect(),
                                        )
                                    },
                                    placeholder: "6 digits",
                                    maxlength: "6",
                                    autocomplete: "off",
                                    inputmode: "numeric",
                                    pattern: "[0-9]*",
                                    disabled: unlock_in_progress()
                                }
                            }
                            if let Some(err) = unlock_error() {
                                div { class: "error-message", "{err}" }
                            }
                            div { class: "modal-buttons",
                                button {
                                    class: "modal-button cancel",
                                    disabled: unlock_in_progress(),
                                    onclick: move |_| {
                                        show_unlock_modal.set(false);
                                        unlock_in_progress.set(false);
                                        unlock_error.set(None);
                                        unlock_code.set(String::new());
                                    },
                                    "Cancel"
                                }
                                button {
                                    class: "modal-button primary",
                                    disabled: unlock_in_progress(),
                                    onclick: {
                                        let hardware_wallet = hardware_wallet.clone();
                                        move |_| {
                                            if unlock_in_progress() {
                                                return;
                                            }

                                            let Some(hw) = hardware_wallet.clone() else {
                                                unlock_error.set(Some("Hardware wallet disconnected".to_string()));
                                                return;
                                            };

                                            let code = unlock_code();
                                            if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
                                                unlock_error.set(Some("Code must be exactly 6 digits.".to_string()));
                                                return;
                                            }

                                            let mode = unlock_mode();
                                            let execute_send_for_unlock = Rc::clone(&execute_send_for_unlock);
                                            spawn(async move {
                                                unlock_in_progress.set(true);
                                                unlock_error.set(None);

                                                let unlock_result = match mode {
                                                    Some(UnlockMode::Pin) => hw.unlock_pin(&code).await.map(|_| ()),
                                                    Some(UnlockMode::Otp) => hw.unlock_otp(&code).await.map(|_| ()),
                                                    None => Err("Unknown unlock mode".into()),
                                                };

                                                match unlock_result {
                                                    Ok(()) => {
                                                        unlock_in_progress.set(false);
                                                        unlock_error.set(None);
                                                        unlock_code.set(String::new());
                                                        show_unlock_modal.set(false);
                                                        execute_send_for_unlock.borrow_mut()();
                                                    }
                                                    Err(err) => {
                                                        unlock_in_progress.set(false);
                                                        unlock_error.set(Some(format_unlock_error_message(&err.to_string())));
                                                    }
                                                }
                                            });
                                        }
                                    },
                                    if unlock_in_progress() {
                                        match unlock_mode() {
                                            Some(UnlockMode::Otp) => "Press Device Button...",
                                            _ => "Unlocking...",
                                        }
                                    } else {
                                        "Unlock and Retry"
                                    }
                                }
                            }
                        }
                    }
                }

                div {
                    style: "
                        display: flex;
                        justify-content: space-between;
                        align-items: center;
                        padding: 24px;
                        border-bottom: none;
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
                        "Send {token_symbol}"
                    }
                    button {
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

                // Token info section
                div {
                    class: "token-info-section",
                    div {
                        class: "balance-display",
                        "Available Balance: {token_balance:.6} {token_symbol}"
                    }
                }

                // Show error if any
                if let Some(error) = error_message() {
                    div {
                        class: "error-message",
                        "{error}"
                    }
                }

                // ← REPLACE THE OLD RECIPIENT INPUT WITH THIS SNS-ENABLED VERSION:
                div {
                    class: "wallet-field",
                    AddressInput {
                        value: recipient.read().clone(),
                        on_change: move |val| recipient.set(val),
                        on_resolved: move |pubkey| resolved_recipient.set(pubkey),
                        label: "Send to:",
                        placeholder: "Enter address or domain (e.g., recipient.sol)",
                        show_address_book: Some(true)
                    }

                    // Keep the recipient balance display
                    if checking_balance() {
                        div {
                            class: "recipient-balance checking",
                            "Checking balance..."
                        }
                    } else if let Some(balance) = recipient_balance() {
                        div {
                            class: "recipient-balance",
                            "Recipient SOL balance: {balance:.4} SOL"
                        }
                    }
                    if let Some(label) = recipient_label() {
                        div {
                            class: "recipient-balance",
                            "Tag: {label}"
                        }
                    }
                    if let Some(count) = recipient_send_count() {
                        div {
                            class: "recipient-balance",
                            "Sent {count} times"
                        }
                    }
                }

                div {
                    class: "wallet-field",
                    label { "Amount ({token_symbol}):" }
                    input {
                        r#type: "number",
                        value: "{amount}",
                        oninput: move |e| amount.set(e.value()),
                        placeholder: "0.0",
                        step: "0.000001",
                        min: "0",
                        max: "{token_balance}"
                    }
                }

                if privacy_supported {
                    div {
                        class: "wallet-field privacy-field",
                        div {
                            class: "privacy-row",
                            div {
                                class: "privacy-label",
                                span { "Privacy" }
                                span { class: "privacy-subtitle", "Send privately (Privacy Cash)" }
                            }
                            label {
                                class: "privacy-toggle",
                                input {
                                    r#type: "checkbox",
                                    checked: privacy_enabled(),
                                    onchange: move |_| {
                                        let enabled = !privacy_enabled();
                                        privacy_enabled.set(enabled);
                                    }
                                }
                                span { class: "privacy-slider" }
                            }
                        }
                    }

                    if privacy_enabled() {
                        if private_balance_loading() {
                            div { class: "privacy-meta", "Fetching private balance..." }
                        } else if let Some(balance) = private_balance() {
                            {
                                let balance_display = balance as f64 / 10_f64.powi(decimals as i32);
                                rsx! {
                                    div {
                                        class: "privacy-meta",
                                        "Private balance: {balance_display:.4} {token_symbol}"
                                    }
                                }
                            }
                        } else if let Some(err) = private_balance_error() {
                            div {
                                class: "privacy-hint",
                                onclick: move |_| refresh_private_balance.borrow_mut()(),
                                "Private balance unavailable. Tap to retry. ({err})"
                            }
                        }
                        if let Some(progress) = privacy_progress() {
                            div { class: "privacy-hint", "{progress}" }
                        } else {
                            {
                                let amount_value = amount().parse::<f64>().ok();
                                let private_balance_value = private_balance().unwrap_or(0);
                                if let Some(amount_value) = amount_value {
                                    let scale = 10_f64.powi(decimals as i32);
                                    let base_units = (amount_value * scale).round() as u64;
                                    if private_balance().is_some() {
                                        if private_balance_value >= base_units {
                                            rsx! { div { class: "privacy-hint", "Balance already revealed; no additional hardware approval is needed to send." } }
                                        } else if hardware_wallet.is_some() {
                                            rsx! { div { class: "privacy-hint", "We will top up privately (2 txs). Your hardware wallet will prompt you to approve the deposit." } }
                                        } else {
                                            rsx! { div { class: "privacy-hint", "We will top up privately (2 txs). You'll sign a deposit before the private send." } }
                                        }
                                    } else {
                                        if hardware_wallet.is_some() {
                                            rsx! { div { class: "privacy-hint", "We'll reveal your private balance (one approval). If a top up is needed, you'll approve a deposit." } }
                                        } else {
                                            rsx! { div { class: "privacy-hint", "We will check your private balance; if a top up is needed, you'll be asked to approve a deposit." } }
                                        }
                                    }
                                } else {
                                    rsx! { div { class: "privacy-hint", "If needed, we will top up privately then send (2 txs)." } }
                                }
                            }
                        }
                    }
                }

                if hardware_wallet.is_some() {
                    div {
                        class: "info-message",
                        "Your hardware wallet will prompt you to approve the {token_symbol} transaction"
                    }
                }

                div { class: "modal-buttons",
                    button {
                        class: "modal-button primary",
                        onclick: move |_| execute_send_for_button.borrow_mut()(),
                        disabled: sending() || resolved_recipient.read().is_none() || amount().is_empty(), // ← UPDATED VALIDATION
                        if sending() && !show_hardware_approval() {
                            "Sending {token_symbol}..."
                        } else {
                            "Send {token_symbol}"
                        }
                    }
                }
            }
        }
    }
}
