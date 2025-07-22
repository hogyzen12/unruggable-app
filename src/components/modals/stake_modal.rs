use dioxus::prelude::*;
use crate::wallet::WalletInfo;
use crate::hardware::HardwareWallet;
use crate::validators::{ValidatorInfo, get_recommended_validators};
use crate::rpc;
use crate::staking;
use std::sync::Arc;

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
    let mut amount = use_signal(|| "".to_string());
    let mut selected_validator = use_signal(|| None as Option<ValidatorInfo>);
    let mut show_validator_dropdown = use_signal(|| false);
    let mut staking = use_signal(|| false);
    let mut error_message = use_signal(|| None as Option<String>);
    let mut validators = use_signal(|| Vec::<ValidatorInfo>::new());

    // Load validators on component mount
    use_effect(move || {
        let validator_list = get_recommended_validators();
        // Set default validator (the first one marked as default)
        if let Some(default_validator) = validator_list.iter().find(|v| v.is_default).cloned() {
            selected_validator.set(Some(default_validator));
        }
        validators.set(validator_list);
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

                h2 { 
                    class: "modal-title",
                    "🏛️ Stake SOL"
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
                                        "🏛️ {validator.name}"
                                    }
                                    div {
                                        class: "validator-details",
                                        "{validator.commission}% commission"
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
                                                    "⭐ {validator.name} (Recommended)"
                                                } else {
                                                    "🏛️ {validator.name}"
                                                }
                                            }
                                            div {
                                                class: "validator-commission",
                                                "{validator.commission}%"
                                            }
                                        }
                                        div {
                                            class: "validator-description",
                                            "{validator.description}"
                                        }
                                        if validator.active_stake > 0.0 {
                                            div {
                                                class: "validator-stats",
                                                "Active Stake: {validator.active_stake:.0} SOL • Skip Rate: {validator.skip_rate:.2}%"
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
                        min: "0.1",
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
                        "Minimum stake amount: 0.1 SOL"
                    }
                }

                // Info messages
                div {
                    class: "stake-info-section",
                    div {
                        class: "info-message warning",
                        "⚠️ Staked SOL will take 2-3 days to unstake. Make sure you have enough SOL for transaction fees."
                    }
                    div {
                        class: "info-message",
                        "💡 Staking rewards are typically paid out every epoch (~2-3 days)."
                    }
                    if hardware_wallet.is_some() {
                        div {
                            class: "info-message",
                            "🔐 Your hardware wallet will prompt you to approve the staking transaction."
                        }
                    }
                }

                div { 
                    class: "modal-buttons",
                    button {
                        class: "modal-button cancel",
                        onclick: move |_| onclose.call(()),
                        "Cancel"
                    }
                    button {
                        class: "modal-button primary",
                        disabled: staking() || amount().is_empty() || amount().parse::<f64>().unwrap_or(0.0) < 0.1 || selected_validator().is_none(),
                        onclick: move |_| {
                            error_message.set(None);
                            
                            // Validate amount
                            let stake_amount = match amount().parse::<f64>() {
                                Ok(amt) if amt >= 0.1 && amt <= current_balance => amt,
                                _ => {
                                    error_message.set(Some("Please enter a valid amount between 0.1 SOL and your available balance".to_string()));
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
                        
                            // Import the staking module
                            use crate::staking::create_stake_account;
                        
                            let wallet_clone = wallet.clone();
                            let hardware_wallet_clone = hardware_wallet.clone(); // Remove the () - it's already the value
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
                                        onsuccess.call(stake_info.transaction_signature);
                                    }
                                    Err(e) => {
                                        println!("Staking error: {}", e);
                                        error_message.set(Some(e.to_string()));
                                        staking.set(false);
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
                }
            }
        }
    }
}