// src/components/address_input.rs
use dioxus::prelude::*;
use solana_sdk::pubkey::Pubkey;
use crate::domain_resolver::DomainResolver;
use std::sync::Arc;

#[derive(Props, Clone, PartialEq)]
pub struct AddressInputProps {
    pub value: String,
    pub on_change: EventHandler<String>,
    pub on_resolved: EventHandler<Option<Pubkey>>,
    pub placeholder: Option<String>,
    pub label: Option<String>,
    pub disabled: Option<bool>,
    pub show_validation: Option<bool>,
    pub auto_resolve: Option<bool>, // Resolve as user types vs on blur
}

#[derive(Clone, PartialEq)]
pub enum ValidationState {
    Empty,
    Resolving,
    Success(Pubkey, String), // pubkey and description
    Error(String),
}

#[component]
pub fn AddressInput(props: AddressInputProps) -> Element {
    let mut validation_state = use_signal(|| ValidationState::Empty);
    let domain_resolver = use_context::<Arc<DomainResolver>>();
    
    let show_validation = props.show_validation.unwrap_or(true);
    let auto_resolve = props.auto_resolve.unwrap_or(false);
    let disabled = props.disabled.unwrap_or(false);

    // Clone necessary values for closures
    let props_value = props.value.clone();
    let props_on_change = props.on_change.clone();
    let props_on_resolved = props.on_resolved.clone();

    let resolve_address_handler = {
        let mut validation_state = validation_state.clone();
        let domain_resolver = domain_resolver.clone();
        let on_resolved = props_on_resolved.clone();
        
        move |input: String| {
            if input.trim().is_empty() {
                validation_state.set(ValidationState::Empty);
                on_resolved.call(None);
                return;
            }

            validation_state.set(ValidationState::Resolving);
            
            // Use the detailed resolver for better UX (now supports SNS + ANS)
            match domain_resolver.resolve_address_with_details(&input) {
                Ok((pubkey, description)) => {
                    validation_state.set(ValidationState::Success(pubkey, description));
                    on_resolved.call(Some(pubkey));
                },
                Err(error) => {
                    validation_state.set(ValidationState::Error(error));
                    on_resolved.call(None);
                }
            }
        }
    };

    // Handle input changes
    let handle_input = {
        let mut resolve_handler = resolve_address_handler.clone();
        let props_on_change = props_on_change.clone();
        
        move |evt: FormEvent| {
            let new_value = evt.value();
            props_on_change.call(new_value.clone());
            
            if auto_resolve && !new_value.trim().is_empty() {
                // Simple debounce using spawn
                let mut resolve_fn = resolve_handler.clone();
                spawn(async move {
                    // Simple delay using tokio (which you already have)
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    resolve_fn(new_value);
                });
            }
        }
    };

    // Handle blur (when user clicks away)
    let handle_blur = {
        let mut resolve_handler = resolve_address_handler.clone();
        let props_value = props_value.clone();
        
        move |_| {
            if !auto_resolve {
                resolve_handler(props_value.clone());
            }
        }
    };

    // CSS classes based on validation state
    let input_class = match &*validation_state.read() {
        ValidationState::Empty => "address-input",
        ValidationState::Resolving => "address-input address-input--resolving",
        ValidationState::Success(_, _) => "address-input address-input--success",
        ValidationState::Error(_) => "address-input address-input--error",
    };

    rsx! {
        div { class: "address-input-container",
            
            // Label
            if let Some(label) = &props.label {
                label { 
                    class: "address-input-label",
                    "{label}"
                }
            }
            
            // Input field
            div { class: "address-input-wrapper",
                input {
                    class: input_class,
                    value: "{props.value}",
                    placeholder: props.placeholder.unwrap_or("Enter address or .sol domain".to_string()),
                    disabled: disabled,
                    oninput: handle_input,
                    onblur: handle_blur,
                }
                
                // Status indicator
                div { class: "address-input-status",
                    match &*validation_state.read() {
                        ValidationState::Resolving => rsx! {
                            span { class: "status-resolving", "🔍" }
                        },
                        ValidationState::Success(_, _) => rsx! {
                            span { class: "status-success", "✅" }
                        },
                        ValidationState::Error(_) => rsx! {
                            span { class: "status-error", "❌" }
                        },
                        ValidationState::Empty => rsx! { span {} }
                    }
                }
            }
            
            // Validation feedback
            if show_validation {
                div { class: "address-input-feedback",
                    match &*validation_state.read() {
                        ValidationState::Resolving => rsx! {
                            div { class: "feedback-resolving",
                                "Resolving domain..."
                            }
                        },
                        ValidationState::Success(pubkey, description) => rsx! {
                            div { class: "feedback-success",
                                div { class: "feedback-description", "{description}" }
                                div { class: "feedback-address", "{pubkey}" }
                            }
                        },
                        ValidationState::Error(error) => rsx! {
                            div { class: "feedback-error",
                                "{error}"
                            }
                        },
                        ValidationState::Empty => rsx! { div {} }
                    }
                }
            }
            
            // Helper text
            if matches!(&*validation_state.read(), ValidationState::Empty) {
                div { class: "address-input-helper",
                    "You can enter a Solana address or domain (.sol, .abc, .bonk, etc.)"
                }
            }
        }
        
        // CSS styles
        style { {CSS_STYLES} }
    }
}

