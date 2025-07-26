use dioxus::prelude::*;
use crate::wallet::WalletInfo;
use crate::hardware::HardwareWallet;
use crate::validators::{ValidatorInfo, get_recommended_validators};
use crate::staking::{self, DetailedStakeAccount, StakeAccountState};
use std::sync::Arc;
use crate::signing::hardware::HardwareSigner;
use crate::staking::create_stake_account;

#[derive(PartialEq, Clone)]
enum ModalMode {
    Stake,
    MyStakes,
}

/// Hardware wallet approval overlay component for staking transactions
#[component]
fn HardwareApprovalOverlay(oncancel: EventHandler<()>) -> Element {
    rsx! {
        div {
            class: "hardware-approval-overlay",
            
            div {
                class: "hardware-approval-content",
                
                h3 { 
                    class: "hardware-approval-title",
                    "Confirm Staking on Hardware Wallet"
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
                    "Please check your hardware wallet and confirm the staking transaction details."
                }
                
                div {
                    class: "hardware-steps",
                    div {
                        class: "hardware-step",
                        div { class: "step-number", "1" }
                        span { "Review the staking details on your Unruggable" }
                    }
                    div {
                        class: "hardware-step",
                        div { class: "step-number", "2" }
                        span { "Press the button to confirm the transaction" }
                    }
                }
                
                button {
                    class: "hardware-cancel-button",
                    onclick: move |_| oncancel.call(()),
                    "Cancel Staking"
                }
            }
        }
    }
}

