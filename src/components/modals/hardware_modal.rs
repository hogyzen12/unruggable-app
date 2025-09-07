// src/components/modals/hardware_modal.rs
use dioxus::prelude::*;
use crate::hardware::{HardwareWallet, HardwareDeviceInfo, HardwareDeviceType};
use std::sync::Arc;

// Define the assets for device icons
const ICON_UNRUGGABLE: Asset = asset!("/assets/icon.png");
const ICON_LEDGER: Asset = asset!("/assets/icons/ledgerLogo.webp");

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
    let mut device_type = use_signal(|| None as Option<HardwareDeviceType>);
    let mut available_devices = use_signal(|| Vec::<HardwareDeviceInfo>::new());
    let mut scanning = use_signal(|| false);
    
    // Store if we have an existing wallet
    let has_existing_wallet = existing_wallet.is_some();
    
    // If we have an existing wallet, get its details
    use_effect(move || {
        if let Some(wallet) = &existing_wallet {
            let wallet = wallet.clone();
            spawn(async move {
                if let Ok(pubkey) = wallet.get_public_key().await {
                    public_key.set(Some(pubkey));
                    connected.set(true);
                }
                if let Some(dev_type) = wallet.get_device_type().await {
                    device_type.set(Some(dev_type));
                }
            });
        }
    });

    // Scan for available devices when modal opens
    use_effect(move || {
        if !has_existing_wallet {
            scanning.set(true);
            spawn(async move {
                let devices = HardwareWallet::scan_available_devices().await;
                available_devices.set(devices);
                scanning.set(false);
            });
        }
    });

    // Function to connect to a specific device type
    let mut connect_device = move |dev_type: HardwareDeviceType| {
        connecting.set(true);
        error_message.set(None);
        
        spawn(async move {
            let wallet = Arc::new(HardwareWallet::new());
            
            let result = match dev_type {
                HardwareDeviceType::ESP32 => wallet.connect_esp32().await,
                HardwareDeviceType::Ledger => wallet.connect_ledger().await,
            };

            match result {
                Ok(_) => {
                    match wallet.get_public_key().await {
                        Ok(pubkey) => {
                            public_key.set(Some(pubkey.clone()));
                            device_type.set(Some(dev_type));
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
    };

    // Function to disconnect
    let disconnect_device = move |_| {
        if let Some(wallet) = hardware_wallet() {
            spawn(async move {
                let _ = wallet.disconnect().await;
            });
        }
        hardware_wallet.set(None);
        connected.set(false);
        public_key.set(None);
        device_type.set(None);
        ondisconnect.call(());
    };

    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| onclose.call(()),
            
            div {
                class: "modal-content hardware-modal",
                onclick: move |e| e.stop_propagation(),
                
                div {
                    class: "modal-header",
                    h2 { class: "modal-title", "Hardware Wallet" }
                    button {
                        class: "modal-close-button",
                        onclick: move |_| onclose.call(()),
                        "×"
                    }
                }
                
                div {
                    class: "modal-body",
                    
                    // Show error if any
                    if let Some(error) = error_message() {
                        div {
                            class: "error-message",
                            div { class: "error-icon", "⚠️" }
                            div { class: "error-text", "{error}" }
                        }
                    }
                    
                    if !connected() {
                        div {
                            class: "connection-section",
                            
                            div {
                                class: "info-header",
                                h3 { "Connect Your Hardware Wallet" }
                                p { class: "info-subtitle", "Secure your transactions with hardware-based signing" }
                            }

                            // Device scanning status
                            if scanning() {
                                div {
                                    class: "scanning-container",
                                    div { class: "scanning-spinner" }
                                    div { class: "scanning-text", "Scanning for devices..." }
                                }
                            } else {
                                // Show available devices or empty state
                                if available_devices().is_empty() {
                                    div {
                                        class: "no-devices-container",
                                        div { class: "no-devices-icon", "🔍" }
                                        div { class: "no-devices-title", "No Hardware Wallets Detected" }
                                        div { class: "no-devices-subtitle", "Please connect your device and ensure:" }
                                        ul {
                                            class: "device-requirements",
                                            li { 
                                                strong { "Unruggable: " }
                                                "Device is connected via USB with proper drivers installed"
                                            }
                                            li { 
                                                strong { "Ledger: " }
                                                "Device is unlocked, Solana app is open, and Ledger Live is closed"
                                            }
                                        }
                                    }
                                } else {
                                    div {
                                        class: "devices-section",
                                        h4 { class: "devices-title", "Available Devices" }
                                        
                                        div {
                                            class: "devices-grid",
                                            for device in available_devices() {
                                                div {
                                                    class: "device-card",
                                                    div {
                                                        class: "device-icon-container",
                                                        div {
                                                            class: if device.device_type == HardwareDeviceType::ESP32 {
                                                                "device-icon device-icon-unruggable"
                                                            } else {
                                                                "device-icon device-icon-ledger"
                                                            },
                                                            // Device logo images
                                                            img {
                                                                src: if device.device_type == HardwareDeviceType::ESP32 {
                                                                    ICON_UNRUGGABLE
                                                                } else {
                                                                    ICON_LEDGER
                                                                },
                                                                alt: if device.device_type == HardwareDeviceType::ESP32 {
                                                                    "Unruggable Hardware Wallet"
                                                                } else {
                                                                    "Ledger Hardware Wallet"
                                                                },
                                                                width: "48",
                                                                height: "48"
                                                            }
                                                        }
                                                    }
                                                    
                                                    div {
                                                        class: "device-info",
                                                        div { class: "device-name", "{device.name}" }
                                                        div { 
                                                            class: if device.device_type == HardwareDeviceType::ESP32 {
                                                                "device-type-badge unruggable-badge"
                                                            } else {
                                                                "device-type-badge ledger-badge"
                                                            },
                                                            if device.device_type == HardwareDeviceType::ESP32 {
                                                                "Unruggable Wallet"
                                                            } else {
                                                                "Ledger Wallet"
                                                            }
                                                        }
                                                    }
                                                    
                                                    button {
                                                        class: if connecting() {
                                                            "connect-device-button connecting"
                                                        } else {
                                                            "connect-device-button"
                                                        },
                                                        disabled: connecting(),
                                                        onclick: {
                                                            let dev_type = device.device_type.clone();
                                                            move |_| connect_device(dev_type.clone())
                                                        },
                                                        if connecting() {
                                                            div { class: "button-spinner" }
                                                            span { "Connecting..." }
                                                        } else {
                                                            span { "Connect" }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        
                    } else {
                        // Connected state - show wallet info and options
                        div {
                            class: "connected-section",
                            
                            div {
                                class: "success-header",
                                div { class: "success-icon", "✅" }
                                h3 { "Hardware Wallet Connected" }
                            }
                            
                            if let Some(dev_type) = device_type() {
                                div {
                                    class: "connected-device-card",
                                    div {
                                        class: "connected-device-icon",
                                        div {
                                            class: if dev_type == HardwareDeviceType::ESP32 {
                                                "device-icon-large device-icon-unruggable"
                                            } else {
                                                "device-icon-large device-icon-ledger"
                                            },
                                            // Larger device logo images for connected state
                                            img {
                                                src: if dev_type == HardwareDeviceType::ESP32 {
                                                    ICON_UNRUGGABLE
                                                } else {
                                                    ICON_LEDGER
                                                },
                                                alt: if dev_type == HardwareDeviceType::ESP32 {
                                                    "Unruggable Hardware Wallet"
                                                } else {
                                                    "Ledger Hardware Wallet"
                                                },
                                                width: "64",
                                                height: "64"
                                            }
                                        }
                                    }
                                    
                                    div {
                                        class: "connected-device-info",
                                        h4 { class: "connected-device-name", "{dev_type}" }
                                        if let Some(pubkey) = public_key() {
                                            div {
                                                class: "device-pubkey-section",
                                                div { class: "pubkey-label", "Public Key:" }
                                                div { 
                                                    class: "pubkey-display",
                                                    onclick: move |_| {
                                                        // Copy to clipboard functionality could be added here
                                                        log::info!("Public key copied: {}", pubkey);
                                                    },
                                                    span { class: "pubkey-text", "{pubkey}" }
                                                    div { class: "copy-hint", "Click to copy" }
                                                }
                                            }
                                        }
                                        
                                        div {
                                            class: "connection-status",
                                            div { class: "status-indicator connected" }
                                            span { "Securely Connected" }
                                        }
                                    }
                                }
                            }
                        }
                        
                        div { 
                            class: "connected-modal-actions",
                            button {
                                class: "connect-device-button",
                                onclick: disconnect_device,
                                div { class: "disconnect-icon", "🔌" }
                                span { "Disconnect Device" }
                            }
                        }
                    }
                }
            }
        }
    }
}