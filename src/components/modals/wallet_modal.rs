use dioxus::prelude::*;
use crate::wallet::{Wallet, WalletInfo};
use crate::storage::import_wallet_from_key;

#[component]
pub fn WalletModal(mode: String, onclose: EventHandler<()>, onsave: EventHandler<WalletInfo>) -> Element {
    let mut wallet_name = use_signal(|| "".to_string());
    let mut import_key = use_signal(|| "".to_string());
    let mut show_generated_key = use_signal(|| false);
    let mut generated_wallet = use_signal(|| None as Option<Wallet>);
    let mut error_message = use_signal(|| None as Option<String>);
    
    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| onclose.call(()),
            
            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                
                h2 { class: "modal-title",
                    if mode == "create" { "Create New Wallet" } else { "Import Wallet" }
                }
                
                // Show error if any
                if let Some(error) = error_message() {
                    div {
                        class: "error-message",
                        "{error}"
                    }
                }
                
                if mode == "create" {
                    if let Some(wallet) = generated_wallet() {
                        // Show generated wallet details
                        div {
                            class: "generated-wallet",
                            div { class: "wallet-field",
                                label { "Wallet Name:" }
                                input {
                                    value: "{wallet_name}",
                                    oninput: move |e| wallet_name.set(e.value()),
                                    placeholder: "My Wallet"
                                }
                            }
                            div { class: "wallet-field",
                                label { "Public Address:" }
                                div { class: "address-display", "{wallet.get_public_key()}" }
                            }
                            div { class: "wallet-field",
                                label { "Private Key:" }
                                div { class: "private-key-warning",
                                    "⚠️ Keep this safe! Never share it with anyone!"
                                }
                                if show_generated_key() {
                                    div { class: "private-key-display", 
                                        "{wallet.get_private_key()}"
                                    }
                                    div { 
                                        class: "key-format-info",
                                        "Solana Keypair (64 bytes) - Compatible with Solana CLI and other wallets"
                                    }
                                    
                                    // Optionally show just the private key too
                                    div { 
                                        class: "private-key-section",
                                        label { "Private Key Only (32 bytes):" }
                                        div { class: "private-key-display", 
                                            "{wallet.get_private_key_only()}"
                                        }
                                    }
                                    div { 
                                        class: "copy-hint",
                                        "Make sure to copy this key before saving!"
                                    }
                                } else {
                                    button {
                                        class: "show-key-button",
                                        onclick: move |_| show_generated_key.set(true),
                                        "Show Private Key"
                                    }
                                }
                            }
                        }
                    } else {
                        div {
                            class: "wallet-field",
                            label { "Wallet Name:" }
                            input {
                                value: "{wallet_name}",
                                oninput: move |e| wallet_name.set(e.value()),
                                placeholder: "My Wallet"
                            }
                        }
                        div {
                            class: "info-message",
                            "Click 'Generate Wallet' to create a new wallet"
                        }
                    }
                } else {
                    // Import mode
                    div {
                        class: "wallet-field",
                        label { "Wallet Name:" }
                        input {
                            value: "{wallet_name}",
                            oninput: move |e| wallet_name.set(e.value()),
                            placeholder: "Imported Wallet"
                        }
                    }
                    div {
                        class: "wallet-field",
                        label { "Private Key:" }
                        textarea {
                            value: "{import_key}",
                            oninput: move |e| import_key.set(e.value()),
                            placeholder: "Enter your base58 encoded private key or Solana keypair"
                        }
                        div {
                            class: "help-text",
                            "Supports both 32-byte private keys and 64-byte Solana keypairs"
                        }
                    }
                }
                
                div { class: "modal-buttons",
                    button {
                        class: "modal-button cancel",
                        onclick: move |_| onclose.call(()),
                        "Cancel"
                    }
                    if mode == "create" {
                        if generated_wallet().is_none() {
                            button {
                                class: "modal-button primary",
                                onclick: move |_| {
                                    let new_wallet = Wallet::new(
                                        if wallet_name().is_empty() { 
                                            "New Wallet".to_string() 
                                        } else { 
                                            wallet_name() 
                                        }
                                    );
                                    generated_wallet.set(Some(new_wallet));
                                },
                                "Generate Wallet"
                            }
                        } else {
                            button {
                                class: "modal-button primary",
                                onclick: move |_| {
                                    if let Some(wallet) = generated_wallet() {
                                        let mut wallet_info = wallet.to_wallet_info();
                                        wallet_info.name = if wallet_name().is_empty() {
                                            wallet.name.clone()
                                        } else {
                                            wallet_name()
                                        };
                                        onsave.call(wallet_info);
                                    }
                                },
                                disabled: !show_generated_key(),
                                if !show_generated_key() {
                                    "Show Private Key First"
                                } else {
                                    "Save Wallet"
                                }
                            }
                        }
                    } else {
                        button {
                            class: "modal-button primary",
                            onclick: move |_| {
                                if !import_key().is_empty() {
                                    match import_wallet_from_key(&import_key(), wallet_name()) {
                                        Ok(wallet_info) => onsave.call(wallet_info),
                                        Err(e) => {
                                            error_message.set(Some(e));
                                        }
                                    }
                                } else {
                                    error_message.set(Some("Please enter a private key".to_string()));
                                }
                            },
                            "Import"
                        }
                    }
                }
            }
        }
    }
}