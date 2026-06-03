// src/components/pin_unlock.rs
use crate::components::pin_input::PinInput;
use crate::storage;
use dioxus::prelude::*;

#[component]
pub fn PinUnlock(on_unlock: EventHandler<()>) -> Element {
    let pin_unlock_lock_art = crate::asset_hosting::app_asset("lock.png");
    let mut error_message = use_signal(|| None::<String>);
    let mut is_locked = use_signal(|| storage::is_pin_locked());
    let mut verifying = use_signal(|| false);

    let handle_pin_input = EventHandler::new(move |_| {
        if error_message().is_some() {
            error_message.set(None);
        }
    });

    let handle_pin_complete = move |pin: String| {
        if verifying() || is_locked() {
            return;
        }

        println!("PIN entry complete - starting unlock verification");
        verifying.set(true);
        error_message.set(None);

        let on_unlock = on_unlock.clone();
        spawn(async move {
            match storage::verify_pin_async(pin).await {
                Ok(_salt) => {
                    println!("PIN verification returned success - switching app to unlocked state");
                    verifying.set(false);
                    error_message.set(None);
                    on_unlock.call(());
                }
                Err(e) => {
                    println!("PIN verification failed: {}", e);
                    verifying.set(false);
                    error_message.set(Some(e));
                    if storage::is_pin_locked() {
                        is_locked.set(true);
                    }
                }
            }
        });
    };

    rsx! {
        div {
            class: "pin-unlock-overlay",

            if is_locked() {
                div {
                    class: "pin-unlock-stage onboarding-pin-shell onboarding-pin-status-shell pin-unlock-locked-stage",

                    div {
                        class: "onboarding-mini-art-shell onboarding-mini-art-shell-lock",
                        img {
                            class: "onboarding-mini-art",
                            src: pin_unlock_lock_art.clone(),
                            alt: "Unruggable lock artwork"
                        }
                    }

                    h2 {
                        class: "onboarding-status-title pin-locked-title",
                        "Wallet Locked"
                    }

                    p {
                        class: "onboarding-support-copy onboarding-support-copy-tight pin-locked-message",
                        "Too many failed attempts."
                        br {}
                        "Your encrypted wallet data is still on this device."
                        br {}
                        "Reinstalling will not recover access. Restore from a saved private key or seed backup instead."
                    }

                    div {
                        class: "flow-note-card flow-note-card-accent pin-unlock-note-card",
                        div {
                            class: "flow-note-label",
                            "Recovery required"
                        }
                        p {
                            class: "flow-note-copy",
                            "Unlock is disabled after too many failed attempts. Use your saved recovery material to regain access."
                        }
                    }
                }
            } else {
                div {
                    class: "pin-unlock-stage onboarding-pin-shell pin-unlock-shell",

                    div {
                        class: "onboarding-mini-art-shell onboarding-mini-art-shell-lock",
                        img {
                            class: "onboarding-mini-art",
                            src: pin_unlock_lock_art,
                            alt: "Unruggable lock artwork"
                        }
                    }

                    p {
                        class: "onboarding-description onboarding-description-tight",
                        "Enter your 6-digit PIN to unlock Unruggable on this device."
                    }

                    div {
                        class: "flow-note-card flow-note-card-accent pin-unlock-note-card",
                        div {
                            class: "flow-note-label",
                            "Encrypted on this device"
                        }
                        p {
                            class: "flow-note-copy",
                            "Your wallet keys stay on this device and unlock with this PIN."
                        }
                    }

                    PinInput {
                        title: "Enter PIN".to_string(),
                        subtitle: Some("Unlock your Unruggable software wallet.".to_string()),
                        error_message: error_message().clone(),
                        on_complete: handle_pin_complete,
                        on_cancel: None,
                        on_input: Some(handle_pin_input),
                        show_strength: Some(false),
                        step_indicator: None,
                        clear_on_complete: Some(false),
                        is_processing: Some(verifying()),
                        processing_label: Some("Unlocking securely...".to_string()),
                    }
                }
            }
        }
    }
}
