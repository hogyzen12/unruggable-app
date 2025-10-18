// src/components/onboarding.rs
use dioxus::prelude::*;
use crate::storage;
use crate::components::pin_input::PinInput;

const ONBOARDING_KEY: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/onboarding_key.webp";

#[derive(Clone, Copy, PartialEq)]
enum PinSetupMode {
    AskUser,
    EnterPin,
    Transitioning,
    ConfirmPin,
}

#[component]
pub fn OnboardingFlow(on_complete: EventHandler<()>) -> Element {
    let mut current_step = use_signal(|| 0);
    let mut pin_setup_mode = use_signal(|| PinSetupMode::AskUser);
    let mut entered_pin = use_signal(|| String::new());
    let mut pin_error = use_signal(|| None::<String>);
    let total_steps = 3; // Welcome, Security, PIN Setup

    let next_step = move |_| {
        if current_step() < total_steps - 1 {
            current_step += 1;
        } else {
            storage::mark_onboarding_completed();
            on_complete.call(());
        }
    };

    let skip = move |_| {
        storage::mark_onboarding_completed();
        on_complete.call(());
    };
    
    let skip_pin = move |_| {
        storage::mark_onboarding_completed();
        on_complete.call(());
    };
    
    let setup_pin = move |_| {
        pin_setup_mode.set(PinSetupMode::EnterPin);
        pin_error.set(None);
    };
    
    let mut confirming_pin = use_signal(|| String::new());
    let mut show_success = use_signal(|| false);
    
    let handle_pin_complete = move |pin: String| {
        match pin_setup_mode() {
            PinSetupMode::EnterPin => {
                if pin.len() == 6 {
                    log::info!("First PIN entered: {} digits", pin.len());
                    entered_pin.set(pin.clone());
                    pin_setup_mode.set(PinSetupMode::Transitioning);
                    pin_error.set(None);
                    
                    // Show transition screen briefly, then move to confirmation
                    spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
                        pin_setup_mode.set(PinSetupMode::ConfirmPin);
                        confirming_pin.set(String::new());
                    });
                }
            }
            PinSetupMode::ConfirmPin => {
                log::info!("Confirming PIN: entered={}, confirmation={}", 
                    entered_pin().len(), pin.len());
                
                confirming_pin.set(pin.clone());
                
                if pin == entered_pin() {
                    // PIN confirmed - save it
                    match storage::save_pin(&pin) {
                        Ok(_) => {
                            log::info!("PIN saved successfully");
                            show_success.set(true);
                            
                            // Show success for a moment before completing
                            spawn(async move {
                                tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                                storage::mark_onboarding_completed();
                                on_complete.call(());
                            });
                        }
                        Err(e) => {
                            log::error!("Failed to save PIN: {}", e);
                            pin_error.set(Some("Failed to save PIN. Please try again.".to_string()));
                        }
                    }
                } else {
                    pin_error.set(Some("PINs don't match. Let's try again.".to_string()));
                    
                    // Delay before resetting to show error
                    spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
                        pin_setup_mode.set(PinSetupMode::EnterPin);
                        entered_pin.set(String::new());
                        confirming_pin.set(String::new());
                        pin_error.set(None);
                    });
                }
            }
            _ => {}
        }
    };
    
    let handle_pin_cancel = EventHandler::new(move |_| {
        pin_setup_mode.set(PinSetupMode::AskUser);
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
                            class: "onboarding-step",
                            h1 { 
                                class: "onboarding-title",
                                "Welcome to"
                            }
                            h1 { 
                                class: "onboarding-title brand",
                                "Unruggable."
                            }
                            
                            img {
                                class: "onboarding-key-image",
                                src: ONBOARDING_KEY,
                                alt: "Unruggable Key"
                            }
                            
                            p { 
                                class: "onboarding-subtitle",
                                span { class: "highlight", "Your Unruggable account" }
                                br {}
                                span { class: "highlight", "is secured." }
                            }
                            
                            div {
                                class: "onboarding-footer",
                                p { class: "powered-by", "Powered by" }
                                p { class: "seeker-logo", "Seeker" }
                                p { class: "seeker-subtitle", "SOLANA ≡ MOBILE" }
                            }
                        }
                    },
                    1 => rsx! {
                        div {
                            class: "onboarding-step",
                            h1 { 
                                class: "onboarding-title",
                                "Secure & Private"
                            }
                            
                            div { 
                                class: "onboarding-icon-large", 
                                "🛡️" 
                            }
                            
                            p { 
                                class: "onboarding-description",
                                "Your keys are stored locally and encrypted."
                                br {}
                                "They never leave your device."
                            }
                        }
                    },
                    2 => rsx! {
                        div {
                            class: "onboarding-step",
                            
                            match pin_setup_mode() {
                                PinSetupMode::AskUser => rsx! {
                                    h1 { 
                                        class: "onboarding-title",
                                        "Set Up PIN"
                                    }
                                    
                                    div { 
                                        class: "onboarding-icon-large", 
                                        "🔐" 
                                    }
                                    
                                    p { 
                                        class: "onboarding-description",
                                        "Protect your wallet with a 6-digit PIN."
                                        br {}
                                        "You'll need it to unlock the app."
                                    }
                                    
                                    div {
                                        class: "onboarding-buttons pin-setup-buttons",
                                        button {
                                            class: "onboarding-button secondary",
                                            onclick: skip_pin,
                                            "Skip for Now"
                                        }
                                        button {
                                            class: "onboarding-button primary",
                                            onclick: setup_pin,
                                            "Set Up PIN"
                                        }
                                    }
                                },
                                PinSetupMode::EnterPin => rsx! {
                                    if show_success() {
                                        div {
                                            class: "pin-success-screen",
                                            div {
                                                class: "success-icon-large",
                                                "✓"
                                            }
                                            h2 {
                                                class: "success-title",
                                                "PIN Set Successfully!"
                                            }
                                            p {
                                                class: "success-subtitle",
                                                "Your wallet is now protected"
                                            }
                                        }
                                    } else {
                                        div {
                                            key: "{entered_pin.read().len()}_entry",
                                            PinInput {
                                                title: "Create Your PIN".to_string(),
                                                subtitle: Some("Choose a secure 6-digit code".to_string()),
                                                error_message: pin_error().clone(),
                                                on_complete: handle_pin_complete,
                                                on_cancel: Some(handle_pin_cancel.clone()),
                                                show_strength: Some(true),
                                                step_indicator: Some("Step 1 of 2".to_string()),
                                                clear_on_complete: Some(true),
                                            }
                                        }
                                    }
                                },
                                PinSetupMode::Transitioning => rsx! {
                                    div {
                                        class: "pin-transition-screen",
                                        div {
                                            class: "transition-icon",
                                            "✓"
                                        }
                                        h2 {
                                            class: "transition-title",
                                            "Great!"
                                        }
                                        p {
                                            class: "transition-subtitle",
                                            "Now confirm your PIN"
                                        }
                                        div {
                                            class: "transition-loader"
                                        }
                                    }
                                },
                                PinSetupMode::ConfirmPin => rsx! {
                                    if show_success() {
                                        div {
                                            class: "pin-success-screen",
                                            div {
                                                class: "success-icon-large animated-checkmark",
                                                "✓"
                                            }
                                            h2 {
                                                class: "success-title",
                                                "PIN Set Successfully!"
                                            }
                                            p {
                                                class: "success-subtitle",
                                                "Your wallet is now protected"
                                            }
                                        }
                                    } else {
                                        div {
                                            key: "{confirming_pin.read().len()}_confirm",
                                            PinInput {
                                                title: "Confirm Your PIN".to_string(),
                                                subtitle: Some("Enter the same PIN again".to_string()),
                                                error_message: pin_error().clone(),
                                                on_complete: handle_pin_complete,
                                                on_cancel: Some(handle_pin_cancel.clone()),
                                                show_strength: Some(false),
                                                step_indicator: Some("Step 2 of 2".to_string()),
                                                clear_on_complete: Some(true),
                                            }
                                        }
                                    }
                                },
                            }
                        }
                    },
                    _ => rsx! { div {} }
                }

                // Only show progress and buttons if not in PIN setup mode
                if current_step() != 2 || pin_setup_mode() == PinSetupMode::AskUser {
                    div {
                        class: "onboarding-progress",
                        for i in 0..total_steps {
                            div {
                                class: if i == current_step() { "progress-dot active" } else { "progress-dot" }
                            }
                        }
                    }
                }

                if current_step() != 2 || pin_setup_mode() == PinSetupMode::AskUser {
                    div {
                        class: "onboarding-buttons",
                        
                        if current_step() < total_steps - 1 {
                            button {
                                class: "onboarding-button secondary",
                                onclick: skip,
                                "Skip"
                            }
                        }
                        
                        button {
                            class: "onboarding-button primary",
                            onclick: next_step,
                            if current_step() < total_steps - 1 { "Next" } else { "Get Started" }
                        }
                    }
                }
            }
        }
    }
}