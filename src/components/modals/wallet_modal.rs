use crate::pin::wipe_secret_string;
use crate::storage::{import_wallet_from_key, save_wallet_to_storage};
use crate::wallet::{Wallet, WalletInfo};
use dioxus::prelude::*;

#[component]
pub fn WalletModal(
    mode: String,
    onclose: EventHandler<()>,
    onsave: EventHandler<WalletInfo>,
) -> Element {
    let mut wallet_name = use_signal(|| "".to_string());
    let mut import_key = use_signal(|| "".to_string());
    let mut show_generated_key = use_signal(|| false);
    let mut generated_wallet = use_signal(|| None as Option<Wallet>);
    let mut error_message = use_signal(|| None as Option<String>);
    let mut show_format_help = use_signal(|| false);
    let mut persist_in_progress = use_signal(|| false);

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
                        label {
                            "Private Key:"
                            button {
                                class: "help-button",
                                onclick: move |_| show_format_help.set(!show_format_help()),
                                "ℹ️"
                            }
                        }
                        textarea {
                            value: "{import_key}",
                            oninput: move |e| import_key.set(e.value()),
                            placeholder: "Enter a private key in base58, JSON, hex, or base64",
                            rows: "4"
                        }

                        // Format help section
                        if show_format_help() {
                            div {
                                class: "format-help",
                                h4 { "Supported Formats:" }
                                div { class: "format-example",
                                    strong { "1. Base58 (Solana standard):" }
                                    code { "5Jxyz...abc123" }
                                }
                                div { class: "format-example",
                                    strong { "2. JSON Array (Phantom/Sollet):" }
                                    code { "[252,183,12,...,159,189]" }
                                }
                                div { class: "format-example",
                                    strong { "3. JSON Object:" }
                                    code { "{{\"privateKey\":\"5Jxyz...abc123\"}}" }
                                }
                                div { class: "format-example",
                                    strong { "4. Hex / Base64:" }
                                    code { "a1b2c3... or SGVsbG8..." }
                                }
                            }
                        }
                    }
                }

                // Buttons section
                div {
                    class: "modal-buttons",
                    button {
                        class: "modal-button cancel",
                        disabled: persist_in_progress(),
                        onclick: move |_| onclose.call(()),
                        "Cancel"
                    }
                    if mode == "create" {
                        if generated_wallet().is_none() {
                            button {
                                class: "modal-button primary",
                                disabled: persist_in_progress(),
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
                                    if persist_in_progress() {
                                        return;
                                    }
                                    if let Some(wallet) = generated_wallet() {
                                        match wallet.to_wallet_info() {
                                            Ok(mut wallet_info) => {
                                                wallet_info.name = if wallet_name().is_empty() {
                                                    wallet.name.clone()
                                                } else {
                                                    wallet_name()
                                                };
                                                persist_in_progress.set(true);
                                                error_message.set(None);

                                                spawn(async move {
                                                    let result = tokio::task::spawn_blocking(move || {
                                                        save_wallet_to_storage(&wallet_info)?;
                                                        Ok::<WalletInfo, String>(wallet_info)
                                                    })
                                                    .await
                                                    .map_err(|e| format!("Wallet save task failed: {e}"))
                                                    .and_then(|result| result);

                                                    persist_in_progress.set(false);

                                                    match result {
                                                        Ok(saved_wallet) => {
                                                            error_message.set(None);
                                                            onsave.call(saved_wallet);
                                                        }
                                                        Err(e) => {
                                                            error_message.set(Some(e));
                                                        }
                                                    }
                                                });
                                            }
                                            Err(e) => {
                                                error_message.set(Some(e));
                                            }
                                        }
                                    }
                                },
                                disabled: persist_in_progress() || !show_generated_key(),
                                if persist_in_progress() {
                                    "Saving..."
                                } else if !show_generated_key() {
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
                                if persist_in_progress() {
                                    return;
                                }
                                if !import_key().is_empty() {
                                    let private_key_input = import_key();
                                    let wallet_name_input = wallet_name();
                                    persist_in_progress.set(true);
                                    error_message.set(None);

                                    spawn(async move {
                                        let result = tokio::task::spawn_blocking(move || {
                                            let mut secret = private_key_input;
                                            let import_result = import_wallet_from_key(&secret, wallet_name_input)
                                                .and_then(|wallet_info| {
                                                    save_wallet_to_storage(&wallet_info)?;
                                                    Ok(wallet_info)
                                                });
                                            wipe_secret_string(&mut secret);
                                            import_result
                                        })
                                        .await
                                        .map_err(|e| format!("Wallet import task failed: {e}"))
                                        .and_then(|result| result);

                                        persist_in_progress.set(false);

                                        match result {
                                            Ok(wallet_info) => {
                                                import_key.set(String::new());
                                                error_message.set(None);
                                                onsave.call(wallet_info);
                                            }
                                            Err(e) => {
                                                error_message.set(Some(e));
                                            }
                                        }
                                    });
                                } else {
                                    error_message.set(Some("Please enter a private key".to_string()));
                                }
                            },
                            disabled: persist_in_progress(),
                            if persist_in_progress() {
                                "Importing..."
                            } else {
                                "Import"
                            }
                        }
                    }
                }
            }
        }
    }
}
