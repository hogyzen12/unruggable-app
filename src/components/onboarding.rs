// src/components/onboarding.rs
use dioxus::prelude::*;
use crate::storage;

const ONBOARDING_KEY: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/onboarding_key.webp";

#[component]
pub fn OnboardingFlow(on_complete: EventHandler<()>) -> Element {
    let mut current_step = use_signal(|| 0);
    let total_steps = 2; // Changed from 3 to 2

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
                    // Removed step 2
                    _ => rsx! { div {} }
                }

                div {
                    class: "onboarding-progress",
                    for i in 0..total_steps {
                        div {
                            class: if i == current_step() { "progress-dot active" } else { "progress-dot" }
                        }
                    }
                }

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