
use dioxus::prelude::*;
use crate::wallet::WalletInfo;
use crate::hardware::HardwareWallet;
use std::sync::Arc;
use qrcode::{QrCode, render::svg};

#[component]
pub fn ReceiveModal(
    wallet: Option<WalletInfo>,
    hardware_wallet: Option<Arc<HardwareWallet>>,
    onclose: EventHandler<()>,
) -> Element {
    let mut copying = use_signal(|| false);
    let mut copied = use_signal(|| false);
    let mut hardware_pubkey = use_signal(|| None as Option<String>);
    
    // Clone hardware_wallet for use in effect
    let hw_clone = hardware_wallet.clone();
    
    // If we have a hardware wallet, get its public key
    use_effect(move || {
        if let Some(hw) = &hw_clone {
            let hw = hw.clone();
            spawn(async move {
                if let Ok(pubkey) = hw.get_public_key().await {
                    hardware_pubkey.set(Some(pubkey));
                }
            });
        }
    });
    
    // Determine which address to show
    let address = if let Some(hw_key) = hardware_pubkey() {
        hw_key
    } else if let Some(w) = &wallet {
        w.address.clone()
    } else {
        "No Wallet".to_string()
    };
    
    // Generate QR code SVG
    let qr_svg = generate_qr_code_svg(&address);
    
    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| onclose.call(()),
            
            div {
                class: "modal-content receive-modal",
                onclick: move |e| e.stop_propagation(),
                
                h2 { class: "modal-title", "Receive" }
                
                // Info message
                div {
                    class: "info-message",
                    "This address can receive SOL and all SPL tokens on Solana"
                }
                
                // QR Code
                div {
                    class: "qr-code-container",
                    div {
                        class: "qr-code",
                        dangerous_inner_html: "{qr_svg}"
                    }
                }
                
                // Wallet name
                if let Some(w) = &wallet {
                    div {
                        class: "wallet-label",
                        "{w.name}"
                    }
                } else if hardware_wallet.is_some() {
                    div {
                        class: "wallet-label",
                        "Hardware Wallet"
                    }
                }
                
                // Address display with copy button
                div {
                    class: "address-container",
                    div {
                        class: "address-display-full",
                        onclick: {
                            let address = address.clone();
                            move |_| handle_copy(address.clone(), copying, copied)
                        },
                        div {
                            class: "address-text",
                            "{address}"
                        }
                        button {
                            class: "copy-button",
                            onclick: {
                                let address = address.clone();
                                move |e: MouseEvent| {
                                    e.stop_propagation();
                                    handle_copy(address.clone(), copying, copied);
                                }
                            },
                            if copying() {
                                "⏳"
                            } else if copied() {
                                "✅ Copied!"
                            } else {
                                "📋 Copy"
                            }
                        }
                    }
                }
                
                // Additional info
                div {
                    class: "receive-info",
                    p {
                        "Send SOL or any SPL token to this address. All tokens on Solana use the same receiving address."
                    }
                    if hardware_wallet.is_some() {
                        p {
                            class: "hardware-info",
                            "🔐 This is your hardware wallet address - keep your device safe!"
                        }
                    }
                }
                
                // Close button
                div { class: "modal-buttons",
                    button {
                        class: "modal-button primary",
                        onclick: move |_| onclose.call(()),
                        "Done"
                    }
                }
            }
        }
    }
}

// Helper function to handle copy to clipboard
fn handle_copy(address: String, mut copying: Signal<bool>, mut copied: Signal<bool>) {
    copying.set(true);
    copied.set(false);
    
    spawn(async move {
        // Copy to clipboard
        #[cfg(feature = "web")]
        {
            if let Some(window) = web_sys::window() {
                if let Some(navigator) = window.navigator() {
                    if let Some(clipboard) = navigator.clipboard() {
                        let _ = clipboard.write_text(&address);
                    }
                }
            }
        }
        
        #[cfg(not(feature = "web"))]
        {
            // For desktop, you might want to use arboard crate for cross-platform clipboard
            // arboard = "3.2"
            // if let Ok(mut clipboard) = arboard::Clipboard::new() {
            //     let _ = clipboard.set_text(&address);
            // }
            println!("Copy to clipboard: {}", address);
        }
        
        // Show copied feedback
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        copying.set(false);
        copied.set(true);
        
        // Reset copied state after 2 seconds
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        copied.set(false);
    });
}

// Helper function to generate QR code as SVG
fn generate_qr_code_svg(data: &str) -> String {
    match QrCode::new(data) {
        Ok(qr_code) => {
            // Generate SVG with proper styling
            let svg_string = qr_code.render()
                .min_dimensions(200, 200)
                .quiet_zone(false) // We handle padding in CSS
                .dark_color(svg::Color("#000000"))
                .light_color(svg::Color("#ffffff"))
                .build();
            svg_string
        }
        Err(e) => {
            // Fallback if QR code generation fails
            println!("Failed to generate QR code: {}", e);
            // Using concat! to avoid issues with # in raw strings
            concat!(
                r#"<svg viewBox="0 0 200 200" xmlns="http://www.w3.org/2000/svg">"#,
                r#"<rect width="200" height="200" fill="white"/>"#,
                r#"<text x="100" y="100" text-anchor="middle" font-family="Arial" font-size="14" fill="gray">"#,
                r#"QR Code Error"#,
                r#"</text></svg>"#
            ).to_string()
        }
    }
}