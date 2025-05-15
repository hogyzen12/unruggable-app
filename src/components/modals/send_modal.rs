use dioxus::prelude::*;
use crate::wallet::{Wallet, WalletInfo};
use crate::hardware::HardwareWallet;
use crate::transaction::TransactionClient;
use crate::signing::hardware::HardwareSigner;
use crate::rpc;
use std::sync::Arc;

#[component]
pub fn SendModalWithHardware(
    wallet: Option<WalletInfo>,
    hardware_wallet: Option<Arc<HardwareWallet>>,
    current_balance: f64,
    custom_rpc: Option<String>,
    onclose: EventHandler<()>,
    onsuccess: EventHandler<String>,
) -> Element {
    let mut recipient = use_signal(|| "".to_string());
    let mut amount = use_signal(|| "".to_string());
    let mut sending = use_signal(|| false);
    let mut error_message = use_signal(|| None as Option<String>);
    let mut recipient_balance = use_signal(|| None as Option<f64>);
    let mut checking_balance = use_signal(|| false);

    // Clone custom_rpc for use in use_effect to prevent moving the original
    let custom_rpc_for_effect = custom_rpc.clone();

    // Determine which address to show based on wallet type
    let display_address = if let Some(hw) = &hardware_wallet {
        // If hardware wallet is connected, try to get its public key
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

    // Check recipient balance when address changes
    use_effect(move || {
        let recipient_addr = recipient();
        let rpc_url = custom_rpc_for_effect.clone(); // Use the cloned version here

        if recipient_addr.len() > 30 { // Basic check if it could be a valid address
            // Validate the address format
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

    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| onclose.call(()),

            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),

                h2 { class: "modal-title",
                    if hardware_wallet.is_some() {
                        "Send SOL (Hardware Wallet)"
                    } else {
                        "Send SOL"
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
                    label { "Available Balance:" }
                    div { class: "balance-display", "{current_balance:.4} SOL" }
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
                            "Balance: {balance:.4} SOL"
                        }
                    }
                }

                div {
                    class: "wallet-field",
                    label { "Amount (SOL):" }
                    input {
                        r#type: "number",
                        value: "{amount}",
                        oninput: move |e| amount.set(e.value()),
                        placeholder: "0.0",
                        step: "0.0001",
                        min: "0"
                    }
                }

                if hardware_wallet.is_some() {
                    div {
                        class: "info-message",
                        "Your hardware wallet will prompt you to approve the transaction"
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

                            let hardware_wallet = hardware_wallet.clone();
                            let wallet_info = wallet.clone();
                            let recipient_address = recipient();
                            let amount_str = amount();
                            let rpc_url = custom_rpc.clone(); // Original custom_rpc is still available here

                            spawn(async move {
                                // Validate inputs
                                let amount_value = match amount_str.parse::<f64>() {
                                    Ok(amt) if amt > 0.0 => amt,
                                    _ => {
                                        error_message.set(Some("Invalid amount".to_string()));
                                        sending.set(false);
                                        return;
                                    }
                                };

                                if amount_value > current_balance {
                                    error_message.set(Some("Insufficient balance".to_string()));
                                    sending.set(false);
                                    return;
                                }

                                // Validate recipient address
                                if let Err(e) = bs58::decode(&recipient_address).into_vec() {
                                    error_message.set(Some(format!("Invalid recipient address: {}", e)));
                                    sending.set(false);
                                    return;
                                }

                                let client = TransactionClient::new(rpc_url.as_deref());

                                // Use hardware wallet if available, otherwise use software wallet
                                if let Some(hw) = hardware_wallet {
                                    let hw_signer = HardwareSigner::from_wallet(hw);
                                    match client.send_sol_with_signer(&hw_signer, &recipient_address, amount_value).await {
                                        Ok(signature) => {
                                            println!("Transaction sent with hardware wallet: {}", signature);
                                            onsuccess.call(signature);
                                        }
                                        Err(e) => {
                                            error_message.set(Some(format!("Transaction failed: {}", e)));
                                        }
                                    }
                                } else if let Some(wallet_info) = wallet_info {
                                    // Load wallet from wallet info
                                    match Wallet::from_wallet_info(&wallet_info) {
                                        Ok(wallet) => {
                                            // Send transaction with amount in SOL
                                            match client.send_sol(&wallet, &recipient_address, amount_value).await {
                                                Ok(signature) => {
                                                    println!("Transaction sent: {}", signature);
                                                    onsuccess.call(signature);
                                                }
                                                Err(e) => {
                                                    error_message.set(Some(format!("Transaction failed: {}", e)));
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            error_message.set(Some(format!("Failed to load wallet: {}", e)));
                                        }
                                    }
                                } else {
                                    error_message.set(Some("No wallet available".to_string()));
                                }

                                sending.set(false);
                            });
                        },
                        disabled: sending() || recipient().is_empty() || amount().is_empty(),
                        if sending() { "Sending..." } else { "Send" }
                    }
                }
            }
        }
    }
}