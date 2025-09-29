// UPDATED: src/components/modals/export_wallet_modal.rs
// Replace the class name to match your existing modals

use dioxus::prelude::*;
use crate::wallet::WalletInfo;

#[component]
pub fn ExportWalletModal(
    wallet: Option<WalletInfo>, 
    onclose: EventHandler<()>
) -> Element {
    let mut show_private_key = use_signal(|| false);
    
    rsx! {
        div { class: "modal-backdrop",  // CHANGED: from "modal-overlay" to "modal-backdrop"
            onclick: move |_| onclose.call(()),
            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                
                div { class: "modal-header",
                    h2 { class: "modal-title", "Export Wallet" }  // ADDED: modal-title class
                    button {
                        class: "modal-close",
                        onclick: move |_| onclose.call(()),
                        "×"
                    }
                }
                
                div { class: "modal-body",
                    if let Some(wallet_info) = wallet {
                        div {
                            div { class: "wallet-field",
                                label { "Wallet Name:" }
                                div { class: "wallet-name-display", "{wallet_info.name}" }
                            }
                            
                            div { class: "wallet-field",
                                label { "Wallet Address:" }
                                div { class: "wallet-address-display", "{wallet_info.address}" }
                            }
                            
                            div { class: "wallet-field",
                                label { "Private Key:" }
                                if !show_private_key() {
                                    div { class: "warning-message",
                                        "⚠️ Your private key gives full access to your wallet. Never share it with anyone!"
                                    }
                                    button {
                                        class: "show-key-button",
                                        onclick: move |_| show_private_key.set(true),
                                        "Show Private Key"
                                    }
                                } else {
                                    div { class: "private-key-display", 
                                        "{wallet_info.encrypted_key}"
                                    }
                                    div { 
                                        class: "key-format-info",
                                        "Base58 encoded Solana keypair (64 bytes) - Compatible with Solana CLI and other wallets"
                                    }
                                    div { 
                                        class: "copy-hint",
                                        "Make sure to copy this key to a secure location!"
                                    }
                                }
                            }
                        }
                    } else {
                        div { class: "error-message", "No wallet selected" }
                    }
                }
                
                div { class: "modal-buttons",
                    button {
                        class: "modal-button cancel",
                        onclick: move |_| onclose.call(()),
                        "Close"
                    }
                }
            }
        }
    }
}