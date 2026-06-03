use crate::components::pin_input::PinInput;
use crate::storage;
use crate::wallet::{Wallet, WalletInfo};
use crate::AppSecurityContext;
use dioxus::prelude::*;

#[component]
pub fn ExportWalletModal(wallet: Option<WalletInfo>, onclose: EventHandler<()>) -> Element {
    let app_security = use_context::<AppSecurityContext>();
    let mut show_private_key = use_signal(|| !storage::has_pin());
    let mut pin_error = use_signal(|| None as Option<String>);
    let mut verifying_pin = use_signal(|| false);
    let exported_private_key = wallet
        .as_ref()
        .and_then(|wallet_info| Wallet::from_wallet_info(wallet_info).ok())
        .map(|wallet| wallet.get_private_key());

    let handle_pin_input = EventHandler::new(move |_| {
        app_security.record_activity();
        if pin_error().is_some() {
            pin_error.set(None);
        }
    });

    let handle_pin_complete = move |pin: String| {
        if verifying_pin() {
            return;
        }

        verifying_pin.set(true);
        pin_error.set(None);

        let app_security = app_security;
        spawn(async move {
            match storage::verify_pin_for_sensitive_action_async(pin).await {
                Ok(()) => {
                    verifying_pin.set(false);
                    pin_error.set(None);
                    app_security.record_activity();
                    show_private_key.set(true);
                }
                Err(e) => {
                    verifying_pin.set(false);
                    pin_error.set(Some(e));
                    if storage::is_pin_locked() {
                        app_security.lock_now("export auth locked");
                    }
                }
            }
        });
    };

    rsx! {
        div { class: "modal-backdrop",
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
                                        "⚠️ Your private key gives full access to your wallet. Enter your PIN again to reveal it and never share it with anyone."
                                    }

                                    div {
                                        class: "onboarding-pin-shell export-wallet-auth-shell",

                                        div {
                                            class: "flow-note-card flow-note-card-accent export-wallet-note-card",
                                            div {
                                                class: "flow-note-label",
                                                "Verification required"
                                            }
                                            p {
                                                class: "flow-note-copy",
                                                "Confirm your 6-digit PIN again before the raw private key is shown."
                                            }
                                        }

                                        PinInput {
                                            title: "Confirm PIN".to_string(),
                                            subtitle: Some("Enter your PIN again to reveal the private key.".to_string()),
                                            error_message: pin_error().clone(),
                                            on_complete: handle_pin_complete,
                                            on_cancel: None,
                                            on_input: Some(handle_pin_input.clone()),
                                            show_strength: Some(false),
                                            step_indicator: Some("Sensitive action".to_string()),
                                            clear_on_complete: Some(true),
                                            is_processing: Some(verifying_pin()),
                                            processing_label: Some("Verifying securely...".to_string()),
                                            reset_key: Some("export-wallet-pin".to_string()),
                                        }
                                    }
                                } else {
                                    if let Some(private_key) = exported_private_key.clone() {
                                        div { class: "private-key-display",
                                            "{private_key}"
                                        }
                                        div {
                                            class: "key-format-info",
                                            "Base58 encoded Solana keypair (64 bytes) - Compatible with Solana CLI and other wallets"
                                        }
                                        div {
                                            class: "copy-hint",
                                            "Make sure to copy this key to a secure location!"
                                        }
                                    } else {
                                        div { class: "error-message",
                                            "Unable to decrypt this wallet. Unlock the app with your PIN first."
                                        }
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
