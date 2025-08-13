// src/components/modals/hardware_modal.rs
use dioxus::prelude::*;
use crate::hardware::{HardwareWallet, HardwareDeviceInfo, HardwareDeviceType};
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

    // Function to refresh device list
    let refresh_devices = move |_| {
        scanning.set(true);
        error_message.set(None);
        spawn(async move {
            let devices = HardwareWallet::scan_available_devices().await;
            available_devices.set(devices);
            scanning.set(false);
        });
    };

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
                        "Connect your hardware wallet via USB"
                    }

                    // Device scanning status
                    if scanning() {
                        div {
                            class: "scanning-message",
                            "🔍 Scanning for devices..."
                        }
                    } else {
                        // Show available devices
                        if available_devices().is_empty() {
                            div {
                                class: "no-devices-message",
                                "No hardware wallets detected."
                                br {}
                                "Please connect your device and ensure:"
                                ul {
                                    li { "ESP32: Device is connected via USB with proper drivers" }
                                    li { "Ledger: Device is unlocked, Solana app is open, and Ledger Live is closed" }
                                }
                            }
                        } else {
                            div {
                                class: "devices-list",
                                h3 { "Available Devices:" }
                                
                                for device in available_devices() {
                                    div {
                                        class: "device-item",
                                        div {
                                            class: "device-info",
                                            span {
                                                class: "device-icon",
                                                match device.device_type {
                                                    HardwareDeviceType::ESP32 => "🔧",
                                                    HardwareDeviceType::Ledger => "🔒",
                                                }
                                            }
                                            div {
                                                class: "device-details",
                                                div { class: "device-name", "{device.name}" }
                                                div { 
                                                    class: "device-type",
                                                    match device.device_type {
                                                        HardwareDeviceType::ESP32 => "ESP32 Hardware Wallet",
                                                        HardwareDeviceType::Ledger => "Ledger Hardware Wallet",
                                                    }
                                                }
                                            }
                                        }
                                        button {
                                            class: "connect-device-button",
                                            disabled: connecting(),
                                            onclick: {
                                                let dev_type = device.device_type.clone();
                                                move |_| connect_device(dev_type.clone())
                                            },
                                            if connecting() { "Connecting..." } else { "Connect" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    div { class: "modal-buttons",
                        button {
                            class: "modal-button secondary",
                            onclick: refresh_devices,
                            disabled: scanning() || connecting(),
                            if scanning() { "Scanning..." } else { "Refresh Devices" }
                        }
                        button {
                            class: "modal-button cancel",
                            onclick: move |_| onclose.call(()),
                            disabled: connecting(),
                            "Cancel"
                        }
                    }
                } else {
                    // Connected state - show wallet info and disconnect option
                    div {
                        class: "success-message",
                        "✅ Hardware wallet connected!"
                    }
                    
                    if let Some(dev_type) = device_type() {
                        div {
                            class: "device-info-connected",
                            div {
                                class: "device-icon-large",
                                match dev_type {
                                    HardwareDeviceType::ESP32 => "🔧",
                                    HardwareDeviceType::Ledger => "🔒",
                                }
                            }
                            div {
                                class: "device-details-connected",
                                div { class: "device-type-connected", "{dev_type}" }
                                if let Some(pubkey) = public_key() {
                                    div {
                                        class: "device-pubkey",
                                        "Public Key: "
                                        span { class: "pubkey-text", "{pubkey}" }
                                    }
                                }
                            }
                        }
                    }
                    
                    div { class: "modal-buttons",
                        button {
                            class: "modal-button secondary",
                            onclick: disconnect_device,
                            "Disconnect"
                        }
                        button {
                            class: "modal-button primary",
                            onclick: move |_| onclose.call(()),
                            "Continue"
                        }
                    }
                }
            }
        }
    }
}