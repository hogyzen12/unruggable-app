// src/components/onboarding.rs
use crate::components::pin_input::PinInput;
use crate::storage;
use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq)]
enum PinSetupMode {
    EnterPin,
    ConfirmPin,
}

#[component]
pub fn OnboardingFlow(on_complete: EventHandler<()>) -> Element {
    let onboarding_key_art = crate::asset_hosting::app_asset("key_screen_1.png");
    let onboarding_lock_art = crate::asset_hosting::app_asset("lock.png");
    let mut current_step = use_signal(|| 0);
    let mut pin_setup_mode = use_signal(|| PinSetupMode::EnterPin);
    let mut entered_pin = use_signal(|| String::new());
    let mut pin_error = use_signal(|| None::<String>);
    let total_steps = 3; // Welcome, Security, PIN Setup

    let next_step = move |_| {
        if current_step() < total_steps - 1 {
            current_step += 1;
        }
    };

    let mut show_success = use_signal(|| false);
    let mut saving_pin = use_signal(|| false);
    let handle_pin_input = EventHandler::new(move |_| {
        if pin_error().is_some() {
            pin_error.set(None);
        }
    });

    let handle_pin_complete = move |pin: String| {
        if saving_pin() {
            return;
        }

        match pin_setup_mode() {
            PinSetupMode::EnterPin => {
                if pin.len() == 6 {
                    log::info!("First PIN entered: {} digits", pin.len());
                    entered_pin.set(pin.clone());
                    pin_setup_mode.set(PinSetupMode::ConfirmPin);
                    pin_error.set(None);
                }
            }
            PinSetupMode::ConfirmPin => {
                log::info!(
                    "Confirming PIN: entered={}, confirmation={}",
                    entered_pin().len(),
                    pin.len()
                );

                if pin == entered_pin() {
                    saving_pin.set(true);
                    pin_error.set(None);

                    let on_complete = on_complete.clone();
                    spawn(async move {
                        match storage::save_pin_async(pin).await {
                            Ok(_) => {
                                log::info!("PIN saved successfully");
                                saving_pin.set(false);
                                show_success.set(true);

                                spawn(async move {
                                    tokio::time::sleep(std::time::Duration::from_millis(1500))
                                        .await;
                                    storage::mark_onboarding_completed();
                                    on_complete.call(());
                                });
                            }
                            Err(e) => {
                                log::error!("Failed to save PIN: {}", e);
                                saving_pin.set(false);
                                pin_error
                                    .set(Some("Failed to save PIN. Please try again.".to_string()));
                            }
                        }
                    });
                } else {
                    pin_error.set(Some("PINs don't match. Let's try again.".to_string()));

                    // Delay before resetting to show error
                    spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
                        pin_setup_mode.set(PinSetupMode::EnterPin);
                        entered_pin.set(String::new());
                        pin_error.set(None);
                    });
                }
            }
        }
    };

    let handle_pin_cancel = EventHandler::new(move |_| {
        saving_pin.set(false);
        pin_setup_mode.set(PinSetupMode::EnterPin);
        entered_pin.set(String::new());
        pin_error.set(None);
    });

    rsx! {
        div {
            class: "onboarding-overlay",

            div {
                class: "onboarding-container",

                match current_step() {
                    0 => rsx! {
                        div {
                            class: "onboarding-step onboarding-step-editorial",
                            div {
                                class: "onboarding-copy-group",
                                h1 {
                                    class: "onboarding-title",
                                    "Welcome to"
                                }
                                h1 {
                                    class: "onboarding-title brand",
                                    "Unruggable"
                                }
                            }

                            div {
                                class: "flow-hero-media flow-hero-media-key",
                                img {
                                    class: "flow-hero-image flow-hero-image-key",
                                    src: onboarding_key_art,
                                    alt: "Unruggable key artwork"
                                }
                            }

                            p {
                                class: "onboarding-support-copy",
                                "Your Unruggable account is secured on this device."
                            }
                        }
                    },
                    1 => rsx! {
                        div {
                            class: "onboarding-step onboarding-step-editorial",
                            h1 {
                                class: "onboarding-title",
                                "Secure and Private"
                            }

                            div {
                                class: "flow-hero-media flow-hero-media-lock",
                                img {
                                    class: "flow-hero-image flow-hero-image-lock",
                                    src: onboarding_lock_art.clone(),
                                    alt: "Unruggable lock artwork"
                                }
                            }

                            p {
                                class: "onboarding-description onboarding-description-wide",
                                "Your software wallet keys are encrypted on-device with your PIN."
                            }

                            p {
                                class: "onboarding-support-copy onboarding-support-copy-tight",
                                "They never leave your device, and the app cannot unlock them without the PIN."
                            }
                        }
                    },
                    2 => rsx! {
                        div {
                            class: "onboarding-step onboarding-step-pin-flow",

                            match pin_setup_mode() {
                                PinSetupMode::EnterPin => rsx! {
                                    if show_success() {
                                        div {
                                            class: "onboarding-pin-shell onboarding-pin-status-shell",
                                            div {
                                                class: "onboarding-mini-art-shell onboarding-mini-art-shell-lock",
                                                img {
                                                    class: "onboarding-mini-art",
                                                    src: onboarding_lock_art.clone(),
                                                    alt: "Unruggable lock artwork"
                                                }
                                            }
                                            h2 {
                                                class: "onboarding-status-title",
                                                "PIN Set Successfully!"
                                            }
                                            p {
                                                class: "onboarding-support-copy onboarding-support-copy-tight",
                                                "Your wallet is now protected and ready to unlock."
                                            }
                                        }
                                    } else {
                                        div {
                                            class: "onboarding-pin-shell",
                                            div {
                                                class: "onboarding-mini-art-shell onboarding-mini-art-shell-lock",
                                                img {
                                                    class: "onboarding-mini-art",
                                                    src: onboarding_lock_art.clone(),
                                                    alt: "Unruggable lock artwork"
                                                }
                                            }
                                            p {
                                                class: "onboarding-description onboarding-description-tight",
                                                "Create a 6-digit PIN for your Unruggable software wallet."
                                            }
                                            div {
                                                class: "flow-note-card flow-note-card-accent",
                                                div {
                                                    class: "flow-note-label",
                                                    "Encrypted on this device"
                                                }
                                                p {
                                                    class: "flow-note-copy",
                                                    "Your wallet keys stay on this device and unlock with this PIN."
                                                }
                                            }
                                            div {
                                                key: "onboarding-pin-entry-shell",
                                                PinInput {
                                                    title: "Create PIN".to_string(),
                                                    subtitle: Some("Choose a 6-digit code for this device.".to_string()),
                                                    error_message: pin_error().clone(),
                                                    on_complete: handle_pin_complete,
                                                    on_cancel: Some(handle_pin_cancel.clone()),
                                                    on_input: Some(handle_pin_input.clone()),
                                                    show_strength: Some(false),
                                                    step_indicator: Some("Step 1 of 2".to_string()),
                                                    clear_on_complete: Some(true),
                                                    is_processing: Some(false),
                                                    processing_label: None,
                                                    reset_key: Some("onboarding-pin-enter".to_string()),
                                                }
                                            }
                                        }
                                    }
                                },
                                PinSetupMode::ConfirmPin => rsx! {
                                    if show_success() {
                                        div {
                                            class: "onboarding-pin-shell onboarding-pin-status-shell",
                                            div {
                                                class: "onboarding-mini-art-shell onboarding-mini-art-shell-lock",
                                                img {
                                                    class: "onboarding-mini-art",
                                                    src: onboarding_lock_art.clone(),
                                                    alt: "Unruggable lock artwork"
                                                }
                                            }
                                            h2 {
                                                class: "onboarding-status-title",
                                                "PIN Set Successfully!"
                                            }
                                            p {
                                                class: "onboarding-support-copy onboarding-support-copy-tight",
                                                "Your wallet is now protected and ready to unlock."
                                            }
                                        }
                                    } else {
                                        div {
                                            class: "onboarding-pin-shell",
                                            div {
                                                class: "onboarding-mini-art-shell onboarding-mini-art-shell-lock",
                                                img {
                                                    class: "onboarding-mini-art",
                                                    src: onboarding_lock_art.clone(),
                                                    alt: "Unruggable lock artwork"
                                                }
                                            }
                                            p {
                                                class: "onboarding-description onboarding-description-tight",
                                                "Enter the same PIN again to finish setup."
                                            }
                                            div {
                                                class: "flow-note-card flow-note-card-accent",
                                                div {
                                                    class: "flow-note-label",
                                                    "Encrypted on this device"
                                                }
                                                p {
                                                    class: "flow-note-copy",
                                                    "Your wallet keys stay on this device and unlock with this PIN."
                                                }
                                            }
                                            div {
                                                key: "onboarding-pin-confirm-shell",
                                                PinInput {
                                                    title: "Confirm PIN".to_string(),
                                                    subtitle: Some("Enter the same 6-digit PIN again.".to_string()),
                                                    error_message: pin_error().clone(),
                                                    on_complete: handle_pin_complete,
                                                    on_cancel: Some(handle_pin_cancel.clone()),
                                                    on_input: Some(handle_pin_input.clone()),
                                                    show_strength: Some(false),
                                                    step_indicator: Some("Step 2 of 2".to_string()),
                                                    clear_on_complete: Some(true),
                                                    is_processing: Some(saving_pin()),
                                                    processing_label: Some("Securing wallet...".to_string()),
                                                    reset_key: Some("onboarding-pin-confirm".to_string()),
                                                }
                                            }
                                        }
                                    }
                                },
                            }
                        }
                    },
                    _ => rsx! { div {} }
                }

                if current_step() != 2 {
                    div {
                        class: "onboarding-progress",
                        for i in 0..total_steps {
                            div {
                                class: if i == current_step() { "progress-dot active" } else { "progress-dot" }
                            }
                        }
                    }
                }

                if current_step() != 2 {
                    div {
                        class: "onboarding-buttons",
                        button {
                            class: "onboarding-button primary",
                            onclick: next_step,
                            if current_step() < total_steps - 1 { "Next" } else { "Get Started" }
                        }
                    }
                }

                p {
                    class: "onboarding-alpha-note",
                    "Initial alpha release."
                }
            }
        }
    }
}
