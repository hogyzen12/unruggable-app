use dioxus::prelude::*;
use crate::wallet::WalletInfo;
use crate::hardware::HardwareWallet;
use crate::bonk_staking::{BonkStakingClient, StakePosition};
use crate::signing::TransactionSigner;
use crate::common::Token;
use std::sync::Arc;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

#[component]
fn HardwareApprovalOverlay(oncancel: EventHandler<()>) -> Element {
    rsx! {
        div {
            class: "hardware-approval-overlay",
            div {
                class: "hardware-approval-content",
                h3 { class: "hardware-approval-title", "Confirm on Hardware Wallet" }
                div {
                    class: "hardware-icon-container",
                    div { class: "hardware-icon", div { class: "blink-indicator" } }
                    div { class: "button-indicator", div { class: "button-press" } }
                }
                p { class: "hardware-approval-text", "Review and confirm the BONK staking transaction on your hardware wallet." }
                button { class: "hardware-cancel-button", onclick: move |_| oncancel.call(()), "Cancel" }
            }
        }
    }
}

#[component]
fn TransactionSuccessModal(signature: String, operation: String, amount: f64, onclose: EventHandler<()>) -> Element {
    let explorer_url = format!("https://explorer.solana.com/tx/{}", signature);
    let solscan_url = format!("https://solscan.io/tx/{}", signature);
    
    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| onclose.call(()),
            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                h2 { class: "modal-title", "{operation} Successful!" }
                div { class: "tx-icon-container", div { class: "tx-success-icon", "✅" } }
                div { class: "success-message", "{operation} completed successfully" }
                div {
                    class: "stake-success-details",
                    div { class: "stake-detail-card", div { class: "stake-detail-label", "Amount:" } div { class: "stake-detail-value", "{amount:.2} BONK" } }
                    div { class: "stake-detail-card", div { class: "stake-detail-label", "Status:" } div { class: "stake-detail-value", "✅ Confirmed" } }
                }
                div {
                    class: "transaction-details",
                    div { class: "wallet-field", label { "Transaction Signature:" } div { class: "address-display", "{signature}" } }
                    div {
                        class: "explorer-links",
                        p { "View in explorer:" }
                        div {
                            class: "explorer-buttons",
                            a { class: "button-standard ghost", href: "{explorer_url}", target: "_blank", "Solana Explorer" }
                            a { class: "button-standard ghost", href: "{solscan_url}", target: "_blank", "Solscan" }
                        }
                    }
                }
                div { class: "modal-buttons", button { class: "button-standard primary", onclick: move |_| onclose.call(()), "Close" } }
            }
        }
    }
}