// CSS styles for the component
const CSS_STYLES: &str = r#"
.address-input-container {
    width: 100%;
    margin-bottom: 1rem;
}

.address-input-label {
    display: block;
    margin-bottom: 0.5rem;
    font-weight: 600;
    color: #374151;
}

.address-input-wrapper {
    position: relative;
    display: flex;
    align-items: center;
}

.address-input {
    width: 100%;
    padding: 0.75rem 3rem 0.75rem 1rem;
    border: 2px solid #d1d5db;
    border-radius: 0.5rem;
    font-size: 1rem;
    transition: all 0.2s ease;
    background: white;
}

.address-input:focus {
    outline: none;
    border-color: #6366f1;
    box-shadow: 0 0 0 3px rgba(99, 102, 241, 0.1);
}

.address-input--resolving {
    border-color: #f59e0b;
}

.address-input--success {
    border-color: #10b981;
}

.address-input--error {
    border-color: #ef4444;
}

.address-input:disabled {
    background-color: #f9fafb;
    color: #9ca3af;
    cursor: not-allowed;
}

.address-input-status {
    position: absolute;
    right: 1rem;
    top: 50%;
    transform: translateY(-50%);
}

.status-resolving {
    animation: spin 1s linear infinite;
}

@keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
}

.address-input-feedback {
    margin-top: 0.5rem;
    min-height: 1.5rem;
}

.feedback-resolving {
    color: #f59e0b;
    font-size: 0.875rem;
}

.feedback-success {
    color: #10b981;
    font-size: 0.875rem;
}

.feedback-description {
    font-weight: 500;
    margin-bottom: 0.25rem;
}

.feedback-address {
    font-family: monospace;
    font-size: 0.75rem;
    opacity: 0.8;
    word-break: break-all;
}

.feedback-error {
    color: #ef4444;
    font-size: 0.875rem;
}

.address-input-helper {
    margin-top: 0.5rem;
    font-size: 0.75rem;
    color: #6b7280;
}

/* Dark mode support */
@media (prefers-color-scheme: dark) {
    .address-input-label {
        color: #f3f4f6;
    }
    
    .address-input {
        background: #1f2937;
        border-color: #4b5563;
        color: #f3f4f6;
    }
    
    .address-input:focus {
        border-color: #8b5cf6;
        box-shadow: 0 0 0 3px rgba(139, 92, 246, 0.1);
    }
    
    .address-input:disabled {
        background-color: #111827;
        color: #6b7280;
    }
    
    .address-input-helper {
        color: #9ca3af;
    }
}
"#;