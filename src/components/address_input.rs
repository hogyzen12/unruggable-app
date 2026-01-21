// src/components/address_input.rs
use dioxus::prelude::*;
use solana_sdk::pubkey::Pubkey;
use crate::sns::SnsResolver;
use crate::storage::{
    load_address_book_from_storage,
    upsert_address_book_entry,
    remove_address_book_entry,
};
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
    pub show_address_book: Option<bool>,
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
    let sns_resolver = use_context::<Arc<SnsResolver>>();
    
    let show_validation = props.show_validation.unwrap_or(true);
    let auto_resolve = props.auto_resolve.unwrap_or(false);
    let disabled = props.disabled.unwrap_or(false);
    let show_address_book = props.show_address_book.unwrap_or(false);

    let mut address_book = use_signal(|| load_address_book_from_storage());
    let mut show_book = use_signal(|| false);
    let mut label_input = use_signal(|| String::new());

    let mut latest_value = use_signal(|| props.value.clone());
    let props_on_change = props.on_change.clone();
    let props_on_resolved = props.on_resolved.clone();

    let resolve_address_handler = {
        let mut validation_state = validation_state.clone();
        let sns_resolver = sns_resolver.clone();
        let on_resolved = props_on_resolved.clone();
        
        move |input: String| {
            if input.trim().is_empty() {
                validation_state.set(ValidationState::Empty);
                on_resolved.call(None);
                return;
            }

            validation_state.set(ValidationState::Resolving);
            
            // Use the detailed resolver for better UX
            match sns_resolver.resolve_address_with_details(&input) {
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
        let mut latest_value = latest_value.clone();
        
        move |evt: FormEvent| {
            let new_value = evt.value();
            props_on_change.call(new_value.clone());
            latest_value.set(new_value.clone());
            
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
        let latest_value = latest_value.clone();
        
        move |_| {
            if !auto_resolve {
                resolve_handler(latest_value());
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
                    "You can enter a Solana address or domain like 'username.sol'"
                }
            }

            if show_address_book {
                div { class: "address-book-inline",
                    button {
                        class: "address-book-toggle",
                        onclick: move |_| show_book.set(!show_book()),
                        if show_book() { "Saved" } else { "Saved" }
                    }

                    if let ValidationState::Success(pubkey, _) = &*validation_state.read() {
                        {
                            let resolved_address = pubkey.to_string();
                            let existing_label = address_book()
                                .iter()
                                .find(|entry| entry.address == resolved_address)
                                .map(|entry| entry.label.clone());
                            let resolved_address_for_remove = resolved_address.clone();

                            rsx! {
                                if let Some(label) = existing_label {
                                    div { class: "address-book-chip",
                                        "Saved as \"{label}\""
                                    }
                                    button {
                                        class: "address-book-action",
                                        onclick: move |_| {
                                            show_book.set(true);
                                            label_input.set(label.clone());
                                        },
                                        "Edit"
                                    }
                                    button {
                                        class: "address-book-action secondary",
                                        onclick: move |_| {
                                            remove_address_book_entry(&resolved_address_for_remove);
                                            address_book.set(load_address_book_from_storage());
                                        },
                                        "Remove"
                                    }
                                } else {
                                    button {
                                        class: "address-book-action",
                                        onclick: move |_| {
                                            show_book.set(true);
                                            label_input.set(String::new());
                                        },
                                        "Save"
                                    }
                                }
                            }
                        }
                    }
                }

                if show_book() {
                    div { class: "address-book-dropdown",
                        if let ValidationState::Success(pubkey, _) = &*validation_state.read() {
                            {
                                let resolved_address = pubkey.to_string();
                                rsx! {
                                    div { class: "address-book-actions-row",
                                        input {
                                            class: "address-book-input",
                                            r#type: "text",
                                            value: "{label_input()}",
                                            placeholder: "Label this address",
                                            oninput: move |e| label_input.set(e.value()),
                                        }
                                        button {
                                            class: "address-book-button",
                                            onclick: move |_| {
                                                let label = label_input().trim().to_string();
                                                if !label.is_empty() {
                                                    upsert_address_book_entry(&resolved_address, &label);
                                                    address_book.set(load_address_book_from_storage());
                                                    label_input.set(String::new());
                                                }
                                            },
                                            "Save"
                                        }
                                    }
                                }
                            }
                        }

                        if address_book().is_empty() {
                            div { class: "address-book-empty", "No saved addresses yet." }
                        } else {
                            div { class: "address-book-list",
                                for entry in address_book().iter() {
                                    {
                                        let address = entry.address.clone();
                                        let label = entry.label.clone();
                                        let mut resolve_handler = resolve_address_handler.clone();
                                        let mut latest_value = latest_value.clone();
                                        rsx! {
                                            div {
                                                key: "{address}",
                                                class: "address-book-item",
                                                onclick: move |_| {
                                                    props_on_change.call(address.clone());
                                                    latest_value.set(address.clone());
                                                    resolve_handler(address.clone());
                                                    show_book.set(false);
                                                },
                                                div { class: "address-book-label", "{label}" }
                                                div { class: "address-book-address", "{address}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
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
    position: relative;
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

.address-book-inline {
    margin-top: 0.5rem;
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
}

.address-book-toggle {
    background: #111827;
    color: #ffffff;
    border: none;
    border-radius: 999px;
    padding: 4px 10px;
    font-size: 0.7rem;
    cursor: pointer;
}

.address-book-chip {
    background: rgba(16, 185, 129, 0.12);
    color: #065f46;
    border-radius: 999px;
    padding: 4px 10px;
    font-size: 0.75rem;
}

.address-book-action {
    background: #2563eb;
    color: #ffffff;
    border: none;
    border-radius: 6px;
    padding: 4px 8px;
    font-size: 0.7rem;
    cursor: pointer;
}

.address-book-action.secondary {
    background: #6b7280;
}

.address-book-dropdown {
    position: absolute;
    top: calc(100% + 8px);
    left: 0;
    right: 0;
    background: #ffffff;
    border-radius: 12px;
    border: 1px solid #e5e7eb;
    box-shadow: 0 12px 32px rgba(15, 23, 42, 0.16);
    padding: 12px;
    z-index: 20;
}

.address-book-actions-row {
    display: flex;
    gap: 8px;
    align-items: center;
    margin-bottom: 10px;
}

.address-book-input {
    flex: 1;
    border: 1px solid #d1d5db;
    border-radius: 8px;
    padding: 6px 8px;
    font-size: 0.85rem;
}

.address-book-button {
    background: #111827;
    color: #ffffff;
    border: none;
    border-radius: 8px;
    padding: 6px 10px;
    font-size: 0.75rem;
    cursor: pointer;
}

.address-book-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-height: 180px;
    overflow-y: auto;
}

.address-book-item {
    padding: 8px 10px;
    background: #f8fafc;
    border-radius: 10px;
    border: 1px solid #e2e8f0;
    cursor: pointer;
}

.address-book-label {
    font-weight: 600;
    font-size: 0.85rem;
    color: #0f172a;
}

.address-book-address {
    font-size: 0.75rem;
    color: #64748b;
    word-break: break-all;
}

.address-book-empty {
    font-size: 0.8rem;
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

    .address-book-toggle {
        background: #334155;
    }

    .address-book-chip {
        background: rgba(16, 185, 129, 0.2);
        color: #d1fae5;
    }

    .address-book-action {
        background: #1d4ed8;
    }

    .address-book-action.secondary {
        background: #475569;
    }

    .address-book-dropdown {
        background: #0f172a;
        border-color: #1f2937;
        box-shadow: 0 16px 40px rgba(0, 0, 0, 0.4);
    }

    .address-book-input {
        background: #1f2937;
        border-color: #374151;
        color: #f3f4f6;
    }

    .address-book-button {
        background: #2563eb;
    }

    .address-book-item {
        background: #111827;
        border-color: #1f2937;
    }

    .address-book-label {
        color: #e2e8f0;
    }

    .address-book-address {
        color: #94a3b8;
    }
}
"#;
