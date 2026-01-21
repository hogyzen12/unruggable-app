// src/components/modals/bulk_send_modal.rs

use dioxus::prelude::*;
use crate::components::common::Token;
use crate::wallet::{Wallet, WalletInfo};
use crate::hardware::HardwareWallet;
use crate::components::modals::send_modal::HardwareWalletEvent;
use crate::transaction::TransactionClient;
use crate::signing::{SignerType, hardware::HardwareSigner};
use crate::storage::{get_address_book_label, get_send_count, increment_send_count};
use crate::components::address_input::AddressInput; // ← ADD THIS IMPORT
use solana_sdk::pubkey::Pubkey; // ← ADD THIS IMPORT
use std::sync::Arc;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct SelectedTokenForBulkSend {
    pub token: Token,
    pub amount: f64,
}

/// Hardware wallet approval overlay component for bulk send
#[component]
fn BulkSendHardwareApprovalOverlay(selected_count: usize, oncancel: EventHandler<()>) -> Element {
    rsx! {
        div {
            class: "hardware-approval-overlay",
            
            div {
                class: "hardware-approval-content",
                
                h3 { 
                    class: "hardware-approval-title",
                    "Confirm Bulk Send ({selected_count} tokens)"
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
                    "Please check your hardware wallet and confirm the bulk transaction details."
                }
                
                div {
                    class: "hardware-steps",
                    div {
                        class: "hardware-step",
                        div { class: "step-number", "1" }
                        span { "Review all {selected_count} token transactions on your device" }
                    }
                    div {
                        class: "hardware-step",
                        div { class: "step-number", "2" }
                        span { "Press the button on your Unruggable to confirm each transaction" }
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

/// Success modal for bulk send
#[component]
pub fn BulkSendSuccessModal(
    signature: String,
    token_count: usize,
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
                
                h2 { class: "modal-title", "Bulk Send Successful!" }
                
                div {
                    class: "tx-icon-container",
                    div {
                        class: "tx-success-icon",
                        "✓" // Checkmark icon
                    }
                }
                
                div {
                    class: "success-message",
                    "Your bulk transaction with {token_count} tokens was submitted to the Solana network."
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

#[component]
pub fn BulkSendModal(
    selected_token_mints: HashSet<String>,
    all_tokens: Vec<Token>,
    wallet: Option<WalletInfo>,
    hardware_wallet: Option<Arc<HardwareWallet>>,
    current_balance: f64, // SOL balance for fees
    custom_rpc: Option<String>,
    onclose: EventHandler<()>,
    onsuccess: EventHandler<String>,
) -> Element {
    // State management - following the pattern from send_modal.rs
    let mut recipient = use_signal(|| "".to_string());
    let mut resolved_recipient = use_signal(|| Option::<Pubkey>::None); // ← ADD THIS LINE
    let mut sending = use_signal(|| false);
    let mut error_message = use_signal(|| None as Option<String>);
    let mut recipient_balance = use_signal(|| None as Option<f64>);
    let mut checking_balance = use_signal(|| false);
    let mut recipient_label = use_signal(|| None as Option<String>);
    let mut recipient_send_count = use_signal(|| None as Option<u64>);
    let mut token_amounts = use_signal(|| std::collections::HashMap::<String, String>::new());
    let mut token_amount_errors = use_signal(|| std::collections::HashMap::<String, String>::new());
    
    // Success modal state
    let mut show_success_modal = use_signal(|| false);
    let mut transaction_signature = use_signal(|| "".to_string());
    let mut was_hardware_transaction = use_signal(|| false);
    
    // Hardware approval overlay state
    let mut show_hardware_approval = use_signal(|| false);
    
    // Get the global TransactionClient from context (pre-initialized with TPU)
    let transaction_client = use_context::<Arc<TransactionClient>>();
    
    // Filter tokens to only selected ones using use_memo for reactivity
    let selected_tokens = use_memo(move || {
        all_tokens.iter()
            .filter(|token| selected_token_mints.contains(&token.mint))
            .cloned()
            .collect::<Vec<Token>>()
    });

    // Calculate total estimated fees using use_memo
    let estimated_fee = use_memo(move || {
        let base_fee = 0.000005; // Base fee per transaction
        let token_count = selected_tokens().len() as f64;
        base_fee * token_count
    });

    // Validate all amounts using use_memo
    let all_amounts_valid = use_memo(move || {
        let amounts = token_amounts();
        let mut errors = std::collections::HashMap::<String, String>::new();
        let mut all_valid = true;
        
        for token in selected_tokens().iter() {
            if let Some(amount_str) = amounts.get(&token.mint) {
                if amount_str.trim().is_empty() {
                    // Empty is okay, just not valid for submission
                    continue;
                }
                
                match amount_str.parse::<f64>() {
                    Ok(amount) => {
                        if amount <= 0.0 {
                            errors.insert(token.mint.clone(), "Amount must be greater than 0".to_string());
                            all_valid = false;
                        } else if amount > token.balance {
                            errors.insert(token.mint.clone(), format!("Max available: {:.6} {}", token.balance, token.symbol));
                            all_valid = false;
                        }
                        // Amount is valid - remove any existing error
                    }
                    Err(_) => {
                        errors.insert(token.mint.clone(), "Invalid number format".to_string());
                        all_valid = false;
                    }
                }
            }
        }
        
        // Update the errors signal
        token_amount_errors.set(errors);
        
        // Only valid if all tokens have amounts and all are valid
        all_valid && selected_tokens().iter().all(|token| {
            amounts.get(&token.mint)
                .map(|s| !s.trim().is_empty() && s.parse::<f64>().is_ok())
                .unwrap_or(false)
        })
    });

    // Check if we have sufficient SOL for fees using use_memo
    let sufficient_sol_for_fees = use_memo(move || {
        current_balance >= estimated_fee()
    });

    // Update recipient balance checking effect to use resolved recipient
    let custom_rpc_for_effect = custom_rpc.clone();
    use_effect(move || {
        if let Some(resolved_pubkey) = *resolved_recipient.read() {
            let recipient_addr = resolved_pubkey.to_string();
            let rpc_url = custom_rpc_for_effect.clone();

            checking_balance.set(true);
            recipient_balance.set(None);

            spawn(async move {
                match crate::rpc::get_balance(&recipient_addr, rpc_url.as_deref()).await {
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

    // Return success modal if transaction completed
    if show_success_modal() {
        return rsx! {
            BulkSendSuccessModal {
                signature: transaction_signature(),
                token_count: selected_tokens().len(),
                was_hardware_wallet: was_hardware_transaction(),
                onclose: move |_| {
                    show_success_modal.set(false);
                    onsuccess.call(transaction_signature());
                }
            }
        };
    }

    // Determine which address to show based on wallet type
    let display_address = if let Some(hw) = &hardware_wallet {
        let mut hw_address = use_signal(|| None as Option<String>);

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
            class: "modal-backdrop",  // Changed from "modal-overlay"
            onclick: move |_| onclose.call(()),
            
            div {
                class: "modal-content bulk-send-modal",
                onclick: move |e| e.stop_propagation(),
                style: "position: relative;",
                
                // Hardware approval overlay
                if show_hardware_approval() {
                    BulkSendHardwareApprovalOverlay {
                        selected_count: selected_tokens().len(),
                        oncancel: move |_| {
                            show_hardware_approval.set(false);
                            sending.set(false);
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
                        "Bulk Send Tokens"
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
                        on_change: move |val| {
                            recipient.set(val);
                            // Reset balance check and error when address changes
                            recipient_balance.set(None);
                            error_message.set(None);
                        },
                        on_resolved: move |pubkey| resolved_recipient.set(pubkey),
                        label: "Send all selected tokens to:",
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
                            "Recipient balance: {balance:.4} SOL"
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
                
                // Selected tokens section
                div { 
                    class: "wallet-field",
                    label { "Selected Tokens ({selected_tokens().len()}):" }
                    
                    div { 
                        class: "selected-tokens-list",
                        for token in selected_tokens().iter() {
                            div {
                                key: "{token.mint}",
                                class: "bulk-token-item",
                                
                                div { 
                                    class: "bulk-token-info",
                                    div { 
                                        class: "bulk-token-icon",
                                        img {
                                            src: "{token.icon_type}",
                                            alt: "{token.symbol}",
                                            width: "32",
                                            height: "32",
                                        }
                                    }
                                    div { 
                                        class: "bulk-token-details",
                                        div { 
                                            class: "bulk-token-name",
                                            "{token.name} ({token.symbol})"
                                        }
                                        div { 
                                            class: "bulk-token-balance",
                                            "Available: {token.balance} {token.symbol}"
                                        }
                                    }
                                }
                                
                                div { 
                                    class: "bulk-token-amount-section",
                                    div {
                                        class: "bulk-token-amount-input",
                                        input {
                                            class: if token_amount_errors().contains_key(&token.mint) {
                                                "form-input amount-input error"
                                            } else {
                                                "form-input amount-input"
                                            },
                                            r#type: "number",
                                            step: "any",
                                            min: "0",
                                            max: "{token.balance}",
                                            placeholder: "Amount",
                                            value: token_amounts().get(&token.mint).cloned().unwrap_or_default(),
                                            oninput: {
                                                let mint = token.mint.clone();
                                                move |e| {
                                                    let mut amounts = token_amounts();
                                                    amounts.insert(mint.clone(), e.value());
                                                    token_amounts.set(amounts);
                                                    // Trigger validation by accessing all_amounts_valid
                                                    let _ = all_amounts_valid();
                                                }
                                            }
                                        }
                                        button {
                                            class: "max-button",
                                            onclick: {
                                                let mint = token.mint.clone();
                                                let balance = token.balance;
                                                move |_| {
                                                    let mut amounts = token_amounts();
                                                    amounts.insert(mint.clone(), balance.to_string());
                                                    token_amounts.set(amounts);
                                                    // Trigger validation
                                                    let _ = all_amounts_valid();
                                                }
                                            },
                                            "MAX"
                                        }
                                    }
                                    
                                    // Show individual token amount error
                                    if let Some(error) = token_amount_errors().get(&token.mint) {
                                        div {
                                            class: "token-amount-error",
                                            "{error}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if hardware_wallet.is_some() {
                    div {
                        class: "info-message",
                        "Your hardware wallet will prompt you to approve each token transaction"
                    }
                }
                
                div { 
                    class: "modal-buttons",
                    button {
                        class: "modal-button primary",
                        disabled: sending() || !all_amounts_valid() || resolved_recipient.read().is_none(), // ← UPDATED VALIDATION
                        onclick: move |_| {
                            // ← VALIDATE RESOLVED RECIPIENT FIRST
                            let recipient_pubkey = match resolved_recipient.read().as_ref() {
                                Some(pubkey) => *pubkey,
                                None => {
                                    error_message.set(Some("Please enter a valid recipient address or domain".to_string()));
                                    return;
                                }
                            };

                            if !sending() {
                                sending.set(true);
                                error_message.set(None);
                                
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
                                let rpc_url = custom_rpc.clone();
                                let mut recipient_send_count = recipient_send_count.clone();
                                let selected_for_send: Vec<SelectedTokenForBulkSend> = selected_tokens()
                                    .iter()
                                    .filter_map(|token| {
                                        token_amounts().get(&token.mint)
                                            .and_then(|amount_str| amount_str.parse::<f64>().ok())
                                            .map(|amount| SelectedTokenForBulkSend { token: token.clone(), amount })
                                    })
                                    .collect();
                                
                                // Clone the transaction client Arc before moving into async
                                let client = transaction_client.clone();
                                
                                spawn(async move {
                                    // ← NO NEED TO VALIDATE recipient_address anymore since it's already a valid pubkey!
                                
                                    println!("Sending bulk transaction with {} tokens to {}", selected_for_send.len(), recipient_address);
                                    for item in &selected_for_send {
                                        println!("  {} {} ({})", item.amount, item.token.symbol, item.token.mint);
                                    }
                                    
                                    // Use the global pre-initialized TransactionClient (already cloned above)
                                
                                    // Determine signer type based on available wallet
                                    let result = if let Some(ref hw) = hardware_wallet_clone {
                                        // Use hardware wallet signer
                                        let hw_signer = HardwareSigner::from_wallet(hw.clone());
                                        client.send_bulk_tokens_with_signer(&hw_signer, &recipient_address, selected_for_send).await
                                    } else if let Some(wallet_info) = wallet_info {
                                        // Use software wallet signer
                                        match Wallet::from_wallet_info(&wallet_info) {
                                            Ok(wallet) => {
                                                let signer = SignerType::from_wallet(wallet);
                                                client.send_bulk_tokens_with_signer(&signer, &recipient_address, selected_for_send).await
                                            }
                                            Err(e) => {
                                                error_message.set(Some(format!("Failed to load wallet: {}", e)));
                                                sending.set(false);
                                                show_hardware_approval.set(false);
                                                return;
                                            }
                                        }
                                    } else {
                                        error_message.set(Some("No wallet available".to_string()));
                                        sending.set(false);
                                        show_hardware_approval.set(false);
                                        return;
                                    };
                                
                                    // Handle the transaction result
                                    match result {
                                        Ok(signature) => {
                                            println!("Bulk transaction sent successfully: {}", signature);

                                            // Hide hardware approval overlay
                                            show_hardware_approval.set(false);

                                            // Set the transaction signature and show success modal
                                            transaction_signature.set(signature);
                                            let new_count = increment_send_count(&recipient_address);
                                            recipient_send_count.set(Some(new_count));
                                            sending.set(false);
                                            show_success_modal.set(true);
                                        }
                                        Err(e) => {
                                            let error_msg = if e.to_string().contains("too large") {
                                                format!("Transaction too large. Please reduce the number of tokens or send in smaller batches. Error: {}", e)
                                            } else if e.to_string().contains("Insufficient") {
                                                format!("Insufficient balance for transaction fees or token amounts. Error: {}", e)
                                            } else {
                                                format!("Transaction failed: {}", e)
                                            };
                                            
                                            error_message.set(Some(error_msg));
                                            sending.set(false);
                                            show_hardware_approval.set(false);
                                        }
                                    }
                                });
                            }
                        },
                        if sending() {
                            "Sending..."
                        } else {
                            "Send All Tokens"
                        }
                    }
                }
            }
        }
    }
}
