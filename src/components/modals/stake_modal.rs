use dioxus::prelude::*;
use crate::wallet::WalletInfo;
use crate::hardware::HardwareWallet;
use crate::validators::{ValidatorInfo, get_recommended_validators};
use crate::staking::{self, DetailedStakeAccount, StakeAccountState};
use crate::staking::{MergeGroup, MergeType};
use crate::unstaking::{instant_unstake_stake_account, can_instant_unstake, normal_unstake_stake_account, can_normal_unstake};
use std::sync::Arc;
use std::collections::HashMap;
use crate::signing::hardware::HardwareSigner;
use crate::staking::create_stake_account;
use crate::staking::find_mergeable_stake_accounts;
use std::sync::LazyLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(PartialEq, Clone, Debug)]
enum ModalMode {
    Stake,
    MyStakes,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorEntry {
    pub account: String,
    pub keybase_name: String,
    pub keybase_id: Option<String>,
    pub keybase_www_url: Option<String>,
    pub keybase_details: Option<String>,
    pub keybase_avatar_url: Option<String>,
    pub updated_at: String,
    pub vote_account: String,
    pub autonomous_system_number: u64,
    pub data_center: String,
    pub data_center_host: Option<String>,
}

// Embed the local JSON file at compile time (mobile-safe)
static VALIDATORS_JSON: &str = include_str!("../../../assets/validators.json");

// Parse JSON only once when first accessed - mobile-friendly!
static VALIDATOR_METADATA: LazyLock<HashMap<String, ValidatorEntry>> = LazyLock::new(|| {
    parse_validators_from_json(VALIDATORS_JSON)
});

/// Parse validators from JSON string with robust handling
fn parse_validators_from_json(json_str: &str) -> HashMap<String, ValidatorEntry> {
    let mut map = HashMap::new();

    match serde_json::from_str::<Value>(json_str) {
        Ok(value) => {
            let entries: Vec<ValidatorEntry> = match value {
                Value::Array(arr) => {
                    arr.into_iter()
                        .filter_map(|v| serde_json::from_value(v).ok())
                        .collect()
                }
                Value::Object(obj) => {
                    if let Ok(entry) = serde_json::from_value(Value::Object(obj)) {
                        vec![entry]
                    } else {
                        vec![]
                    }
                }
                _ => vec![],
            };

            for entry in entries {
                map.insert(entry.vote_account.clone(), entry);
            }
            println!("Successfully loaded {} validators from local JSON", map.len());
        }
        Err(e) => {
            eprintln!("Failed to parse validators JSON: {}", e);
        }
    }

    map
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
    let mut merge_groups = use_signal(|| Vec::<MergeGroup>::new());
    let mut merging = use_signal(|| false);

    let instant_unstaking = use_signal(|| false);
    let normal_unstaking = use_signal(|| false);

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
    use_effect(move || {
        let current_mode = mode();
        println!("🔍 DEBUG: use_effect triggered with mode: {:?}", current_mode);
        
        if current_mode == ModalMode::MyStakes {
            // Check if we're already loading or already have data
            if loading_stakes() {
                println!("⏳ DEBUG: Already loading, skipping...");
                return;
            }
            
            if !stake_accounts().is_empty() {
                println!("📊 DEBUG: Already have {} accounts, skipping...", stake_accounts().len());
                return;
            }
            
            println!("🚀 DEBUG: Starting stake scan...");
            loading_stakes.set(true);
            error_message.set(None);

            let wallet_clone = wallet_for_effect.clone();
            let hardware_wallet_clone = hardware_wallet_for_effect.clone();
            let custom_rpc_clone = custom_rpc_for_effect.clone();

            spawn(async move {
                println!("📡 DEBUG: In async block");
                
                // Get wallet address
                let wallet_address = if let Some(hw) = &hardware_wallet_clone {
                    match hw.get_public_key().await {
                        Ok(addr) => {
                            println!("✅ DEBUG: HW wallet address: {}", addr);
                            addr
                        }
                        Err(e) => {
                            println!("❌ DEBUG: HW wallet error: {}", e);
                            error_message.set(Some(format!("Failed to get hardware wallet address: {}", e)));
                            loading_stakes.set(false);
                            return;
                        }
                    }
                } else if let Some(w) = &wallet_clone {
                    println!("💼 DEBUG: SW wallet address: {}", w.address);
                    w.address.clone()
                } else {
                    println!("❌ DEBUG: No wallet");
                    error_message.set(Some("No wallet available".to_string()));
                    loading_stakes.set(false);
                    return;
                };

                println!("🔍 DEBUG: Calling scan_stake_accounts...");

                // Scan for stake accounts
                match staking::scan_stake_accounts(&wallet_address, custom_rpc_clone.as_deref()).await {
                    Ok(accounts) => {
                        println!("✅ DEBUG: Successfully got {} accounts - setting in UI", accounts.len());
                        stake_accounts.set(accounts);
                        loading_stakes.set(false);
                    }
                    Err(e) => {
                        println!("❌ DEBUG: Scan error: {}", e);
                        error_message.set(Some(format!("Failed to load stake accounts: {}", e)));
                        loading_stakes.set(false);
                    }
                }
            });
        } else {
            println!("ℹ️ DEBUG: Mode is not MyStakes, current mode: {:?}", current_mode);
        }
    });

    use_effect(move || {
        let accounts = stake_accounts();
        if !accounts.is_empty() {
            println!("🔍 DEBUG: Calculating merge opportunities for {} accounts", accounts.len());
            // Use current epoch 835 for now (from your logs)
            let current_epoch = 835;
            let groups = find_mergeable_stake_accounts(&accounts, current_epoch);
            println!("🔗 DEBUG: Found {} merge groups", groups.len());
            merge_groups.set(groups);
        } else {
            merge_groups.set(Vec::new());
        }
    });

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
                                println!("🔘 BUTTON CLICKED: My Staked Sol");
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

                div {
                    class: "modal-body",

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
                        // My Stakes interface with modern UI
                        div {
                            class: "stakes-overview-modern",
                            
                            // Compact summary header (replaces the old stakes-summary grid)
                            div {
                                class: "stakes-summary-compact",
                                span { class: "summary-label", "TOTAL STAKED" }
                                span { class: "summary-value", "{total_staked:.6} SOL" }
                                span { class: "summary-label", "STAKE ACCOUNTS" }
                                span { class: "summary-count", "{stake_accounts().len()}" }
                            }

                            // Merge info (only show if merges available)
                            if !merge_groups().is_empty() {
                                div {
                                    class: "merge-simple-section",
                                    div {
                                        class: "merge-simple-info",
                                        "🔗 Found {merge_groups().len()} merge opportunities to consolidate your stake accounts"
                                    }
                                }
                            }

                            // Loading state (preserved)
                            if loading_stakes() {
                                div {
                                    class: "loading-stakes-modern",
                                    div { class: "loading-spinner" }
                                    "🔍 Scanning for stake accounts..."
                                }
                            } 
                            // Empty state (preserved but modernized)
                            else if stake_accounts().is_empty() {
                                div {
                                    class: "no-stakes-modern",
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
                            } 
                            // Modern stake accounts list
                            else {
                                div {
                                    class: "stakes-list-modern",
                                    
                                    for account in stake_accounts() {
                                        div {
                                            key: "{account.pubkey}",
                                            class: "stake-account-modern",
                                            
                                            // Account header with validator info
                                            div {
                                                class: "stake-account-header-modern",
                                                
                                                // Validator logo and info
                                                div {
                                                    class: "validator-info-modern",
                                                    div {
                                                        class: "validator-logo-modern",
                                                        img {
                                                            src: {
                                                                let metadata_map = VALIDATOR_METADATA.clone();
                                                                println!("🔍 DEBUG: Looking for validator in account.validator_name: '{}'", account.validator_name);
                                                                // Removed the available keys println
                                                                
                                                                // Clean and extract potential vote account
                                                                let cleaned_name = account.validator_name.trim_start_matches("Validator ").trim().to_string();
                                                                let potential_vote_account = cleaned_name.chars().filter(|c| c.is_alphanumeric()).collect::<String>();
                                                                println!("🔍 DEBUG: Extracted potential vote account: '{}'", potential_vote_account);
                                                                
                                                                // Find match
                                                                let validator_match = metadata_map.get(&potential_vote_account)
                                                                    .or_else(|| metadata_map.values().find(|v| {
                                                                        let matches = v.vote_account.starts_with(&potential_vote_account) ||
                                                                                    v.vote_account.contains(&potential_vote_account) ||
                                                                                    v.keybase_name.to_lowercase().contains(&cleaned_name.to_lowercase()) ||
                                                                                    cleaned_name.to_lowercase().contains(&v.keybase_name.to_lowercase());
                                                                        if matches {
                                                                            println!("✅ Found match for validator: {}", v.keybase_name);
                                                                        }
                                                                        matches
                                                                    }));
                                                                
                                                                let validator_logo = validator_match
                                                                    .and_then(|v| {
                                                                        println!("🖼️ Logo URL for {}: {:?}", v.keybase_name, v.keybase_avatar_url);
                                                                        v.keybase_avatar_url.clone()
                                                                    })
                                                                    .unwrap_or_else(|| {
                                                                        println!("❌ No logo found for validator: {}", account.validator_name);
                                                                        "data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' width='32' height='32' viewBox='0 0 32 32'><rect width='32' height='32' rx='8' fill='%23374151'/><text x='16' y='20' text-anchor='middle' fill='white' font-family='monospace' font-size='12'>V</text></svg>".to_string()
                                                                    });
                                                                validator_logo
                                                            },
                                                            alt: "Validator Logo"
                                                        }
                                                    }
                                                    div {
                                                        class: "validator-details-modern",
                                                        div {
                                                            class: "validator-name-modern",
                                                            {
                                                                let metadata_map = VALIDATOR_METADATA.clone();
                                                                let cleaned_name = account.validator_name.trim_start_matches("Validator ").trim().to_string();
                                                                let potential_vote_account = cleaned_name.chars().filter(|c| c.is_alphanumeric()).collect::<String>();
                                                                
                                                                // Find match (same as above)
                                                                let validator_match = metadata_map.get(&potential_vote_account)
                                                                    .or_else(|| metadata_map.values().find(|v| {
                                                                        v.vote_account.starts_with(&potential_vote_account) ||
                                                                        v.vote_account.contains(&potential_vote_account) ||
                                                                        v.keybase_name.to_lowercase().contains(&cleaned_name.to_lowercase()) ||
                                                                        cleaned_name.to_lowercase().contains(&v.keybase_name.to_lowercase())
                                                                    }));
                                                                
                                                                validator_match
                                                                    .map(|v| v.keybase_name.clone())
                                                                    .unwrap_or_else(|| {
                                                                        if !account.validator_name.is_empty() {
                                                                            let pubkey = account.validator_name.trim_start_matches("Validator ").trim();
                                                                            format!("Validator {}...{}", &pubkey[0..4], &pubkey[pubkey.len()-4..])
                                                                        } else {
                                                                            "Unknown Validator".to_string()
                                                                        }
                                                                    })
                                                            }
                                                        }
                                                        div {
                                                            class: "validator-address-modern",
                                                            {
                                                                let pubkey_str = account.pubkey.to_string();
                                                                format!("{}...{}", &pubkey_str[..4], &pubkey_str[pubkey_str.len()-4..])
                                                            }
                                                        }
                                                    }
                                                }
                                                
                                                // Status badge
                                                div {
                                                    class: match account.state {
                                                        StakeAccountState::Delegated => "status-badge active",
                                                        StakeAccountState::Initialized => "status-badge activating", 
                                                        StakeAccountState::Uninitialized => "status-badge inactive",
                                                        StakeAccountState::RewardsPool => "status-badge rewards",
                                                    },
                                                    match account.state {
                                                        StakeAccountState::Delegated => "ACTIVE",
                                                        StakeAccountState::Initialized => "ACTIVATING",
                                                        StakeAccountState::Uninitialized => "INACTIVE", 
                                                        StakeAccountState::RewardsPool => "REWARDS",
                                                    }
                                                }
                                            }
                                            
                                            // Simplified staked amount (no label, rounded to 2 decimals)
                                            div {
                                                class: "stake-account-details-modern",
                                                span {
                                                    class: "detail-value stake-amount",
                                                    "{(account.balance.saturating_sub(account.rent_exempt_reserve) as f64 / 1_000_000_000.0):.2} SOL"
                                                }
                                            }
                                            
                                            // Action buttons: Change copy to instant unstake (placeholder), keep normal unstake
                                            div {
                                                class: "stake-actions-modern",
                                                if account.state == StakeAccountState::Delegated {
                                                    button {
                                                        class: "action-btn secondary",
                                                        disabled: instant_unstaking() || !can_instant_unstake(&account),
                                                        onclick: {
                                                            // Clone all necessary values for the async block
                                                            let account_clone = account.clone();
                                                            let wallet_for_instant = wallet.clone();
                                                            let hardware_wallet_for_instant = hardware_wallet.clone();
                                                            let custom_rpc_for_instant = custom_rpc.clone();
                                                            
                                                            // Clone mutable signals
                                                            let mut instant_unstaking_clone = instant_unstaking.clone();
                                                            let mut error_message_clone = error_message.clone();
                                                            let mut show_hardware_approval_clone = show_hardware_approval.clone();
                                                            let mut stake_accounts_clone = stake_accounts.clone();
                                                            
                                                            move |_| {
                                                                let stake_balance_sol = (account_clone.balance.saturating_sub(account_clone.rent_exempt_reserve)) as f64 / 1_000_000_000.0;
                                                                println!("INSTANT UNSTAKE: Starting for account {} ({:.6} SOL)", 
                                                                    account_clone.pubkey, stake_balance_sol);
                                                                
                                                                instant_unstaking_clone.set(true);
                                                                error_message_clone.set(None);
                                                                
                                                                // Show hardware approval overlay if using hardware wallet
                                                                if hardware_wallet_for_instant.is_some() {
                                                                    show_hardware_approval_clone.set(true);
                                                                }
                                                                
                                                                // Clone for async block
                                                                let wallet_clone = wallet_for_instant.clone();
                                                                let hardware_wallet_clone = hardware_wallet_for_instant.clone();
                                                                let custom_rpc_clone = custom_rpc_for_instant.clone();
                                                                let account_async = account_clone.clone();
                                                                
                                                                spawn(async move {
                                                                    println!("INSTANT UNSTAKE: Executing transaction...");
                                                                    
                                                                    match instant_unstake_stake_account(
                                                                        &account_async,
                                                                        wallet_clone.as_ref(),
                                                                        hardware_wallet_clone,
                                                                        custom_rpc_clone.as_deref(),
                                                                    ).await {
                                                                        Ok(signature) => {
                                                                            println!("✅ Instant unstake completed: {}", signature);
                                                                            
                                                                            // Hide hardware approval overlay
                                                                            show_hardware_approval_clone.set(false);
                                                                            
                                                                            // Clear stake accounts to trigger refresh
                                                                            stake_accounts_clone.set(Vec::new());
                                                                            
                                                                            // Show success message
                                                                            error_message_clone.set(Some(format!(
                                                                                "✅ Instant unstake successful! Stake account transferred to pool and deactivated. Transaction: {}", 
                                                                                signature
                                                                            )));
                                                                            
                                                                            // Clear success message after 8 seconds
                                                                            let mut error_message_clear = error_message_clone.clone();
                                                                            spawn(async move {
                                                                                tokio::time::sleep(std::time::Duration::from_millis(8_000)).await;
                                                                                error_message_clear.set(None);
                                                                            });
                                                                        }
                                                                        Err(e) => {
                                                                            println!("❌ Instant unstake error: {}", e);
                                                                            error_message_clone.set(Some(format!("Instant unstake failed: {}", e)));
                                                                            show_hardware_approval_clone.set(false);
                                                                        }
                                                                    }
                                                                    
                                                                    instant_unstaking_clone.set(false);
                                                                });
                                                            }
                                                        },
                                                        if instant_unstaking() {
                                                            "⏳"
                                                        } else {
                                                            "⚡"
                                                        }
                                                    }
                                                    
                                                    button {
                                                        class: "action-btn primary",
                                                        disabled: normal_unstaking() || instant_unstaking() || !can_normal_unstake(&account),
                                                        onclick: {
                                                            // Clone all necessary values for the async block (matching instant unstake pattern)
                                                            let account_clone = account.clone();
                                                            let wallet_for_normal = wallet.clone();
                                                            let hardware_wallet_for_normal = hardware_wallet.clone();
                                                            let custom_rpc_for_normal = custom_rpc.clone();
                                                            
                                                            // Clone mutable signals
                                                            let mut normal_unstaking_clone = normal_unstaking.clone();
                                                            let mut error_message_clone = error_message.clone();
                                                            let mut show_hardware_approval_clone = show_hardware_approval.clone();
                                                            let mut stake_accounts_clone = stake_accounts.clone();
                                                            
                                                            move |_| {
                                                                let stake_balance_sol = (account_clone.balance.saturating_sub(account_clone.rent_exempt_reserve)) as f64 / 1_000_000_000.0;
                                                                println!("NORMAL UNSTAKE: Starting for account {} ({:.6} SOL)", 
                                                                    account_clone.pubkey, stake_balance_sol);
                                                                
                                                                normal_unstaking_clone.set(true);
                                                                error_message_clone.set(None);
                                                                
                                                                // Show hardware approval overlay if using hardware wallet
                                                                if hardware_wallet_for_normal.is_some() {
                                                                    show_hardware_approval_clone.set(true);
                                                                }
                                                                
                                                                // Clone for async block
                                                                let wallet_clone = wallet_for_normal.clone();
                                                                let hardware_wallet_clone = hardware_wallet_for_normal.clone();
                                                                let custom_rpc_clone = custom_rpc_for_normal.clone();
                                                                let account_async = account_clone.clone();
                                                                
                                                                spawn(async move {
                                                                    println!("NORMAL UNSTAKE: Executing deactivate transaction...");
                                                                    
                                                                    match normal_unstake_stake_account(
                                                                        &account_async,
                                                                        wallet_clone.as_ref(),
                                                                        hardware_wallet_clone,
                                                                        custom_rpc_clone.as_deref(),
                                                                    ).await {
                                                                        Ok(signature) => {
                                                                            println!("✅ Normal unstake completed: {}", signature);
                                                                            
                                                                            // Hide hardware approval overlay
                                                                            show_hardware_approval_clone.set(false);
                                                                            
                                                                            // Clear stake accounts to trigger refresh
                                                                            stake_accounts_clone.set(Vec::new());
                                                                            
                                                                            // Show success message
                                                                            error_message_clone.set(Some(format!(
                                                                                "✅ Normal unstake successful! Stake account has been deactivated and will be available for withdrawal after the cooldown period. Transaction: {}", 
                                                                                signature
                                                                            )));
                                                                            
                                                                            // Clear success message after 8 seconds
                                                                            let mut error_message_clear = error_message_clone.clone();
                                                                            spawn(async move {
                                                                                tokio::time::sleep(std::time::Duration::from_millis(8_000)).await;
                                                                                error_message_clear.set(None);
                                                                            });
                                                                        }
                                                                        Err(e) => {
                                                                            println!("❌ Normal unstake error: {}", e);
                                                                            error_message_clone.set(Some(format!("Normal unstake failed: {}", e)));
                                                                            show_hardware_approval_clone.set(false);
                                                                        }
                                                                    }
                                                                    
                                                                    normal_unstaking_clone.set(false);
                                                                });
                                                            }
                                                        },
                                                        if normal_unstaking() {
                                                            "⏳ Deactivating..."
                                                        } else {
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
                                onclick: {
                                    // Clone props outside the closure to avoid move issues
                                    let wallet_for_refresh = wallet.clone();
                                    let hardware_wallet_for_refresh = hardware_wallet.clone();
                                    let custom_rpc_for_refresh = custom_rpc.clone();
                                    
                                    move |_| {
                                        // Manually trigger refresh
                                        loading_stakes.set(true);
                                        error_message.set(None);
                        
                                        let wallet_clone = wallet_for_refresh.clone();
                                        let hardware_wallet_clone = hardware_wallet_for_refresh.clone();
                                        let custom_rpc_clone = custom_rpc_for_refresh.clone();
                        
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
                                                    println!("✅ Refreshed: Found {} stake accounts", accounts.len());
                                                    stake_accounts.set(accounts);
                                                    loading_stakes.set(false);
                                                }
                                                Err(e) => {
                                                    println!("❌ Refresh error: {}", e);
                                                    error_message.set(Some(format!("Failed to refresh: {}", e)));
                                                    loading_stakes.set(false);
                                                }
                                            }
                                        });
                                    }
                                },
                                if loading_stakes() {
                                    "🔄 Refreshing..."
                                } else {
                                    "🔄 Refresh"
                                }
                            }
                        }
                        if mode() == ModalMode::MyStakes && !merge_groups().is_empty() {
                            button {
                                class: "modal-button merge-simple",
                                disabled: merging(),
                                onclick: {
                                    // Clone props outside the closure to avoid move issues
                                    let wallet_for_merge = wallet.clone();
                                    let hardware_wallet_for_merge = hardware_wallet.clone();
                                    let custom_rpc_for_merge = custom_rpc.clone();
                                    
                                    move |_| {
                                        println!("🔗 DEBUG: Merge button clicked!");
                                        println!("🔗 DEBUG: Available merge groups: {}", merge_groups().len());
                                        
                                        for (i, group) in merge_groups().iter().enumerate() {
                                            println!("  Group {}: {} - {} accounts, {:.6} SOL", 
                                                i + 1, 
                                                group.merge_type, 
                                                group.accounts.len(),
                                                group.total_amount as f64 / 1_000_000_000.0
                                            );
                                        }
                                        
                                        merging.set(true);
                                        
                                        // Clone for the async block
                                        let wallet_clone = wallet_for_merge.clone();
                                        let hardware_wallet_clone = hardware_wallet_for_merge.clone();
                                        let custom_rpc_clone = custom_rpc_for_merge.clone();
                                        let merge_groups_clone = merge_groups();
                                        
                                        // Clone signals that need to be mutable
                                        let mut merging_clone = merging.clone();
                                        let mut stake_accounts_clone = stake_accounts.clone();
                                        let mut error_message_clone = error_message.clone();
                                        let mut show_hardware_approval_clone = show_hardware_approval.clone();
                                        
                                        // Show hardware approval overlay if using hardware wallet
                                        if hardware_wallet_for_merge.is_some() {
                                            show_hardware_approval.set(true);
                                        }
                                        
                                        spawn(async move {
                                            // Get the first merge group for now (simplest implementation)
                                            if let Some(first_group) = merge_groups_clone.first() {
                                                println!("🔗 Processing merge group with {} accounts", first_group.accounts.len());
                                                
                                                match staking::merge_stake_accounts(
                                                    first_group,
                                                    wallet_clone.as_ref(),
                                                    hardware_wallet_clone,
                                                    custom_rpc_clone.as_deref(),
                                                ).await {
                                                    Ok(signature) => {
                                                        println!("✅ Merge completed: {}", signature);
                                                        
                                                        // Hide hardware approval overlay if it was shown
                                                        show_hardware_approval_clone.set(false);
                                                        
                                                        // Clear stake accounts to trigger refresh on next scan
                                                        stake_accounts_clone.set(Vec::new());
                                                        
                                                        // Show success message
                                                        error_message_clone.set(Some(format!(
                                                            "✅ Successfully merged {} accounts! Transaction: {}", 
                                                            first_group.accounts.len(), 
                                                            signature
                                                        )));
                                                        
                                                        // Clear the message after 5 seconds
                                                        let mut error_message_clear = error_message_clone.clone();
                                                        spawn(async move {
                                                            tokio::time::sleep(std::time::Duration::from_millis(5_000)).await;
                                                            error_message_clear.set(None);
                                                        });
                                                    }
                                                    Err(e) => {
                                                        println!("❌ Merge failed: {}", e);
                                                        
                                                        // Hide hardware approval overlay if it was shown
                                                        show_hardware_approval_clone.set(false);
                                                        
                                                        error_message_clone.set(Some(format!("❌ Merge failed: {}", e)));
                                                        
                                                        // Clear error message after 10 seconds
                                                        let mut error_message_clear = error_message_clone.clone();
                                                        spawn(async move {
                                                            tokio::time::sleep(std::time::Duration::from_millis(10_000)).await;
                                                            error_message_clear.set(None);
                                                        });
                                                    }
                                                }
                                            } else {
                                                println!("❌ No merge groups available");
                                                error_message_clone.set(Some("❌ No merge opportunities found".to_string()));
                                            }
                                            
                                            merging_clone.set(false);
                                        });
                                    }
                                },
                                if merging() {
                                    "🔄 Merging..."
                                } else {
                                    "🔗 Merge Stake Accounts ({merge_groups().len()})"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}