#[component]
pub fn BonkStakingModal(
    tokens: Vec<Token>,
    wallet: Option<WalletInfo>,
    hardware_wallet: Option<Arc<HardwareWallet>>,
    custom_rpc: Option<String>,
    onclose: EventHandler<()>,
    onsuccess: EventHandler<String>,
) -> Element {
    let wallet_address = use_signal(|| wallet.as_ref().map(|w| w.address.clone()));
    
    // State management
    let mut selected_mode = use_signal(|| "view".to_string()); // "view" or "stake"
    let mut amount = use_signal(|| "".to_string());
    let mut selected_duration = use_signal(|| 30u64);
    let mut processing = use_signal(|| false);
    let mut error_message = use_signal(|| None::<String>);
    
    // Balance and stakes
    let mut bonk_balance = use_signal(|| 0.0);
    let mut active_stakes = use_signal(|| Vec::<StakePosition>::new());
    let mut fetching_balance = use_signal(|| false);
    let mut fetching_stakes = use_signal(|| false);
    
    // Success modal
    let mut show_success_modal = use_signal(|| false);
    let mut transaction_signature = use_signal(|| "".to_string());
    let mut was_hardware_transaction = use_signal(|| false);
    let mut show_hardware_approval = use_signal(|| false);
    
    // Fetch BONK balance on mount
    let custom_rpc_for_effect = custom_rpc.clone();
    use_effect(move || {
        if let Some(address) = wallet_address() {
            if !fetching_balance() && bonk_balance() == 0.0 {
                fetching_balance.set(true);
                
                let rpc_url = custom_rpc_for_effect.clone();
                spawn(async move {
                    let client = BonkStakingClient::new(rpc_url.as_deref());
                    
                    // Convert address string to Pubkey
                    match Pubkey::from_str(&address) {
                        Ok(pubkey) => {
                            match client.get_bonk_balance(&pubkey).await {
                                Ok(balance) => bonk_balance.set(balance),
                                Err(e) => error_message.set(Some(format!("Failed to fetch BONK balance: {}", e))),
                            }
                        }
                        Err(e) => error_message.set(Some(format!("Invalid wallet address: {}", e))),
                    }
                    
                    fetching_balance.set(false);
                });
            }
        }
    });
    
    // Fetch active stakes
    let custom_rpc_for_stakes = custom_rpc.clone();
    use_effect(move || {
        if let Some(address) = wallet_address() {
            if active_stakes().is_empty() && !fetching_stakes() {
                fetching_stakes.set(true);
                
                let rpc_url = custom_rpc_for_stakes.clone();
                spawn(async move {
                    let client = BonkStakingClient::new(rpc_url.as_deref());
                    
                    match client.get_user_stakes(&address).await {
                        Ok(stakes) => active_stakes.set(stakes),
                        Err(e) => error_message.set(Some(format!("Failed to fetch stakes: {}", e))),
                    }
                    
                    fetching_stakes.set(false);
                });
            }
        }
    });
    
    let has_hardware = hardware_wallet.is_some();
    let duration_options = BonkStakingClient::get_duration_options();
    let duration_options_for_summary = duration_options.clone();
    
    // Calculate totals
    let total_locked = active_stakes().iter().map(|s| s.amount).sum::<f64>();
    let total_claimable = active_stakes().iter().filter(|s| s.is_unlocked).map(|s| s.amount).sum::<f64>();
    
    if show_success_modal() {
        return rsx! {
            TransactionSuccessModal {
                signature: transaction_signature(),
                operation: "BONK Stake".to_string(),
                amount: amount().parse::<f64>().unwrap_or(0.0),
                onclose: move |_| {
                    show_success_modal.set(false);
                    onsuccess.call(transaction_signature());
                }
            }
        };
    }
    
    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| onclose.call(()),
            
            div {
                class: "modal-content stake-modal",
                onclick: move |e| e.stop_propagation(),
                style: "position: relative;",
                
                if show_hardware_approval() {
                    HardwareApprovalOverlay {
                        oncancel: move |_| {
                            show_hardware_approval.set(false);
                            processing.set(false);
                        }
                    }
                }
                
                div {
                    class: "modal-header",
                    h2 { class: "modal-title", "BONK Staking" }
                    button {
                        class: "modal-close-button",
                        onclick: move |_| onclose.call(()),
                        "×"
                    }
                }
                
                if let Some(error) = error_message() {
                    div { class: "error-message", "{error}" }
                }
                
                div { class: "modal-body",
                    
                    // Mode selector
                    div {
                        style: "display: flex; gap: 8px; margin-bottom: 16px;",
                        button {
                            class: if selected_mode() == "view" { "button-standard primary" } else { "button-standard secondary" },
                            style: "flex: 1; padding: 10px; font-size: 14px;",
                            onclick: move |_| selected_mode.set("view".to_string()),
                            "My Positions"
                        }
                        button {
                            class: if selected_mode() == "stake" { "button-standard primary" } else { "button-standard secondary" },
                            style: "flex: 1; padding: 10px; font-size: 14px;",
                            onclick: move |_| selected_mode.set("stake".to_string()),
                            "Stake BONK"
                        }
                    }
                    
                    // View Mode - Positions
                    if selected_mode() == "view" {
                        if fetching_stakes() {
                            div { class: "loading-stakes-modern", div { class: "loading-spinner" } "Loading positions..." }
                        } else {
                            div {
                                // Summary cards
                                div {
                                    style: "display: grid; grid-template-columns: 1fr 1fr; gap: 12px; margin-bottom: 16px;",
                                    div {
                                        style: "background: rgba(255,255,255,0.05); border-radius: 12px; padding: 16px;",
                                        div { style: "font-size: 11px; opacity: 0.7; margin-bottom: 4px;", "Total Locked" }
                                        div { style: "font-size: 20px; font-weight: 600;", "{total_locked:.2} BONK" }
                                    }
                                    div {
                                        style: "background: rgba(255,255,255,0.05); border-radius: 12px; padding: 16px;",
                                        div { style: "font-size: 11px; opacity: 0.7; margin-bottom: 4px;", "Claimable" }
                                        div { style: "font-size: 20px; font-weight: 600; color: #22c55e;", "{total_claimable:.2} BONK" }
                                    }
                                }
                                
                                // Claim all button
                                if total_claimable > 0.0 {
                                    button {
                                        class: "button-standard primary",
                                        style: "width: 100%; margin-bottom: 20px;",
                                        disabled: processing(),
                                        onclick: move |_| {
                                            processing.set(true);
                                            error_message.set(Some("Claim all functionality coming soon".to_string()));
                                            processing.set(false);
                                        },
                                        if processing() { "Claiming..." } else { "Claim All Unlocked" }
                                    }
                                }
                                
                                // Active locks list
                                div { style: "margin-top: 12px;",
                                    h3 { style: "font-size: 14px; font-weight: 600; margin-bottom: 12px;", "Active Locks" }
                                    
                                    if active_stakes().is_empty() {
                                        div {
                                            style: "text-align: center; padding: 40px; opacity: 0.5;",
                                            "No active stakes found. Stake BONK to start earning!"
                                        }
                                    } else {
                                        for stake in active_stakes() {
                                            div {
                                                key: "{stake.unlock_time}",
                                                style: "background: rgba(255,255,255,0.03); border-radius: 12px; padding: 16px; margin-bottom: 12px; border: 1px solid rgba(255,255,255,0.1);",
                                                div {
                                                    style: "display: flex; justify-content: space-between; align-items: start; margin-bottom: 12px;",
                                                    div {
                                                        div { style: "font-size: 18px; font-weight: 600;", "{stake.amount:.2} BONK" }
                                                        div { style: "font-size: 11px; opacity: 0.6; margin-top: 2px;", "{stake.duration_days} days" }
                                                    }
                                                    div {
                                                        style: if stake.is_unlocked { "background: #22c55e; color: white; padding: 4px 12px; border-radius: 20px; font-size: 11px; font-weight: 600;" } else { "background: rgba(255,255,255,0.1); padding: 4px 12px; border-radius: 20px; font-size: 11px;" },
                                                        if stake.is_unlocked { "Unlocked" } else { "Locked" }
                                                    }
                                                }
                                                div {
                                                    style: "display: grid; grid-template-columns: 1fr 1fr; gap: 12px; font-size: 12px;",
                                                    div {
                                                        div { style: "opacity: 0.6; margin-bottom: 2px;", "Multiplier" }
                                                        div { style: "font-weight: 600;", "{stake.multiplier}x" }
                                                    }
                                                    div {
                                                        div { style: "opacity: 0.6; margin-bottom: 2px;", "Unlock Time" }
                                                        div { style: "font-weight: 600;", "{stake.unlock_time}" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    // Stake Mode
                    if selected_mode() == "stake" {
                        if fetching_balance() {
                            div { class: "loading-stakes-modern", div { class: "loading-spinner" } "Loading balance..." }
                        } else {
                            div {
                                // Available balance
                                div {
                                    style: "background: rgba(255,255,255,0.05); border-radius: 12px; padding: 16px; margin-bottom: 16px;",
                                    div { style: "font-size: 11px; opacity: 0.7; margin-bottom: 4px;", "Available Balance" }
                                    div { style: "font-size: 20px; font-weight: 600;", "{bonk_balance:.2} BONK" }
                                }
                                
                                // Amount input
                                div {
                                    style: "margin-bottom: 16px;",
                                    label { style: "display: block; margin-bottom: 6px; font-size: 12px; font-weight: 600;", "Amount to Stake:" }
                                    div {
                                        style: "display: flex; gap: 6px;",
                                        input {
                                            class: "amount-input",
                                            r#type: "text",
                                            placeholder: "0.00",
                                            style: "flex: 1; font-size: 14px;",
                                            value: "{amount()}",
                                            oninput: move |evt| {
                                                amount.set(evt.value().clone());
                                                error_message.set(None);
                                            }
                                        }
                                        button {
                                            class: "button-standard secondary",
                                            style: "padding: 8px 16px; font-size: 11px;",
                                            onclick: move |_| amount.set(format!("{:.2}", bonk_balance())),
                                            "Max"
                                        }
                                    }
                                }
                                
                                // Lock duration selector (improved cards)
                                div {
                                    style: "margin-bottom: 16px;",
                                    label { style: "display: block; margin-bottom: 8px; font-size: 12px; font-weight: 600;", "Select Lock Duration:" }
                                    div {
                                        style: "display: grid; grid-template-columns: 1fr 1fr; gap: 8px;",
                                        for (days, label, multiplier) in duration_options {
                                            button {
                                                key: "{days}",
                                                class: if selected_duration() == days { "duration-card-selected" } else { "duration-card" },
                                                onclick: move |_| selected_duration.set(days),
                                                div { style: "font-size: 16px; font-weight: 600; margin-bottom: 4px;", "{label}" }
                                                div { style: "font-size: 11px; opacity: 0.7;", "{multiplier}x weight" }
                                            }
                                        }
                                    }
                                }
                                
                                // Summary
                                {
                                    let show_summary = !amount().is_empty() && amount().parse::<f64>().unwrap_or(0.0) > 0.0;
                                    if show_summary {
                                        let amt = amount().parse::<f64>().unwrap_or(0.0);
                                        let duration = selected_duration();
                                        let multiplier = duration_options_for_summary.iter()
                                            .find(|(d, _, _)| *d == duration)
                                            .map(|(_, _, m)| *m)
                                            .unwrap_or(1.0);
                                        
                                        rsx! {
                                            div {
                                                style: "background: rgba(255,255,255,0.05); border-radius: 12px; padding: 16px; margin-bottom: 16px;",
                                                h4 { style: "font-size: 14px; margin-bottom: 12px;", "Stake Summary" }
                                                div { style: "display: flex; justify-content: space-between; margin-bottom: 8px; font-size: 13px;", span { "Amount:" } span { "{amt:.2} BONK" } }
                                                div { style: "display: flex; justify-content: space-between; margin-bottom: 8px; font-size: 13px;", span { "Duration:" } span { "{duration} days" } }
                                                div { style: "display: flex; justify-content: space-between; font-size: 13px; font-weight: 600;", span { "Weight Multiplier:" } span { style: "color: #22c55e;", "{multiplier}x" } }
                                            }
                                        }
                                    } else {
                                        rsx! { div { style: "display: none;" } }
                                    }
                                }
                                
                                // Stake button
                                button {
                                    class: "button-standard primary",
                                    style: "width: 100%; padding: 12px; font-size: 14px;",
                                    disabled: processing() || amount().is_empty(),
                                    onclick: {
                                        let wallet_c = wallet.clone();
                                        let hw_c = hardware_wallet.clone();
                                        let rpc_c = custom_rpc.clone();
                                        
                                        move |_| {
                                            let amt_str = amount();
                                            let duration = selected_duration();
                                            
                                            let amt_f64 = match amt_str.parse::<f64>() {
                                                Ok(a) if a > 0.0 => a,
                                                _ => {
                                                    error_message.set(Some("Please enter a valid amount".to_string()));
                                                    return;
                                                }
                                            };
                                            
                                            if amt_f64 > bonk_balance() {
                                                error_message.set(Some("Insufficient BONK balance".to_string()));
                                                return;
                                            }
                                            
                                            processing.set(true);
                                            error_message.set(None);
                                            if has_hardware { show_hardware_approval.set(true); }
                                            
                                            let wallet_clone = wallet_c.clone();
                                            let hw_clone = hw_c.clone();
                                            let rpc_clone = rpc_c.clone();
                                            
                                            spawn(async move {
                                                let signer: Box<dyn TransactionSigner> = if let Some(hw) = hw_clone {
                                                    Box::new(crate::signing::hardware::HardwareSigner::from_wallet(hw))
                                                } else if let Some(w) = wallet_clone {
                                                    match crate::wallet::Wallet::from_wallet_info(&w) {
                                                        Ok(wallet_obj) => Box::new(crate::signing::software::SoftwareSigner::new(wallet_obj)),
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
                                                
                                                let client = BonkStakingClient::new(rpc_clone.as_deref());
                                                let amount_lamports = (amt_f64 * 1_000_000_000.0) as u64;
                                                
                                                match client.stake_bonk_with_signer(&*signer, amount_lamports, duration).await {
                                                    Ok(result) => {
                                                        show_hardware_approval.set(false);
                                                        transaction_signature.set(result.signature);
                                                        show_success_modal.set(true);
                                                    }
                                                    Err(e) => {
                                                        show_hardware_approval.set(false);
                                                        error_message.set(Some(format!("Stake failed: {}", e)));
                                                    }
                                                }
                                                
                                                processing.set(false);
                                            });
                                        }
                                    },
                                    if processing() { "Staking..." } else { "Stake BONK" }
                                }
                            }
                        }
                    }
                }
                

            }
        }
    }
}