// src/components/modals/send_token_modal.rs
use dioxus::prelude::*;
use crate::wallet::{Wallet, WalletInfo};
use crate::hardware::HardwareWallet;
use crate::transaction::TransactionClient;
use crate::signing::hardware::HardwareSigner;
use crate::rpc;
use std::sync::Arc;

// Import HardwareWalletEvent from send_modal instead of defining it again
use crate::components::modals::send_modal::HardwareWalletEvent;

/// Modal component to display transaction success details for tokens
#[component]
pub fn TokenTransactionSuccessModal(
    signature: String,
    token_symbol: String,
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
                
                // Add hardware wallet reconnection notice if this was a hardware wallet transaction
                if was_hardware_wallet {
                    div {
                        class: "hardware-reconnect-notice",
                        "Your hardware wallet has been disconnected after the transaction. You'll need to reconnect it for future transactions."
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
    let mut amount = use_signal(|| "".to_string());
    let mut sending = use_signal(|| false);
    let mut error_message = use_signal(|| None as Option<String>);
    let mut recipient_balance = use_signal(|| None as Option<f64>);
    let mut checking_balance = use_signal(|| false);
    
    // Add state for transaction success modal - always declared
    let mut show_success_modal = use_signal(|| false);
    let mut transaction_signature = use_signal(|| "".to_string());
    let mut was_hardware_transaction = use_signal(|| false);
    
    // Add state for hardware wallet approval overlay - always declared
    let mut show_hardware_approval = use_signal(|| false);

    // Use decimals or default to 6 for most SPL tokens
    let decimals = token_decimals.unwrap_or(6);

    // Use effect to check recipient balance when address changes
    let custom_rpc_for_effect = custom_rpc.clone();
    use_effect(move || {
        let recipient_addr = recipient();
        let rpc_url = custom_rpc_for_effect.clone();

        if recipient_addr.len() > 30 {
            if bs58::decode(&recipient_addr).into_vec().is_ok() {
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
            }
        } else {
            recipient_balance.set(None);
        }
    });

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

                h2 { 
                    class: "modal-title", 
                    "Send {token_symbol}"
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

                div {
                    class: "wallet-field",
                    label { "From Address:" }
                    div { class: "address-display", "{display_address}" }
                }

                div {
                    class: "wallet-field",
                    label { "Recipient Address:" }
                    input {
                        value: "{recipient}",
                        oninput: move |e| recipient.set(e.value()),
                        placeholder: "Enter Solana address"
                    }
                    // Show recipient balance if available
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

                if hardware_wallet.is_some() {
                    div {
                        class: "info-message",
                        "Your hardware wallet will prompt you to approve the {token_symbol} transaction"
                    }
                }

                div { class: "modal-buttons",
                    button {
                        class: "modal-button cancel",
                        onclick: move |_| onclose.call(()),
                        "Cancel"
                    }
                    button {
                        class: "modal-button primary",
                        onclick: move |_| {
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
                            let recipient_address = recipient();
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

                                if amount_value > token_balance {
                                    error_message.set(Some(format!("Insufficient {} balance", token_symbol_clone)));
                                    sending.set(false);
                                    show_hardware_approval.set(false);
                                    return;
                                }

                                // Validate recipient address
                                if let Err(e) = bs58::decode(&recipient_address).into_vec() {
                                    error_message.set(Some(format!("Invalid recipient address: {}", e)));
                                    sending.set(false);
                                    show_hardware_approval.set(false);
                                    return;
                                }

                                let client = TransactionClient::new(rpc_url.as_deref());

                                // Use hardware wallet if available, otherwise use software wallet
                                if let Some(hw) = hardware_wallet_clone {
                                    let hw_signer = HardwareSigner::from_wallet(hw.clone());
                                    match client.send_spl_token_with_signer(&hw_signer, &recipient_address, amount_value, &token_mint_clone).await {
                                        Ok(signature) => {
                                            println!("Token transaction sent with hardware wallet: {}", signature);
                                            
                                            // Hide hardware approval overlay
                                            show_hardware_approval.set(false);
                                            
                                            // Disconnect the hardware wallet
                                            hw.disconnect().await;
                                            
                                            // Notify the parent component about hardware wallet disconnection
                                            onhardware_handler.call(HardwareWalletEvent {
                                                connected: false,
                                                pubkey: None,
                                            });
                                            
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
                        disabled: sending() || recipient().is_empty() || amount().is_empty(),
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