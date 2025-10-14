use dioxus::prelude::*;
use crate::wallet::WalletInfo;
use crate::hardware::HardwareWallet;
use crate::carrot::{CarrotClient, CarrotBalances};
use crate::signing::{SignerType, TransactionSigner};
use carrot_sdk::{USDC_MINT, USDT_MINT, PYUSD_MINT};
use std::sync::Arc;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

/// Hardware wallet approval overlay for Carrot transactions
#[component]
fn HardwareApprovalOverlay(oncancel: EventHandler<()>) -> Element {
    rsx! {
        div {
            class: "hardware-approval-overlay",
            
            div {
                class: "hardware-approval-content",
                
                h3 { 
                    class: "hardware-approval-title",
                    "Confirm Transaction on Hardware Wallet"
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
                    "Please check your hardware wallet and confirm the transaction."
                }
                
                div {
                    class: "hardware-steps",
                    div {
                        class: "hardware-step",
                        div { class: "step-number", "1" }
                        span { "Review the transaction details on your Unruggable" }
                    }
                    div {
                        class: "hardware-step",
                        div { class: "step-number", "2" }
                        span { "Press the button to confirm" }
                    }
                }
                
                button {
                    class: "hardware-cancel-button",
                    onclick: move |_| oncancel.call(()),
                    "Cancel"
                }
            }
        }
    }
}