/// Modal component to display staking success details
#[component]
fn StakeSuccessModal(
    signature: String,
    stake_amount: f64,
    validator_name: String,
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
                
                h2 { class: "modal-title", "Stake Account Created Successfully!" }

                if was_hardware_wallet {
                    div {
                        class: "success-message",
                        "🔐 Staking transaction completed successfully with your hardware wallet!"
                    }
                }
                
                div {
                    class: "tx-icon-container",
                    div {
                        class: "tx-success-icon stake-success",
                        "🏛️" // Staking icon
                    }
                }
                
                div {
                    class: "success-message",
                    "Your stake account was created and delegated to the validator."
                }
                
                div {
                    class: "stake-success-details",
                    div {
                        class: "stake-detail-card",
                        div {
                            class: "stake-detail-label",
                            "Staked Amount:"
                        }
                        div {
                            class: "stake-detail-value stake-amount",
                            "{stake_amount:.6} SOL"
                        }
                    }
                    
                    div {
                        class: "stake-detail-card",
                        div {
                            class: "stake-detail-label",
                            "Validator:"
                        }
                        div {
                            class: "stake-detail-value",
                            "🏛️ {validator_name}"
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
                            "✅ Activating (2-3 epochs)"
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
                            onclick: move |_| {
                                println!("Signature copied to clipboard: {}", signature);
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

#[component]
pub fn StakeModal(
    wallet: Option<WalletInfo>,
    hardware_wallet: Option<Arc<HardwareWallet>>,
    current_balance: f64,
    custom_rpc: Option<String>,
    onclose: EventHandler<()>,
    onsuccess: EventHandler<String>,
) -> Element {
    // State management
    let mut mode = use_signal(|| ModalMode::Stake);
    let mut amount = use_signal(|| "".to_string());
    let mut selected_validator = use_signal(|| None as Option<ValidatorInfo>);
    let mut show_validator_dropdown = use_signal(|| false);
    let mut staking = use_signal(|| false);
    let mut loading_stakes = use_signal(|| false);
    let mut error_message = use_signal(|| None as Option<String>);
    let mut validators = use_signal(|| Vec::<ValidatorInfo>::new());
    let mut stake_accounts = use_signal(|| Vec::<DetailedStakeAccount>::new());
    
    // Add state for staking success modal
    let mut show_success_modal = use_signal(|| false);
    let mut success_signature = use_signal(|| "".to_string());
    let mut success_amount = use_signal(|| 0.0);
    let mut success_validator = use_signal(|| "".to_string());

    // Hardware wallet prompting states
    let mut show_hardware_approval = use_signal(|| false);
    let mut was_hardware_transaction = use_signal(|| false);

    // Load validators on component mount
    use_effect(move || {
        spawn(async move {
            println!("📋 Stake modal opened - loading validators with live data...");
            
            // This single call handles everything: 
            // - Fetches live data from RPC
            // - Updates with real commission, stake, and skip rates  
            // - Falls back to static data if RPC fails
            // - Prints detailed debug info to console
            let validator_list = get_recommended_validators().await;
            
            // Set default validator (the first one marked as default)
            if let Some(default_validator) = validator_list.iter().find(|v| v.is_default).cloned() {
                println!("🌟 Selected default validator: {}", default_validator.name);
                selected_validator.set(Some(default_validator));
            }
            
            validators.set(validator_list);
            println!("🚀 Validator data loaded and ready for UI");
        });
    });

    // Clone values before use_effect to avoid move issues
    let wallet_for_effect = wallet.clone();
    let hardware_wallet_for_effect = hardware_wallet.clone();
    let custom_rpc_for_effect = custom_rpc.clone();

    // Load stake accounts when switching to My Stakes mode
    {
        let current_mode = mode();
        use_effect(move || {
            if current_mode == ModalMode::MyStakes && !loading_stakes() {
                loading_stakes.set(true);
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
                                loading_stakes.set(false);
                                return;
                            }
                        }
                    } else if let Some(w) = &wallet_clone {
                        w.address.clone()
                    } else {
                        error_message.set(Some("No wallet available".to_string()));
                        loading_stakes.set(false);
                        return;
                    };

                    // Scan for stake accounts
                    match staking::scan_stake_accounts(&wallet_address, custom_rpc_clone.as_deref()).await {
                        Ok(accounts) => {
                            println!("Found {} stake accounts", accounts.len());
                            stake_accounts.set(accounts);
                            loading_stakes.set(false);
                        }
                        Err(e) => {
                            println!("Error scanning stake accounts: {}", e);
                            error_message.set(Some(format!("Failed to load stake accounts: {}", e)));
                            loading_stakes.set(false);
                        }
                    }
                });
            }
        });
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

    // Calculate total staked amount
    let total_staked = stake_accounts().iter()
        .map(|account| account.balance.saturating_sub(account.rent_exempt_reserve))
        .sum::<u64>() as f64 / 1_000_000_000.0;

    // Show success modal if staking was successful
    if show_success_modal() {
        return rsx! {
            StakeSuccessModal {
                signature: success_signature(),
                stake_amount: success_amount(),
                validator_name: success_validator(),
                was_hardware_wallet: was_hardware_transaction(),  // ADD THIS LINE
                onclose: move |_| {
                    show_success_modal.set(false);
                    // Call onsuccess when the user closes the modal
                    onsuccess.call(success_signature());
                }
            }
        };
    }

    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| {
                show_validator_dropdown.set(false);
                onclose.call(());
            },

            div {
                class: "modal-content stake-modal",
                onclick: move |e| e.stop_propagation(),
                style: "position: relative;", // Needed for absolute positioning of overlay

                // Hardware approval overlay - shown when waiting for hardware confirmation
                if show_hardware_approval() {
                    HardwareApprovalOverlay {
                        oncancel: move |_| {
                            show_hardware_approval.set(false);
                            staking.set(false);
                        }
                    }
                }

                // Header with toggle
                div {
                    class: "modal-header-with-toggle",
                    h2 { 
                        class: "modal-title",
                        if mode() == ModalMode::Stake {
                            "Stake SOL"
                        } else {
                            "My Staked Sol"
                        }
                    }
                    
                    div {
                        class: "mode-toggle",
                        button {
                            class: if mode() == ModalMode::Stake { "toggle-button active" } else { "toggle-button" },
                            onclick: move |_| {
                                mode.set(ModalMode::Stake);
                                error_message.set(None);
                            },
                            "Stake SOL"
                        }
                        button {
                            class: if mode() == ModalMode::MyStakes { "toggle-button active" } else { "toggle-button" },
                            onclick: move |_| {
                                mode.set(ModalMode::MyStakes);
                                error_message.set(None);
                            },
                            "My Staked Sol"
                        }
                    }
                }

                // Show error if any
                if let Some(error) = error_message() {
                    div {
                        class: "error-message",
                        "{error}"
                    }
                }

                // Conditional content based on mode
                if mode() == ModalMode::Stake {
                    // Original staking interface
                    div {
                        class: "wallet-field",
                        label { "From Address:" }
                        div { class: "address-display", "{display_address}" }
                    }

                    div {
                        class: "wallet-field",
                        label { "Available Balance:" }
                        div { 
                            class: "balance-display", 
                            "{current_balance:.6} SOL" 
                        }
                    }

                    // Validator Selection
                    div {
                        class: "wallet-field",
                        label { "Choose Validator:" }
                        div {
                            class: "validator-selector",
                            button {
                                class: "validator-dropdown-button",
                                onclick: move |e| {
                                    e.stop_propagation();
                                    show_validator_dropdown.set(!show_validator_dropdown());
                                },
                                if let Some(validator) = selected_validator() {
                                    div {
                                        class: "selected-validator",
                                        div {
                                            class: "validator-name",
                                            "{validator.name}"
                                        }
                                        div {
                                            class: "validator-details",
                                            "Commission: {validator.commission}% • Skip Rate: {validator.skip_rate:.1}%"
                                        }
                                    }
                                } else {
                                    div {
                                        class: "validator-placeholder",
                                        "Select a validator..."
                                    }
                                }
                                
                                div {
                                    class: "dropdown-arrow",
                                    if show_validator_dropdown() { "▲" } else { "▼" }
                                }
                            }
                    
                            // Validator Dropdown
                            if show_validator_dropdown() {
                                div {
                                    class: "validator-dropdown",
                                    onclick: move |e| e.stop_propagation(),
                                    for validator in validators() {
                                        div {
                                            key: "{validator.identity}",
                                            class: "validator-option",
                                            onclick: move |_| {
                                                selected_validator.set(Some(validator.clone()));
                                                show_validator_dropdown.set(false);
                                                error_message.set(None);
                                            },
                                            div {
                                                class: "validator-option-header",
                                                div {
                                                    class: "validator-option-name",
                                                    if validator.is_default {
                                                        "{validator.name} (⭐ Recommended)"
                                                    } else {
                                                        "{validator.name}"
                                                    }
                                                }
                                                div {
                                                    class: "validator-commission",
                                                    "Commission: {validator.commission}%"
                                                }
                                            }
                                            div {
                                                class: "validator-description",
                                                "{validator.description}"
                                            }
                                            if validator.active_stake > 0.0 {
                                                div {
                                                    class: "validator-stats",
                                                    "Active Stake: {validator.active_stake:.0} SOL • Skip Rate: {validator.skip_rate:.1}%"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    div {
                        class: "wallet-field",
                        label { "Amount to Stake (SOL):" }
                        input {
                            class: "amount-input-field",
                            r#type: "number",
                            step: "0.000001",
                            min: "0.01",
                            max: "{current_balance}",
                            placeholder: "0.0",
                            value: "{amount}",
                            oninput: move |e| {
                                amount.set(e.value());
                                error_message.set(None);
                            }
                        }
                        div {
                            class: "field-hint",
                            "Minimum stake amount: 0.01 SOL"
                        }
                    }

                    // Info messages
                    div {
                        class: "stake-info-section",
                        div {
                            class: "info-message warning",
                            "Staked SOL will take 2-3 days to unstake. Make sure you have enough SOL for transaction fees."
                        }
                        if hardware_wallet.is_some() {
                            div {
                                class: "info-message",
                                "🔐 Your hardware wallet will prompt you to approve the staking transaction."
                            }
                        }
                    }
                } else {
                    // My Stakes interface
                    div {
                        class: "stakes-overview",
                        div {
                            class: "stakes-summary",
                            div {
                                class: "summary-card",
                                div {
                                    class: "summary-label",
                                    "Total Staked"
                                }
                                div {
                                    class: "summary-value",
                                    "{total_staked:.6} SOL"
                                }
                            }
                            div {
                                class: "summary-card",
                                div {
                                    class: "summary-label",
                                    "Stake Accounts"
                                }
                                div {
                                    class: "summary-value",
                                    "{stake_accounts().len()}"
                                }
                            }
                        }

                        if loading_stakes() {
                            div {
                                class: "loading-stakes",
                                "🔍 Scanning for stake accounts..."
                            }
                        } else if stake_accounts().is_empty() {
                            div {
                                class: "no-stakes",
                                div {
                                    class: "no-stakes-icon",
                                    "🏛️"
                                }
                                div {
                                    class: "no-stakes-title",
                                    "No Stake Accounts Found"
                                }
                                div {
                                    class: "no-stakes-description",
                                    "You don't have any active stake accounts yet. Switch to 'Stake SOL' to create your first stake account."
                                }
                            }
                        } else {
                            div {
                                class: "stakes-list",
                                for account in stake_accounts() {
                                    div {
                                        key: "{account.pubkey}",
                                        class: "stake-account-card",
                                        div {
                                            class: "stake-account-header",
                                            div {
                                                class: "stake-account-address",
                                                "{account.pubkey.to_string().chars().take(4).collect::<String>()}...{account.pubkey.to_string().chars().rev().take(4).collect::<String>()}"
                                            }
                                            div {
                                                class: match account.state {
                                                    StakeAccountState::Delegated => "stake-status active",
                                                    StakeAccountState::Initialized => "stake-status initialized", 
                                                    StakeAccountState::Uninitialized => "stake-status uninitialized",
                                                    StakeAccountState::RewardsPool => "stake-status rewards",
                                                },
                                                "{account.state}"
                                            }
                                        }
                                        
                                        div {
                                            class: "stake-account-details",
                                            div {
                                                class: "stake-detail-row",
                                                div {
                                                    class: "stake-detail-label",
                                                    "Validator:"
                                                }
                                                div {
                                                    class: "stake-detail-value",
                                                    "🏛️ {account.validator_name}"
                                                }
                                            }
                                            
                                            div {
                                                class: "stake-detail-row",
                                                div {
                                                    class: "stake-detail-label",
                                                    "Staked Amount:"
                                                }
                                                div {
                                                    class: "stake-detail-value stake-amount",
                                                    "{(account.balance.saturating_sub(account.rent_exempt_reserve) as f64 / 1_000_000_000.0):.6} SOL"
                                                }
                                            }
                                            
                                            if let Some(activation_epoch) = account.activation_epoch {
                                                div {
                                                    class: "stake-detail-row",
                                                    div {
                                                        class: "stake-detail-label",
                                                        "Activation Epoch:"
                                                    }
                                                    div {
                                                        class: "stake-detail-value",
                                                        "{activation_epoch}"
                                                    }
                                                }
                                            }
                                            
                                            if let Some(deactivation_epoch) = account.deactivation_epoch {
                                                div {
                                                    class: "stake-detail-row",
                                                    div {
                                                        class: "stake-detail-label",
                                                        "Deactivation Epoch:"
                                                    }
                                                    div {
                                                        class: "stake-detail-value",
                                                        "{deactivation_epoch}"
                                                    }
                                                }
                                            }
                                        }
                                        
                                        div {
                                            class: "stake-account-actions",
                                            button {
                                                class: "action-button secondary",
                                                onclick: move |_| {
                                                    // Copy address to clipboard (placeholder)
                                                    println!("Copy address: {}", account.pubkey);
                                                },
                                                "📋 Copy Address"
                                            }
                                            
                                            if account.state == StakeAccountState::Delegated {
                                                button {
                                                    class: "action-button danger",
                                                    onclick: move |_| {
                                                        // TODO: Implement unstake functionality
                                                        println!("Unstake: {}", account.pubkey);
                                                    },
                                                    "🔓 Unstake"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                div { 
                    class: "modal-buttons",
                    button {
                        class: "modal-button cancel",
                        onclick: move |_| onclose.call(()),
                        "Close"
                    }
                    
                    if mode() == ModalMode::Stake {
                        button {
                            class: "modal-button primary",
                            disabled: staking() || amount().is_empty() || amount().parse::<f64>().unwrap_or(0.0) < 0.01 || selected_validator().is_none(),
                            onclick: move |_| {
                                error_message.set(None);
                                
                                // Validate amount
                                let stake_amount = match amount().parse::<f64>() {
                                    Ok(amt) if amt >= 0.01 && amt <= current_balance => amt,
                                    _ => {
                                        error_message.set(Some("Please enter a valid amount between 0.01 SOL and your available balance".to_string()));
                                        return;
                                    }
                                };
                            
                                // Validate validator selection
                                let validator = match selected_validator() {
                                    Some(v) => v,
                                    None => {
                                        error_message.set(Some("Please select a validator".to_string()));
                                        return;
                                    }
                                };
                            
                                staking.set(true);

                                // Show hardware approval overlay if using hardware wallet
                                if hardware_wallet.is_some() {
                                    show_hardware_approval.set(true);
                                    was_hardware_transaction.set(true);
                                } else {
                                    was_hardware_transaction.set(false);
                                }
                            
                                
                            
                                let wallet_clone = wallet.clone();
                                let hardware_wallet_clone = hardware_wallet.clone();
                                let custom_rpc_clone = custom_rpc.clone();
                                let validator_vote_account = validator.vote_account.clone();
                            
                                spawn(async move {
                                    match create_stake_account(
                                        wallet_clone.as_ref(),
                                        hardware_wallet_clone,
                                        &validator_vote_account,
                                        stake_amount,
                                        custom_rpc_clone.as_deref(),
                                    ).await {
                                        Ok(stake_info) => {
                                            println!("Successfully created stake account: {:?}", stake_info);
                                            staking.set(false);
                                            show_hardware_approval.set(false);
                                            
                                            // Set success modal data
                                            success_signature.set(stake_info.transaction_signature);
                                            success_amount.set(stake_amount);
                                            success_validator.set(validator.name.clone());
                                            show_success_modal.set(true);
                                        }
                                        Err(e) => {
                                            println!("Staking error: {}", e);
                                            error_message.set(Some(e.to_string()));
                                            staking.set(false);
                                            show_hardware_approval.set(false);
                                        }
                                    }
                                });
                            },
                            if staking() {
                                "Creating Stake Account..."
                            } else {
                                "Stake SOL"
                            }
                        }
                    } else {
                        if !stake_accounts().is_empty() {
                            button {
                                class: "modal-button secondary",
                                disabled: loading_stakes(),
                                onclick: move |_| {
                                    // Manually trigger refresh
                                    loading_stakes.set(true);
                                    error_message.set(None);

                                    let wallet_clone = wallet.clone();
                                    let hardware_wallet_clone = hardware_wallet.clone();
                                    let custom_rpc_clone = custom_rpc.clone();

                                    spawn(async move {
                                        // Get wallet address
                                        let wallet_address = if let Some(hw) = &hardware_wallet_clone {
                                            match hw.get_public_key().await {
                                                Ok(addr) => addr,
                                                Err(e) => {
                                                    error_message.set(Some(format!("Failed to get hardware wallet address: {}", e)));
                                                    loading_stakes.set(false);
                                                    return;
                                                }
                                            }
                                        } else if let Some(w) = &wallet_clone {
                                            w.address.clone()
                                        } else {
                                            error_message.set(Some("No wallet available".to_string()));
                                            loading_stakes.set(false);
                                            return;
                                        };

                                        // Scan for stake accounts
                                        match staking::scan_stake_accounts(&wallet_address, custom_rpc_clone.as_deref()).await {
                                            Ok(accounts) => {
                                                println!("Refreshed: Found {} stake accounts", accounts.len());
                                                stake_accounts.set(accounts);
                                                loading_stakes.set(false);
                                            }
                                            Err(e) => {
                                                println!("Error refreshing stake accounts: {}", e);
                                                error_message.set(Some(format!("Failed to refresh stake accounts: {}", e)));
                                                loading_stakes.set(false);
                                            }
                                        }
                                    });
                                },
                                if loading_stakes() {
                                    "🔄 Refreshing..."
                                } else {
                                    "🔄 Refresh"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}