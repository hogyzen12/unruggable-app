use dioxus::prelude::*;
use crate::wallet::WalletInfo;
use crate::hardware::HardwareWallet;
use crate::squads::{SquadsClient, MultisigInfo, PendingTransaction};
use crate::signing::{SignerType, TransactionSigner};
use std::sync::Arc;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

/// Hardware wallet approval overlay for Squads transactions
#[component]
fn HardwareApprovalOverlay(oncancel: EventHandler<()>) -> Element {
    rsx! {
        div {
            class: "hardware-approval-overlay",
            
            div {
                class: "hardware-approval-content",
                
                h3 { 
                    class: "hardware-approval-title",
                    "Confirm Approval on Hardware Wallet"
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
                    "Please check your hardware wallet and confirm the multisig approval."
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

/// Success modal for approved transactions
#[component]
fn ApprovalSuccessModal(
    signature: String,
    threshold_met: bool,
    approval_count: u16,
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
                
                h2 { class: "modal-title", "Transaction Approved!" }
                
                div {
                    class: "tx-icon-container",
                    div {
                        class: "tx-success-icon",
                        if threshold_met { "✅" } else { "⏳" }
                    }
                }
                
                div {
                    class: "success-message",
                    if threshold_met {
                        "Threshold met! Transaction is ready to execute."
                    } else {
                        "Your approval was recorded. More approvals needed."
                    }
                }
                
                div {
                    class: "stake-success-details",
                    div {
                        class: "stake-detail-card",
                        div {
                            class: "stake-detail-label",
                            "Approvals:"
                        }
                        div {
                            class: "stake-detail-value",
                            "{approval_count}"
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
                            if threshold_met { "✅ Ready to Execute" } else { "⏳ Awaiting Approvals" }
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
                                class: "button-standard ghost",
                                href: "{solana_explorer_url}",
                                target: "_blank",
                                "Solana Explorer"
                            }
                            a {
                                class: "button-standard ghost",
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
                        class: "button-standard primary",
                        onclick: move |_| onclose.call(()),
                        "Close"
                    }
                }
            }
        }
    }
}

#[component]
pub fn SquadsModal(
    wallet: Option<WalletInfo>,
    hardware_wallet: Option<Arc<HardwareWallet>>,
    custom_rpc: Option<String>,
    onclose: EventHandler<()>,
) -> Element {
    // State management
    let mut loading_multisigs = use_signal(|| false);
    let mut multisigs = use_signal(|| Vec::<MultisigInfo>::new());
    let mut pending_transactions = use_signal(|| Vec::<PendingTransaction>::new());
    let mut loading_pending_transactions = use_signal(|| false);
    let mut selected_multisig = use_signal(|| None as Option<MultisigInfo>);
    let mut show_multisig_dropdown = use_signal(|| false);
    let mut error_message = use_signal(|| None as Option<String>);
    let mut approving = use_signal(|| false);
    let mut show_hardware_approval = use_signal(|| false);
    let mut show_success_modal = use_signal(|| false);
    let mut success_signature = use_signal(|| String::new());
    let mut success_threshold_met = use_signal(|| false);
    let mut success_approval_count = use_signal(|| 0u16);

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

    // Clone values before use_effect to avoid move issues
    let wallet_for_effect = wallet.clone();
    let hardware_wallet_for_effect = hardware_wallet.clone();
    let custom_rpc_for_effect = custom_rpc.clone();

    // Load multisigs on mount - using same pattern as stake_modal
    use_effect(move || {
        // GUARD: Don't run if already loading
        if loading_multisigs() {
            println!("Already loading multisigs, skipping...");
            return;
        }
        
        // GUARD: Don't run if we already have data
        if !multisigs().is_empty() {
            println!("Already have {} multisigs, skipping...", multisigs().len());
            return;
        }
        
        println!("Starting multisig fetch...");
        loading_multisigs.set(true);
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
                        loading_multisigs.set(false);
                        return;
                    }
                }
            } else if let Some(w) = &wallet_clone {
                w.address.clone()
            } else {
                error_message.set(Some("No wallet available".to_string()));
                loading_multisigs.set(false);
                return;
            };

            let wallet_pubkey = match Pubkey::from_str(&wallet_address) {
                Ok(pk) => pk,
                Err(e) => {
                    error_message.set(Some(format!("Invalid wallet address: {}", e)));
                    loading_multisigs.set(false);
                    return;
                }
            };

            // Create Squads client
            let client = SquadsClient::new(custom_rpc_clone.as_deref());

            // Fetch multisigs from Squads API
            match client.find_user_multisigs(&wallet_pubkey).await {
                Ok(found_multisigs) => {
                    if !found_multisigs.is_empty() {
                        println!("Found {} multisigs for wallet", found_multisigs.len());
                        
                        // Set the first multisig as selected (default selection)
                        if let Some(first_multisig) = found_multisigs.first() {
                            selected_multisig.set(Some(first_multisig.clone()));
                            println!("Selected default multisig: {}", first_multisig.name);
                            
                            // Fetch pending transactions for the selected multisig
                            loading_pending_transactions.set(true);
                            println!("Fetching pending transactions for multisig: {}", first_multisig.address);
                            match client.find_pending_transactions(&first_multisig.address, &wallet_pubkey).await {
                                Ok(found_pending) => {
                                    println!("Found {} pending transactions", found_pending.len());
                                    for (i, tx) in found_pending.iter().enumerate() {
                                        println!("  Transaction {}: index={}, proposal={}, has_approved={}, approved_count={}", 
                                            i, tx.transaction_index, tx.proposal, tx.has_approved, tx.approved_count);
                                    }
                                    println!("Setting pending_transactions state with {} transactions", found_pending.len());
                                    pending_transactions.set(found_pending);
                                    println!("pending_transactions state updated, current count: {}", pending_transactions().len());
                                    loading_pending_transactions.set(false);
                                }
                                Err(e) => {
                                    println!("Error fetching pending transactions: {}", e);
                                    loading_pending_transactions.set(false);
                                    // Don't show error to user, just log it
                                    // Pending transactions are optional
                                }
                            }
                        }
                        
                        multisigs.set(found_multisigs);
                    } else {
                        println!("No multisigs found for wallet");
                    }
                }
                Err(e) => {
                    error_message.set(Some(format!("Error loading multisigs: {}", e)));
                    println!("Error loading multisigs: {}", e);
                }
            }

            loading_multisigs.set(false);
        });
    });

    // CRITICAL: Read signals at top level to establish reactivity tracking
    let pending_txs = pending_transactions();
    let pending_txs_loading = loading_pending_transactions();
    let selected_ms = selected_multisig();
    
    println!("RENDER: pending_transactions count = {}, loading = {}, selected_multisig = {}", 
        pending_txs.len(), pending_txs_loading, selected_ms.is_some());

    // Show success modal if approval was successful
    if show_success_modal() {
        return rsx! {
            ApprovalSuccessModal {
                signature: success_signature(),
                threshold_met: success_threshold_met(),
                approval_count: success_approval_count(),
                onclose: move |_| {
                    show_success_modal.set(false);
                    onclose.call(());
                }
            }
        };
    }

    rsx! {
            div {
                class: "modal-backdrop",
            onclick: move |_| {
                show_multisig_dropdown.set(false);
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
                            approving.set(false);
                        }
                    }
                }

                // Header
                div {
                    class: "modal-header",
                    h2 { 
                        class: "modal-title",
                        "Squads Multisig"
                    }
                    button {
                        class: "modal-close-button",
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

                div {
                    class: "modal-body",

                    // Loading state
                    if loading_multisigs() {
                        div {
                            class: "loading-stakes-modern",
                            div { class: "loading-spinner" }
                            "Loading multisigs..."
                        }
                    }
                    // Empty state
                    else if multisigs().is_empty() {
                        div {
                            class: "no-stakes-modern",
                            div {
                                class: "no-stakes-icon",
                                "🏛️"
                            }
                            div {
                                class: "no-stakes-title",
                                "No Multisigs Found"
                            }
                            div {
                                class: "no-stakes-description",
                                "You are not a member of any multisig accounts yet."
                            }
                        }
                    }
                    // Display multisig selector and details
                    else {
                        // Multisig Selector Dropdown (matching validator dropdown style)
                        div {
                            class: "wallet-field",
                            label { "Select Multisig:" }
                            div {
                                class: "validator-selector",
                                button {
                                    class: "validator-dropdown-button",
                                    onclick: move |e| {
                                        e.stop_propagation();
                                        show_multisig_dropdown.set(!show_multisig_dropdown());
                                    },
                                    if let Some(multisig) = selected_multisig() {
                                        div {
                                            class: "selected-validator",
                                            div {
                                                class: "validator-name",
                                                "{multisig.name}"
                                            }
                                            div {
                                                class: "validator-details",
                                                "Threshold: {multisig.threshold}/{multisig.members.len()} • Vault: {multisig.vault_balance:.6} SOL"
                                            }
                                        }
                                    } else {
                                        div {
                                            class: "validator-placeholder",
                                            "Select a multisig..."
                                        }
                                    }
                                    
                                    div {
                                        class: "dropdown-arrow",
                                        if show_multisig_dropdown() { "▲" } else { "▼" }
                                    }
                                }
                        
                                // Multisig Dropdown
                                if show_multisig_dropdown() {
                                    div {
                                        class: "validator-dropdown",
                                        onclick: move |e| e.stop_propagation(),
                                        for multisig in multisigs() {
                                            div {
                                                key: "{multisig.address}",
                                                class: "validator-option",
                                                onclick: {
                                                    let multisig_addr = multisig.address;
                                                    let wallet_c = wallet.clone();
                                                    let hw_c = hardware_wallet.clone();
                                                    let rpc_c = custom_rpc.clone();
                                                    
                                                    move |_| {
                                                        selected_multisig.set(Some(multisig.clone()));
                                                        show_multisig_dropdown.set(false);
                                                        error_message.set(None);
                                                        println!("Selected multisig: {}", multisig.name);
                                                        
                                                        // Clone for spawn closure
                                                        let wallet_clone = wallet_c.clone();
                                                        let hw_clone = hw_c.clone();
                                                        let rpc_clone = rpc_c.clone();
                                                        
                                                        spawn(async move {
                                                            // Get wallet address
                                                            let wallet_address = if let Some(hw) = &hw_clone {
                                                                match hw.get_public_key().await {
                                                                    Ok(addr) => addr,
                                                                    Err(_) => return,
                                                                }
                                                            } else if let Some(w) = &wallet_clone {
                                                                w.address.clone()
                                                            } else {
                                                                return;
                                                            };

                                                            let wallet_pubkey = match Pubkey::from_str(&wallet_address) {
                                                                Ok(pk) => pk,
                                                                Err(_) => return,
                                                            };

                                                            let client = SquadsClient::new(rpc_clone.as_deref());
                                                            
                                                            loading_pending_transactions.set(true);
                                                            println!("Fetching pending transactions for multisig: {}", multisig_addr);
                                                            match client.find_pending_transactions(&multisig_addr, &wallet_pubkey).await {
                                                                Ok(found_pending) => {
                                                                    println!("Found {} pending transactions", found_pending.len());
                                                                    for (i, tx) in found_pending.iter().enumerate() {
                                                                        println!("  Transaction {}: index={}, proposal={}, has_approved={}, approved_count={}", 
                                                                            i, tx.transaction_index, tx.proposal, tx.has_approved, tx.approved_count);
                                                                    }
                                                                    println!("Setting pending_transactions state with {} transactions", found_pending.len());
                                                                    pending_transactions.set(found_pending);
                                                                    println!("pending_transactions state updated, current count: {}", pending_transactions().len());
                                                                    loading_pending_transactions.set(false);
                                                                }
                                                                Err(e) => {
                                                                    println!("Error fetching pending transactions: {}", e);
                                                                    loading_pending_transactions.set(false);
                                                                }
                                                            }
                                                        });
                                                    }
                                                },
                                                div {
                                                    class: "validator-option-header",
                                                    div {
                                                        class: "validator-option-name",
                                                        "{multisig.name}"
                                                    }
                                                    div {
                                                        class: "validator-commission",
                                                        "{multisig.vault_balance:.6} SOL"
                                                    }
                                                }
                                                div {
                                                    class: "validator-description",
                                                    "Threshold: {multisig.threshold}/{multisig.members.len()} • Transaction Index: {multisig.transaction_index}"
                                                }
                                                div {
                                                    class: "validator-stats",
                                                    "Address: {multisig.address}"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Selected Multisig Details
                        if let Some(multisig) = selected_ms.clone() {
                            // Multisig Info Section
                            div {
                                class: "stakes-overview-modern",
                                
                                div {
                                    class: "stake-account-modern",
                                    
                                    div {
                                        class: "stake-account-header-modern",
                                        div {
                                            class: "validator-info-modern",
                                            div {
                                                class: "validator-details-modern",
                                                div {
                                                    class: "validator-name-modern",
                                                    "Multisig Details"
                                                }
                                                div {
                                                    class: "validator-description-text",
                                                    "Address: {multisig.address}"
                                                }
                                                div {
                                                    class: "validator-description-text",
                                                    "Vault: {multisig.vault_address}"
                                                }
                                                div {
                                                    class: "validator-description-text",
                                                    "Vault Balance: {multisig.vault_balance:.6} SOL"
                                                }
                                                div {
                                                    class: "validator-description-text",
                                                    "Members: {multisig.members.len()} • Threshold: {multisig.threshold}"
                                                }
                                                div {
                                                    class: "validator-description-text",
                                                    "Transaction Index: {multisig.transaction_index}"
                                                }
                                                
                                                // Pending Transactions inline
                                                div {
                                                    style: "margin-top: 20px; padding-top: 20px; border-top: 1px solid #3a3a3a;",
                                                    
                                                    div {
                                                        class: "validator-name-modern",
                                                        style: "margin-bottom: 10px;",
                                                        "Pending Transactions ({pending_txs.len()})"
                                                    }
                                                    
                                                    if pending_txs_loading {
                                                        div {
                                                            class: "validator-description-text",
                                                            "Loading pending transactions..."
                                                        }
                                                    } else if pending_txs.is_empty() {
                                                        div {
                                                            class: "validator-description-text",
                                                            "No pending transactions"
                                                        }
                                                                    } else {
                                                                        for tx in pending_txs.clone() {
                                                                            div {
                                                                key: "{tx.transaction}",
                                                                style: "margin-top: 15px; padding: 15px; background: #1a1a1a; border-radius: 12px; border: 1.5px solid #4a4a4a;",
                                                                
                                                                div {
                                                                    class: "validator-name-modern",
                                                                    style: "font-size: 14px; margin-bottom: 8px;",
                                                                    "{tx.description}"
                                                                }
                                                                
                                                                div {
                                                                    class: "validator-description-text",
                                                                    "Proposal: {tx.proposal}"
                                                                }
                                                                div {
                                                                    class: "validator-description-text",
                                                                    "Approvals: {tx.approved_count}/{multisig.threshold}"
                                                                }
                                                                div {
                                                                    class: "validator-description-text",
                                                                    "Status: {tx.status:?}"
                                                                }
                                                                
                                                                div {
                                                                    style: "margin-top: 10px;",
                                                                    
                                                                    // Check if threshold is met (transaction is ready to execute)
                                                                    if tx.approved_count >= multisig.threshold {
                                                                        // Transaction is approved and ready to execute
                                                                        button {
                                                                            class: "button-standard primary",
                                                                            disabled: approving(),
                                                                            style: "width: 100%; background: white; color: #1a1a1a; font-weight: 700; text-transform: uppercase; letter-spacing: 0.5px; border-radius: 12px; padding: 14px 24px;",
                                                                            onclick: {
                                                                                let tx_index = tx.transaction_index;
                                                                                let multisig_addr = tx.multisig;
                                                                                let wallet_clone = wallet.clone();
                                                                                let hw_clone = hardware_wallet.clone();
                                                                                let rpc_clone = custom_rpc.clone();
                                                                                
                                                                                move |_| {
                                                                                    approving.set(true);
                                                                                    error_message.set(None);
                                                                                    
                                                                                    let wallet_c = wallet_clone.clone();
                                                                                    let hw_c = hw_clone.clone();
                                                                                    let rpc_c = rpc_clone.clone();
                                                                                    
                                                                                    spawn(async move {
                                                                                        // Create signer
                                                                                        let signer: Box<dyn TransactionSigner> = if let Some(hw) = hw_c {
                                                                                            show_hardware_approval.set(true);
                                                                                            Box::new(crate::signing::hardware::HardwareSigner::from_wallet(hw))
                                                                                        } else if let Some(w) = wallet_c {
                                                                                            match crate::wallet::Wallet::from_wallet_info(&w) {
                                                                                                Ok(wallet_obj) => {
                                                                                                    Box::new(crate::signing::software::SoftwareSigner::new(wallet_obj))
                                                                                                }
                                                                                                Err(e) => {
                                                                                                    error_message.set(Some(format!("Failed to load wallet: {}", e)));
                                                                                                    approving.set(false);
                                                                                                    return;
                                                                                                }
                                                                                            }
                                                                                        } else {
                                                                                            error_message.set(Some("No wallet available".to_string()));
                                                                                            approving.set(false);
                                                                                            return;
                                                                                        };
                                                                                        
                                                                                        // Create client and execute
                                                                                        let client = SquadsClient::new(rpc_c.as_deref());
                                                                                        
                                                                                        match client.execute_transaction_with_signer(&*signer, &multisig_addr, tx_index).await {
                                                                                            Ok(signature) => {
                                                                                                show_hardware_approval.set(false);
                                                                                                success_signature.set(signature);
                                                                                                success_threshold_met.set(true);
                                                                                                success_approval_count.set(tx.approved_count);
                                                                                                show_success_modal.set(true);
                                                                                            }
                                                                                            Err(e) => {
                                                                                                show_hardware_approval.set(false);
                                                                                                error_message.set(Some(format!("Execution failed: {}", e)));
                                                                                            }
                                                                                        }
                                                                                        
                                                                                        approving.set(false);
                                                                                    });
                                                                                }
                                                                            },
                                                                            if approving() { "Executing..." } else { "Execute Transaction" }
                                                                        }
                                                                    } else if tx.has_approved {
                                                                        div {
                                                                            style: "color: #9ca3af; font-weight: 600; text-align: center; padding: 12px; background: #2a2a2a; border-radius: 8px;",
                                                                            "✓ You have approved"
                                                                        }
                                                                    } else {
                                                                        button {
                                                                            class: "button-standard primary",
                                                                            disabled: approving(),
                                                                            style: "width: 100%; background: white; color: #1a1a1a; font-weight: 700; text-transform: uppercase; letter-spacing: 0.5px; border-radius: 12px; padding: 14px 24px;",
                                                                            onclick: {
                                                                                let tx_index = tx.transaction_index;
                                                                                let multisig_addr = tx.multisig;
                                                                                let wallet_clone = wallet.clone();
                                                                                let hw_clone = hardware_wallet.clone();
                                                                                let rpc_clone = custom_rpc.clone();
                                                                                
                                                                                move |_| {
                                                                                    approving.set(true);
                                                                                    error_message.set(None);
                                                                                    
                                                                                    let wallet_c = wallet_clone.clone();
                                                                                    let hw_c = hw_clone.clone();
                                                                                    let rpc_c = rpc_clone.clone();
                                                                                    
                                                                                    spawn(async move {
                                                                                        // Create signer
                                                                                        let signer: Box<dyn TransactionSigner> = if let Some(hw) = hw_c {
                                                                                            show_hardware_approval.set(true);
                                                                                            Box::new(crate::signing::hardware::HardwareSigner::from_wallet(hw))
                                                                                        } else if let Some(w) = wallet_c {
                                                                                            match crate::wallet::Wallet::from_wallet_info(&w) {
                                                                                                Ok(wallet_obj) => {
                                                                                                    Box::new(crate::signing::software::SoftwareSigner::new(wallet_obj))
                                                                                                }
                                                                                                Err(e) => {
                                                                                                    error_message.set(Some(format!("Failed to load wallet: {}", e)));
                                                                                                    approving.set(false);
                                                                                                    return;
                                                                                                }
                                                                                            }
                                                                                        } else {
                                                                                            error_message.set(Some("No wallet available".to_string()));
                                                                                            approving.set(false);
                                                                                            return;
                                                                                        };
                                                                                        
                                                                                        // Create client and approve
                                                                                        let client = SquadsClient::new(rpc_c.as_deref());
                                                                                        
                                                                                        match client.approve_transaction_with_signer(&*signer, &multisig_addr, tx_index).await {
                                                                                            Ok(result) => {
                                                                                                show_hardware_approval.set(false);
                                                                                                success_signature.set(result.signature);
                                                                                                success_threshold_met.set(result.threshold_met);
                                                                                                success_approval_count.set(result.approval_count);
                                                                                                show_success_modal.set(true);
                                                                                            }
                                                                                            Err(e) => {
                                                                                                show_hardware_approval.set(false);
                                                                                                error_message.set(Some(format!("Approval failed: {}", e)));
                                                                                            }
                                                                                        }
                                                                                        
                                                                                        approving.set(false);
                                                                                    });
                                                                                }
                                                                            },
                                                                            if approving() { "Approving..." } else { "Approve Transaction" }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }


            }
        }
    }
}