/// Success modal for completed transactions
#[component]
fn TransactionSuccessModal(
    signature: String,
    operation: String,
    amount: f64,
    asset_symbol: String,
    onclose: EventHandler<()>,
) -> Element {
    let solana_explorer_url = format!("https://explorer.solana.com/tx/{}", signature);
    let solscan_url = format!("https://solscan.io/tx/{}", signature);
    
    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| onclose.call(()),
            
            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                
                h2 { class: "modal-title", "Transaction Successful!" }
                
                div {
                    class: "tx-icon-container",
                    div {
                        class: "tx-success-icon",
                        "✅"
                    }
                }
                
                div {
                    class: "success-message",
                    "{operation} completed successfully"
                }
                
                div {
                    class: "stake-success-details",
                    div {
                        class: "stake-detail-card",
                        div {
                            class: "stake-detail-label",
                            "Amount:"
                        }
                        div {
                            class: "stake-detail-value",
                            "{amount:.6} {asset_symbol}"
                        }
                    }
                    
                    div {
                        class: "stake-detail-card",
                        div {
                            class: "stake-detail-label",
                            "Status:"
                        }
                        div {
                            class: "stake-detail-value",
                            "✅ Confirmed"
                        }
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
                            "{signature}"
                        }
                    }
                    
                    div {
                        class: "explorer-links",
                        p { "View in explorer:" }
                        
                        div {
                            class: "explorer-buttons",
                            a {
                                class: "explorer-button",
                                href: "{solana_explorer_url}",
                                target: "_blank",
                                "Solana Explorer"
                            }
                            a {
                                class: "explorer-button",
                                href: "{solscan_url}",
                                target: "_blank",
                                "Solscan"
                            }
                        }
                    }
                }
                
                div { 
                    class: "modal-buttons",
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
pub fn CarrotModal(
    wallet: Option<WalletInfo>,
    hardware_wallet: Option<Arc<HardwareWallet>>,
    custom_rpc: Option<String>,
    onclose: EventHandler<()>,
) -> Element {
    // State management
    let mut loading_balances = use_signal(|| false);
    let mut balances = use_signal(|| CarrotBalances::default());
    let mut selected_operation = use_signal(|| "deposit"); // "deposit" or "withdraw"
    let mut selected_asset = use_signal(|| "USDC");
    let mut amount_input = use_signal(|| String::new());
    let mut error_message = use_signal(|| None as Option<String>);
    let mut processing = use_signal(|| false);
    let mut show_hardware_approval = use_signal(|| false);
    let mut show_success_modal = use_signal(|| false);
    let mut success_signature = use_signal(|| String::new());
    let mut success_operation = use_signal(|| String::new());
    let mut success_amount = use_signal(|| 0.0f64);
    let mut success_asset = use_signal(|| String::new());

    // Get wallet address
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

    // Clone values before use_effect
    let wallet_for_effect = wallet.clone();
    let hardware_wallet_for_effect = hardware_wallet.clone();
    let custom_rpc_for_effect = custom_rpc.clone();

    // Load balances on mount
    use_effect(move || {
        if loading_balances() {
            println!("Already loading balances, skipping...");
            return;
        }
        
        if balances().usdc > 0.0 || balances().crt > 0.0 {
            println!("Already have balance data, skipping...");
            return;
        }
        
        println!("Starting balance fetch...");
        loading_balances.set(true);
        error_message.set(None);

        let wallet_clone = wallet_for_effect.clone();
        let hardware_wallet_clone = hardware_wallet_for_effect.clone();
        let custom_rpc_clone = custom_rpc_for_effect.clone();

        spawn(async move {
            // Get wallet address
            let wallet_address = if let Some(hw) = &hardware_wallet_clone {
                match hw.get_public_key().await {
                    Ok(addr) => addr,
                    Err(e) => {
                        error_message.set(Some(format!("Failed to get hardware wallet address: {}", e)));
                        loading_balances.set(false);
                        return;
                    }
                }
            } else if let Some(w) = &wallet_clone {
                w.address.clone()
            } else {
                error_message.set(Some("No wallet available".to_string()));
                loading_balances.set(false);
                return;
            };

            let wallet_pubkey = match Pubkey::from_str(&wallet_address) {
                Ok(pk) => pk,
                Err(e) => {
                    error_message.set(Some(format!("Invalid wallet address: {}", e)));
                    loading_balances.set(false);
                    return;
                }
            };

            // Create Carrot client
            let client = CarrotClient::new(custom_rpc_clone.as_deref());

            // Fetch balances
            match client.get_balances(&wallet_pubkey).await {
                Ok(fetched_balances) => {
                    println!("Fetched balances - USDC: {}, USDT: {}, pyUSD: {}, CRT: {}", 
                        fetched_balances.usdc, fetched_balances.usdt, 
                        fetched_balances.pyusd, fetched_balances.crt);
                    balances.set(fetched_balances);
                }
                Err(e) => {
                    error_message.set(Some(format!("Error loading balances: {}", e)));
                    println!("Error loading balances: {}", e);
                }
            }

            loading_balances.set(false);
        });
    });

    // Show success modal if transaction was successful
    if show_success_modal() {
        return rsx! {
            TransactionSuccessModal {
                signature: success_signature(),
                operation: success_operation(),
                amount: success_amount(),
                asset_symbol: success_asset(),
                onclose: move |_| {
                    show_success_modal.set(false);
                    onclose.call(());
                }
            }
        };
    }

    // Get the mint address for selected asset
    let get_asset_mint = move || -> Pubkey {
        match selected_asset().as_ref() {
            "USDC" => USDC_MINT,
            "USDT" => USDT_MINT,
            "pyUSD" => PYUSD_MINT,
            _ => USDC_MINT,
        }
    };

    // Get balance for selected asset
    let get_asset_balance = move || -> f64 {
        match selected_asset().as_ref() {
            "USDC" => balances().usdc,
            "USDT" => balances().usdt,
            "pyUSD" => balances().pyusd,
            _ => 0.0,
        }
    };

    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| {
                onclose.call(());
            },

            div {
                class: "modal-content stake-modal",
                onclick: move |e| e.stop_propagation(),
                style: "position: relative;",

                // Hardware approval overlay
                if show_hardware_approval() {
                    HardwareApprovalOverlay {
                        oncancel: move |_| {
                            show_hardware_approval.set(false);
                            processing.set(false);
                        }
                    }
                }

                // Header
                h2 { 
                    class: "modal-title",
                    "Carrot Protocol"
                }

                // Show error if any
                if let Some(error) = error_message() {
                    div {
                        class: "error-message",
                        "{error}"
                    }
                }

                div {
                    class: "modal-body",

                    // Loading state
                    if loading_balances() {
                        div {
                            class: "loading-stakes-modern",
                            div { class: "loading-spinner" }
                            "Loading balances..."
                        }
                    }
                    else {
                        // Mobile-optimized vertical layout
                        
                        // Operation selector (Deposit/Withdraw)
                        div {
                            style: "display: flex; gap: 8px; margin-bottom: 16px;",
                            button {
                                class: if selected_operation() == "deposit" { "modal-button primary" } else { "modal-button secondary" },
                                style: "flex: 1; padding: 8px; font-size: 13px;",
                                onclick: move |_| {
                                    selected_operation.set("deposit");
                                    amount_input.set(String::new());
                                    error_message.set(None);
                                },
                                "Deposit"
                            }
                            button {
                                class: if selected_operation() == "withdraw" { "modal-button primary" } else { "modal-button secondary" },
                                style: "flex: 1; padding: 8px; font-size: 13px;",
                                onclick: move |_| {
                                    selected_operation.set("withdraw");
                                    amount_input.set(String::new());
                                    error_message.set(None);
                                },
                                "Withdraw"
                            }
                        }

                        // Asset selector
                        div {
                            style: "margin-bottom: 10px;",
                            label { 
                                style: "display: block; margin-bottom: 6px; font-size: 12px; font-weight: 600;",
                                if selected_operation() == "deposit" {
                                    "Select Asset:"
                                } else {
                                    "Asset to Receive:"
                                }
                            }
                            div {
                                style: "display: flex; gap: 6px;",
                                button {
                                    class: if selected_asset() == "USDC" { "modal-button primary" } else { "modal-button secondary" },
                                    style: "flex: 1; padding: 6px; font-size: 11px;",
                                    onclick: move |_| {
                                        selected_asset.set("USDC");
                                        if selected_operation() == "deposit" {
                                            amount_input.set(String::new());
                                        }
                                    },
                                    "USDC"
                                }
                                button {
                                    class: if selected_asset() == "USDT" { "modal-button primary" } else { "modal-button secondary" },
                                    style: "flex: 1; padding: 6px; font-size: 11px;",
                                    onclick: move |_| {
                                        selected_asset.set("USDT");
                                        if selected_operation() == "deposit" {
                                            amount_input.set(String::new());
                                        }
                                    },
                                    "USDT"
                                }
                                button {
                                    class: if selected_asset() == "pyUSD" { "modal-button primary" } else { "modal-button secondary" },
                                    style: "flex: 1; padding: 6px; font-size: 11px;",
                                    onclick: move |_| {
                                        selected_asset.set("pyUSD");
                                        if selected_operation() == "deposit" {
                                            amount_input.set(String::new());
                                        }
                                    },
                                    "pyUSD"
                                }
                            }
                            div {
                                style: "margin-top: 4px; font-size: 10px; opacity: 0.7;",
                                if selected_operation() == "deposit" {
                                    "Available: {get_asset_balance():.2} {selected_asset()}"
                                } else {
                                    "Available: {balances().crt:.6} CRT"
                                }
                            }
                        }
                        
                        // Amount input with inline Max button
                        div {
                            style: "margin-bottom: 12px;",
                            label { 
                                style: "display: block; margin-bottom: 6px; font-size: 12px; font-weight: 600;",
                                if selected_operation() == "deposit" {
                                    "Amount ({selected_asset()}):"
                                } else {
                                    "Amount (CRT):"
                                }
                            }
                            div {
                                style: "display: flex; gap: 6px;",
                                input {
                                    class: "amount-input",
                                    r#type: "text",
                                    placeholder: "0.00",
                                    style: "flex: 1; font-size: 14px;",
                                    value: "{amount_input()}",
                                    oninput: move |evt| {
                                        amount_input.set(evt.value().clone());
                                        error_message.set(None);
                                    }
                                }
                                button {
                                    class: "modal-button secondary",
                                    style: "padding: 8px 16px; font-size: 11px;",
                                    onclick: move |_| {
                                        let max_amount = if selected_operation() == "deposit" {
                                            get_asset_balance()
                                        } else {
                                            balances().crt
                                        };
                                        amount_input.set(format!("{:.6}", max_amount));
                                    },
                                    "Max"
                                }
                            }
                        }

                        // Action button
                        div {
                            button {
                                class: "modal-button primary",
                                disabled: processing() || amount_input().is_empty(),
                                style: "width: 100%; padding: 12px; font-size: 14px;",
                                onclick: {
                                    let wallet_c = wallet.clone();
                                    let hw_c = hardware_wallet.clone();
                                    let rpc_c = custom_rpc.clone();
                                    
                                    move |_| {
                                        let operation = selected_operation();
                                        let asset = selected_asset();
                                        let amount_str = amount_input();
                                        
                                        // Validate amount
                                        let amount_f64 = match amount_str.parse::<f64>() {
                                            Ok(amt) if amt > 0.0 => amt,
                                            _ => {
                                                error_message.set(Some("Please enter a valid amount".to_string()));
                                                return;
                                            }
                                        };

                                        // Convert to lamports/smallest unit
                                        let amount_lamports = if operation == "deposit" {
                                            (amount_f64 * 1_000_000.0) as u64  // USDC/USDT/pyUSD have 6 decimals
                                        } else {
                                            (amount_f64 * 1_000_000_000.0) as u64  // CRT has 9 decimals
                                        };

                                        processing.set(true);
                                        error_message.set(None);
                                        
                                        let wallet_clone = wallet_c.clone();
                                        let hw_clone = hw_c.clone();
                                        let rpc_clone = rpc_c.clone();
                                        let asset_mint = get_asset_mint();
                                        
                                        spawn(async move {
                                            // Create signer
                                            let signer: Box<dyn TransactionSigner> = if let Some(hw) = hw_clone {
                                                show_hardware_approval.set(true);
                                                Box::new(crate::signing::hardware::HardwareSigner::from_wallet(hw))
                                            } else if let Some(w) = wallet_clone {
                                                match crate::wallet::Wallet::from_wallet_info(&w) {
                                                    Ok(wallet_obj) => {
                                                        Box::new(crate::signing::software::SoftwareSigner::new(wallet_obj))
                                                    }
                                                    Err(e) => {
                                                        error_message.set(Some(format!("Failed to load wallet: {}", e)));
                                                        processing.set(false);
                                                        return;
                                                    }
                                                }
                                            } else {
                                                error_message.set(Some("No wallet available".to_string()));
                                                processing.set(false);
                                                return;
                                            };
                                            
                                            // Create client
                                            let client = CarrotClient::new(rpc_clone.as_deref());
                                            
                                            // Execute operation
                                            let result = if operation == "deposit" {
                                                client.deposit_with_signer(&*signer, &asset_mint, amount_lamports).await
                                                    .map(|r| (r.signature, r.crt_received, "CRT".to_string()))
                                            } else {
                                                client.withdraw_with_signer(&*signer, &asset_mint, amount_lamports).await
                                                    .map(|r| (r.signature, r.asset_received, asset.to_string()))
                                            };
                                            
                                            match result {
                                                Ok((signature, received_amount, received_asset)) => {
                                                    show_hardware_approval.set(false);
                                                    success_signature.set(signature);
                                                    success_operation.set(
                                                        if operation == "deposit" { "Deposit" } else { "Withdraw" }.to_string()
                                                    );
                                                    success_amount.set(received_amount);
                                                    success_asset.set(received_asset);
                                                    show_success_modal.set(true);
                                                }
                                                Err(e) => {
                                                    show_hardware_approval.set(false);
                                                    error_message.set(Some(format!("Transaction failed: {}", e)));
                                                }
                                            }
                                            
                                            processing.set(false);
                                        });
                                    }
                                },
                                if processing() {
                                    if selected_operation() == "deposit" { "Depositing..." } else { "Withdrawing..." }
                                } else {
                                    if selected_operation() == "deposit" { "Deposit" } else { "Withdraw" }
                                }
                            }
                            
                            button {
                                class: "modal-button secondary",
                                style: "width: 100%; margin-top: 10px;",
                                onclick: move |_| onclose.call(()),
                                "Close"
                            }
                        }
                    }
                }
            }
        }
    }
}