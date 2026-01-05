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
    let solscan_url = format!("https://solscan.io/tx/{}", signature);
    let orb_url = format!("https://orb.helius.dev/tx/{}?cluster=mainnet-beta&tab=summary", signature);
    
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
                            a { class: "button-standard ghost", href: "{solscan_url}", target: "_blank", "Solscan" }
                            a { class: "button-standard ghost", href: "{orb_url}", target: "_blank", "Orb" }
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
    let mut selected_mode = use_signal(|| "stake".to_string()); // "view" or "stake" - default to stake page
    let mut amount = use_signal(|| "".to_string());
    let mut selected_duration = use_signal(|| 30u64);
    let mut processing = use_signal(|| false);
    let mut error_message = use_signal(|| None::<String>);
    
    // Balance and stakes
    let mut bonk_balance = use_signal(|| 0.0);
    let mut active_stakes = use_signal(|| Vec::<StakePosition>::new());
    let mut fetching_balance = use_signal(|| false);
    let mut fetching_stakes = use_signal(|| false);
    let mut balance_loaded = use_signal(|| false);
    let mut stakes_loaded = use_signal(|| false);
    
    // Success modal
    let mut show_success_modal = use_signal(|| false);
    let mut transaction_signature = use_signal(|| "".to_string());
    let mut was_hardware_transaction = use_signal(|| false);
    let mut show_hardware_approval = use_signal(|| false);
    
    // Fetch BONK balance on mount
    let custom_rpc_for_effect = custom_rpc.clone();
    use_effect(move || {
        if let Some(address) = wallet_address() {
            if !fetching_balance() && !balance_loaded() {
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
                    balance_loaded.set(true);
                });
            }
        }
    });
    
    // Fetch active stakes
    let custom_rpc_for_stakes = custom_rpc.clone();
    use_effect(move || {
        if let Some(address) = wallet_address() {
            if !fetching_stakes() && !stakes_loaded() {
                fetching_stakes.set(true);
                
                let rpc_url = custom_rpc_for_stakes.clone();
                spawn(async move {
                    let client = BonkStakingClient::new(rpc_url.as_deref());
                    
                    match client.get_user_stakes(&address).await {
                        Ok(stakes) => active_stakes.set(stakes),
                        Err(e) => error_message.set(Some(format!("Failed to fetch stakes: {}", e))),
                    }
                    
                    fetching_stakes.set(false);
                    stakes_loaded.set(true);
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
                
                // Header
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
                        "BONK Staking"
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
                
                if let Some(error) = error_message() {
                    div { class: "error-message", "{error}" }
                }
                
                div { class: "modal-body",

                    // Mode toggle
                    div {
                        class: "mode-toggle",
                        style: "margin-bottom: 16px;",
                        button {
                            class: if selected_mode() == "view" { "toggle-button active" } else { "toggle-button" },
                            onclick: move |_| selected_mode.set("view".to_string()),
                            "My Positions"
                        }
                        button {
                            class: if selected_mode() == "stake" { "toggle-button active" } else { "toggle-button" },
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
                                    style: "display: grid; grid-template-columns: 1fr; gap: 10px; margin-bottom: 12px;",
                                    div {
                                        style: "background: #141414; border: 1px solid #353535; border-radius: 12px; padding: 12px;",
                                        div { style: "font-size: 11px; color: #9ca3af; margin-bottom: 4px;", "Total Locked" }
                                        div { style: "font-size: 18px; font-weight: 600; color: white;", "{total_locked:.2} BONK" }
                                    }
                                    div {
                                        style: "background: #141414; border: 1px solid #353535; border-radius: 12px; padding: 12px;",
                                        div { style: "font-size: 11px; color: #9ca3af; margin-bottom: 4px;", "Claimable" }
                                        div { style: "font-size: 18px; font-weight: 600; color: white;", "{total_claimable:.2} BONK" }
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
                                                key: "{stake.receipt_address}",
                                                style: "background: #141414; border: 1px solid #353535; border-radius: 12px; padding: 12px; margin-bottom: 10px;",
                                                div {
                                                    style: "display: flex; justify-content: space-between; align-items: center;",
                                                    div {
                                                        div { style: "font-size: 16px; font-weight: 600; color: white;", "{stake.amount:.2} BONK" }
                                                        div { style: "font-size: 11px; color: #9ca3af; margin-top: 2px;", "{stake.duration_days} days • {stake.multiplier}x" }
                                                    }
                                                    div {
                                                        style: if stake.is_unlocked { "background: #2f2f2f; color: white; padding: 4px 10px; border-radius: 999px; font-size: 10px; font-weight: 600; border: 1px solid #4a4a4a;" } else { "background: #1f1f1f; color: #d1d5db; padding: 4px 10px; border-radius: 999px; font-size: 10px; border: 1px solid #353535;" },
                                                        if stake.is_unlocked { "Unlocked" } else { "Locked" }
                                                    }
                                                }
                                                div {
                                                    style: "margin-top: 8px; font-size: 11px; color: #9ca3af;",
                                                    "Unlocks: {stake.unlock_time}"
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
                                    style: "background: #1a1a1a; border: 1.5px solid #4a4a4a; border-radius: 12px; padding: 16px; margin-bottom: 16px;",
                                    div { style: "font-size: 11px; color: #9ca3af; margin-bottom: 4px;", "Available Balance" }
                                    div { style: "font-size: 20px; font-weight: 600; color: white;", "{bonk_balance:.2} BONK" }
                                }
                                
                                // Amount input
                                div {
                                    style: "margin-bottom: 16px;",
                                    label { style: "display: block; margin-bottom: 10px; color: #9ca3af; font-size: 13px; font-weight: 500;", "Amount to Stake:" }
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
                                            style: "padding: 10px 16px; font-size: 11px; background: #3a3a3a; color: white; border: 1px solid #5a5a5a; border-radius: 8px; font-weight: 600;",
                                            onclick: move |_| amount.set(format!("{:.2}", bonk_balance())),
                                            "Max"
                                        }
                                    }
                                }
                                
                                // Lock duration selector (improved cards)
                                div {
                                    style: "margin-bottom: 16px;",
                                    label { style: "display: block; margin-bottom: 10px; color: #9ca3af; font-size: 13px; font-weight: 500;", "Select Lock Duration:" }
                                    div {
                                        style: "display: grid; grid-template-columns: 1fr 1fr; gap: 8px;",
                                        for (days, label, multiplier) in duration_options {
                                            button {
                                                key: "{days}",
                                                class: if selected_duration() == days { "duration-card-selected" } else { "duration-card" },
                                                onclick: move |_| selected_duration.set(days),
                                                div { style: "font-size: 16px; font-weight: 600; color: white; margin-bottom: 4px;", "{label}" }
                                                div { style: "font-size: 11px; color: #9ca3af;", "{multiplier}x weight" }
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
                                                style: "background: #1a1a1a; border: 1.5px solid #4a4a4a; border-radius: 12px; padding: 16px; margin-bottom: 16px;",
                                                h4 { style: "font-size: 14px; margin-bottom: 12px; color: white;", "Stake Summary" }
                                                div { style: "display: flex; justify-content: space-between; margin-bottom: 8px; font-size: 13px; color: white;", span { "Amount:" } span { "{amt:.2} BONK" } }
                                                div { style: "display: flex; justify-content: space-between; margin-bottom: 8px; font-size: 13px; color: white;", span { "Duration:" } span { "{duration} days" } }
                                                div { style: "display: flex; justify-content: space-between; font-size: 13px; font-weight: 600; color: white;", span { "Weight Multiplier:" } span { "{multiplier}x" } }
                                            }
                                        }
                                    } else {
                                        rsx! { div { style: "display: none;" } }
                                    }
                                }
                                
                                // Stake button
                                button {
                                    class: "button-standard primary",
                                    style: "width: 100%; background: white; color: #1a1a1a; font-weight: 700; text-transform: uppercase; letter-spacing: 0.5px; border-radius: 12px; padding: 14px 24px;",
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
                                                let is_hardware = hw_clone.is_some();

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
                                                let amount_lamports = (amt_f64 * 100_000.0) as u64;
                                                
                                                match client.stake_bonk_with_signer(&*signer, amount_lamports, duration, is_hardware).await {
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
