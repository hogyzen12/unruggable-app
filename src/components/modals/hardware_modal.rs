use dioxus::prelude::*;
use crate::hardware::HardwareWallet;
use std::sync::Arc;

#[component]
pub fn HardwareWalletModal(
    onclose: EventHandler<()>,
    onsuccess: EventHandler<Arc<HardwareWallet>>,
    ondisconnect: EventHandler<()>,
    existing_wallet: Option<Arc<HardwareWallet>>,
) -> Element {
    let mut connecting = use_signal(|| false);
    let mut error_message = use_signal(|| None as Option<String>);
    let mut hardware_wallet = use_signal(|| existing_wallet.clone());
    let mut connected = use_signal(|| existing_wallet.is_some());
    let mut public_key = use_signal(|| None as Option<String>);
    
    // Store if we have an existing wallet
    let has_existing_wallet = existing_wallet.is_some();
    
    // If we have an existing wallet, get its public key
    use_effect(move || {
        if let Some(wallet) = &existing_wallet {
            let wallet = wallet.clone();
            spawn(async move {
                if let Ok(pubkey) = wallet.get_public_key().await {
                    public_key.set(Some(pubkey));
                    connected.set(true);
                }
            });
        }
    });
    
    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| onclose.call(()),
            
            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                
                h2 { class: "modal-title", "Hardware Wallet" }
                
                // Show error if any
                if let Some(error) = error_message() {
                    div {
                        class: "error-message",
                        "{error}"
                    }
                }
                
                if !connected() {
                    div {
                        class: "info-message",
                        "Connect your Unruggable hardware wallet via USB"
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
                                connecting.set(true);
                                error_message.set(None);
                                
                                spawn(async move {
                                    let wallet = Arc::new(HardwareWallet::new());
                                    match wallet.connect().await {
                                        Ok(_) => {
                                            match wallet.get_public_key().await {
                                                Ok(pubkey) => {
                                                    public_key.set(Some(pubkey.clone()));
                                                    hardware_wallet.set(Some(wallet.clone()));
                                                    connected.set(true);
                                                    connecting.set(false);
                                                    
                                                    // Automatically proceed after successful connection
                                                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                                                    onsuccess.call(wallet);
                                                }
                                                Err(e) => {
                                                    error_message.set(Some(format!("Failed to get public key: {}", e)));
                                                    connecting.set(false);
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            error_message.set(Some(format!("Failed to connect: {}", e)));
                                            connecting.set(false);
                                        }
                                    }
                                });
                            },
                            disabled: connecting(),
                            if connecting() { "Connecting..." } else { "Connect Hardware Wallet" }
                        }
                    }
                } else {
                    // Connected state - show wallet info and disconnect option
                    div {
                        class: "success-message",
                        "Hardware wallet connected!"
                    }
                    
                    if let Some(pubkey) = public_key() {
                        div {
                            class: "wallet-field",
                            label { "Public Address:" }
                            div { class: "address-display", "{pubkey}" }
                        }
                    }
                    
                    div { class: "modal-buttons",
                        button {
                            class: "modal-button secondary",
                            onclick: move |_| {
                                if let Some(wallet) = hardware_wallet() {
                                    let wallet = wallet.clone();
                                    spawn(async move {
                                        wallet.disconnect().await;
                                    });
                                }
                                hardware_wallet.set(None);
                                connected.set(false);
                                public_key.set(None);
                                ondisconnect.call(());
                            },
                            "Disconnect"
                        }
                        button {
                            class: "modal-button cancel",
                            onclick: move |_| onclose.call(()),
                            "Close"
                        }
                        if !has_existing_wallet {
                            button {
                                class: "modal-button primary",
                                onclick: move |_| {
                                    if let Some(wallet) = hardware_wallet() {
                                        onsuccess.call(wallet);
                                    }
                                },
                                "Use This Wallet"
                            }
                        }
                    }
                }
            }
        }
    }
}