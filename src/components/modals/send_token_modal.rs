// src/components/modals/send_token_modal.rs
use dioxus::prelude::*;
use crate::wallet::{Wallet, WalletInfo};
use crate::hardware::HardwareWallet;
use crate::transaction::TransactionClient;
use crate::signing::hardware::HardwareSigner;
use crate::signing::{SignerType, TransactionSigner};
use crate::privacycash;
use crate::rpc;
use crate::components::address_input::AddressInput; // ← ADD THIS IMPORT
use solana_sdk::pubkey::Pubkey; // ← ADD THIS IMPORT
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

// Import HardwareWalletEvent from send_modal instead of defining it again
use crate::components::modals::send_modal::HardwareWalletEvent;

const DEFAULT_RPC_URL: &str = "https://johna-k3cr1v-fast-mainnet.helius-rpc.com";

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
    let orb_url = format!("https://orb.helius.dev/tx/{}?cluster=mainnet-beta&tab=summary", signature);
    
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
                        span { "Press the button on your Unruggable to confirm" }
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
    onclose: EventHandler<()>,
    onsuccess: EventHandler<String>,
    #[props(!optional)] onhardware: EventHandler<HardwareWalletEvent>,
) -> Element {
    // Always declare all hooks at the top of the component - never conditionally
    let mut recipient = use_signal(|| "".to_string());
    let mut resolved_recipient = use_signal(|| Option::<Pubkey>::None); // ← ADD THIS LINE
    let mut amount = use_signal(|| "".to_string());
    let mut sending = use_signal(|| false);
    let mut error_message = use_signal(|| None as Option<String>);
    let mut recipient_balance = use_signal(|| None as Option<f64>);
    let mut checking_balance = use_signal(|| false);
    let privacy_supported = matches!(token_symbol.as_str(), "USDC" | "USDT" | "ORE");
    let mut privacy_enabled = use_signal(|| false);
    let mut private_balance = use_signal(|| None as Option<u64>);
    let mut private_balance_loading = use_signal(|| false);
    let mut privacy_progress = use_signal(|| None as Option<String>);
    let mut private_balance_error = use_signal(|| None as Option<String>);
    
    // Add state for transaction success modal - always declared
    let mut show_success_modal = use_signal(|| false);
    let mut transaction_signature = use_signal(|| "".to_string());
    let mut was_hardware_transaction = use_signal(|| false);
    
    // Add state for hardware wallet approval overlay - always declared
    let mut show_hardware_approval = use_signal(|| false);

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
            if !privacy_supported || hw_for_refresh.is_some() {
                private_balance.set(None);
                return;
            }
            private_balance_loading.set(true);
            private_balance_error.set(None);
            let rpc_url = rpc_url.clone().unwrap_or_else(|| DEFAULT_RPC_URL.to_string());
            let wallet_info = wallet_info.clone();
            let mint = mint.clone();
            let mut private_balance = private_balance.clone();
            let mut private_balance_loading = private_balance_loading.clone();
            let mut private_balance_error = private_balance_error.clone();
            spawn(async move {
                let Some(wallet_info) = wallet_info else {
                    private_balance_loading.set(false);
                    return;
                };
                let Ok(wallet) = Wallet::from_wallet_info(&wallet_info) else {
                    private_balance_loading.set(false);
                    return;
                };
                let signer = SignerType::from_wallet(wallet);
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
    let display_address = if let Some(hw) = &hardware_wallet {
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
                        placeholder: "Enter address or domain (e.g., recipient.sol)"
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
                        } else if hardware_wallet.is_some() {
                            div { class: "privacy-hint", "Private send is only supported for software wallets right now." }
                        } else {
                            div { class: "privacy-hint", "If needed, we will top-up privately then send (2 txs)." }
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
                        onclick: move |_| {
                            // ← VALIDATE RESOLVED RECIPIENT FIRST
                            let recipient_pubkey = match resolved_recipient.read().as_ref() {
                                Some(pubkey) => *pubkey,
                                None => {
                                    error_message.set(Some("Please enter a valid recipient address or domain".to_string()));
                                    return;
                                }
                            };

                            error_message.set(None);
                            sending.set(true);

                            // Show hardware approval overlay if using hardware wallet
                            if hardware_wallet.is_some() {
                                show_hardware_approval.set(true);
                                was_hardware_transaction.set(true);
                            } else {
                                was_hardware_transaction.set(false);
                            }

                            // Clone values for async task
                            let hardware_wallet_clone = hardware_wallet.clone();
                            let wallet_info = wallet.clone();
                            let recipient_address = recipient_pubkey.to_string(); // ← USE RESOLVED PUBKEY
                            let amount_str = amount();
                            let rpc_url = custom_rpc.clone();
                            let token_mint_clone = token_mint.clone();
                            let token_symbol_clone = token_symbol.clone();
                            
                            // Clone the onhardware event handler for use in async block
                            let onhardware_handler = onhardware.clone();

                            spawn(async move {
                                // Validate inputs
                                let amount_value = match amount_str.parse::<f64>() {
                                    Ok(amt) if amt > 0.0 => amt,
                                    _ => {
                                        error_message.set(Some("Invalid amount".to_string()));
                                        sending.set(false);
                                        show_hardware_approval.set(false);
                                        return;
                                    }
                                };

                                // ← NO NEED TO VALIDATE recipient_address anymore since it's already a valid pubkey!

                                let client = TransactionClient::new(rpc_url.as_deref());

                                // Use hardware wallet if available, otherwise use software wallet
                                if privacy_enabled() && privacy_supported {
                                    if hardware_wallet_clone.is_some() {
                                        error_message.set(Some("Private send is not supported with hardware wallets yet".to_string()));
                                        sending.set(false);
                                        show_hardware_approval.set(false);
                                        return;
                                    }

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

                                    let signer = SignerType::from_wallet(wallet);
                                    let Ok(authority) = signer.get_public_key().await else {
                                        error_message.set(Some("Failed to get public key".to_string()));
                                        sending.set(false);
                                        return;
                                    };

                                    let Ok(signature) = privacycash::sign_auth_message(&signer).await else {
                                        error_message.set(Some("Failed to sign auth message".to_string()));
                                        sending.set(false);
                                        return;
                                    };

                                    let rpc_url = rpc_url.unwrap_or_else(|| DEFAULT_RPC_URL.to_string());
                                    let scale = 10_f64.powi(decimals as i32);
                                    let base_units = (amount_value * scale).round() as u64;
                                    privacy_progress.set(Some("Checking private balance…".to_string()));
                                    let mut private_balance_value = match privacycash::get_private_balance_spl(
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
                                            error_message.set(Some(format!("Failed to fetch private balance: {err}")));
                                            sending.set(false);
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
                                        privacy_progress.set(Some("Step 1/2: Depositing to private balance…".to_string()));
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
                                                error_message.set(Some(format!("Failed to build deposit tx: {err}")));
                                                sending.set(false);
                                                return;
                                            }
                                        };

                                        let tx_client = TransactionClient::new(Some(rpc_url.as_str()));
                                        let recent_blockhash = match tx_client.get_recent_blockhash().await {
                                            Ok(hash) => hash,
                                            Err(err) => {
                                                error_message.set(Some(format!("Failed to get blockhash: {err}")));
                                                sending.set(false);
                                                return;
                                            }
                                        };

                                        if let Err(err) = privacycash::sign_transaction(&signer, &mut tx, recent_blockhash).await {
                                            error_message.set(Some(format!("Failed to sign deposit tx: {err}")));
                                            sending.set(false);
                                            return;
                                        }

                                        if let Err(err) = privacycash::submit_deposit(&authority, &tx).await {
                                            error_message.set(Some(format!("Deposit failed: {err}")));
                                            sending.set(false);
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
                                            private_balance_value = balance;
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
                                            error_message.set(Some(format!("Failed to build withdraw request: {err}")));
                                            sending.set(false);
                                            return;
                                        }
                                    };

                                    match privacycash::submit_withdraw(&req).await {
                                        Ok(signature) => {
                                            privacy_progress.set(None);
                                            transaction_signature.set(signature);
                                            sending.set(false);
                                            show_success_modal.set(true);
                                        }
                                        Err(err) => {
                                            privacy_progress.set(None);
                                            error_message.set(Some(format!("Withdraw failed: {err}")));
                                            sending.set(false);
                                        }
                                    }
                                } else {
                                    if amount_value > token_balance {
                                        error_message.set(Some(format!("Insufficient {} balance", token_symbol_clone)));
                                        sending.set(false);
                                        show_hardware_approval.set(false);
                                        return;
                                    }
                                }
                                if let Some(hw) = hardware_wallet_clone {
                                    let hw_signer = HardwareSigner::from_wallet(hw.clone());
                                    match client.send_spl_token_with_signer(&hw_signer, &recipient_address, amount_value, &token_mint_clone).await {
                                        Ok(signature) => {
                                            println!("Token transaction sent with hardware wallet: {}", signature);

                                            // Hide hardware approval overlay
                                            show_hardware_approval.set(false);

                                            // Set the transaction signature and show success modal
                                            transaction_signature.set(signature);
                                            sending.set(false);
                                            show_success_modal.set(true);
                                        }
                                        Err(e) => {
                                            error_message.set(Some(format!("Transaction failed: {}", e)));
                                            sending.set(false);
                                            show_hardware_approval.set(false);
                                        }
                                    }
                                } else if let Some(wallet_info) = wallet_info {
                                    // Load wallet from wallet info
                                    match Wallet::from_wallet_info(&wallet_info) {
                                        Ok(wallet) => {
                                            // Send SPL token transaction
                                            match client.send_spl_token(&wallet, &recipient_address, amount_value, &token_mint_clone).await {
                                                Ok(signature) => {
                                                    println!("Token transaction sent: {}", signature);
                                                    
                                                    // Set the transaction signature and show success modal
                                                    transaction_signature.set(signature);
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
                        },
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
