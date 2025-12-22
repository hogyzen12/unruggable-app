// src/components/pin_unlock.rs
use dioxus::prelude::*;
use crate::storage;
use crate::components::pin_input::PinInput;

#[component]
pub fn PinUnlock(on_unlock: EventHandler<String>) -> Element {
    let mut error_message = use_signal(|| None::<String>);
    let mut is_locked = use_signal(|| storage::is_pin_locked());

    let handle_pin_complete = move |pin: String| {
        // Verify PIN
        match storage::verify_pin(&pin) {
            Ok(_salt) => {
                // PIN verified successfully
                log::info!("PIN verified - unlocking app");
                error_message.set(None);
                on_unlock.call(pin); // Pass PIN to handler
            }
            Err(e) => {
                // PIN verification failed
                log::warn!("PIN verification failed: {}", e);
                error_message.set(Some(e.clone()));

                // Check if locked
                if storage::is_pin_locked() {
                    is_locked.set(true);
                }
            }
        }
    };
    
    rsx! {
        div {
            class: "pin-unlock-overlay",
            
            if is_locked() {
                div {
                    class: "pin-locked-container",
                    
                    div {
                        class: "pin-locked-icon",
                        "🔒"
                    }
                    
                    h2 {
                        class: "pin-locked-title",
                        "Wallet Locked"
                    }
                    
                    p {
                        class: "pin-locked-message",
                        "Too many failed attempts."
                        br {}
                        "Please reinstall the app to reset."
                    }
                }
            } else {
                PinInput {
                    title: "Enter PIN".to_string(),
                    subtitle: Some("Unlock your wallet".to_string()),
                    error_message: error_message().clone(),
                    on_complete: handle_pin_complete,
                    on_cancel: None,
                    show_strength: Some(false),
                    step_indicator: None,
                    clear_on_complete: Some(true),
                }
            }
        }
    }
}