// src/components/pin_input.rs
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct PinInputProps {
    pub on_complete: EventHandler<String>,
    pub on_cancel: Option<EventHandler<()>>,
    pub title: String,
    pub subtitle: Option<String>,
    pub error_message: Option<String>,
    pub show_strength: Option<bool>,
    pub step_indicator: Option<String>,
    pub clear_on_complete: Option<bool>,
}

#[component]
pub fn PinInput(props: PinInputProps) -> Element {
    let mut pin = use_signal(|| String::new());
    let mut submitted = use_signal(|| false);
    let pin_length = 6;
    
    // Calculate PIN strength
    let pin_strength = {
        let pin_str = pin();
        if pin_str.is_empty() {
            ("", "")
        } else if pin_str.len() < 3 {
            ("weak", "Weak")
        } else if pin_str.chars().collect::<std::collections::HashSet<_>>().len() < 3 {
            ("weak", "Too repetitive")
        } else if pin_str == "123456" || pin_str == "000000" || pin_str == "111111" {
            ("weak", "Too common")
        } else if pin_str.chars().collect::<std::collections::HashSet<_>>().len() < 4 {
            ("medium", "Fair")
        } else {
            ("strong", "Strong")
        }
    };
    
    let on_complete = props.on_complete.clone();
    let clear_on_complete = props.clear_on_complete.unwrap_or(false);
    
    let mut handle_digit = move |digit: char| {
        if submitted() {
            return; // Prevent input after submission
        }
        
        let current_pin = pin();
        if current_pin.len() < pin_length {
            let new_pin = format!("{}{}", current_pin, digit);
            pin.set(new_pin.clone());
            
            // Auto-submit when complete
            if new_pin.len() == pin_length {
                submitted.set(true);
                on_complete.call(new_pin.clone());
                
                // Only clear if explicitly requested
                if clear_on_complete {
                    // Small delay before clearing for visual feedback
                    spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        pin.set(String::new());
                        submitted.set(false);
                    });
                }
            }
        }
    };
    
    let handle_backspace = move |_| {
        if submitted() {
            submitted.set(false); // Allow editing again
        }
        let current_pin = pin();
        if !current_pin.is_empty() {
            pin.set(current_pin[..current_pin.len()-1].to_string());
        }
    };
    
    let has_cancel = props.on_cancel.is_some();
    let on_cancel_clone = props.on_cancel.clone();
    
    let _handle_clear = move |_: dioxus::events::MouseEvent| {
        pin.set(String::new());
        submitted.set(false);
    };
    
    rsx! {
        div {
            class: "pin-input-overlay",
            
            div {
                class: "pin-input-container",
                
                // Step indicator
                if let Some(step) = &props.step_indicator {
                    div {
                        class: "pin-step-indicator",
                        "{step}"
                    }
                }
                
                h2 { 
                    class: "pin-input-title",
                    "{props.title}"
                }
                
                if let Some(subtitle) = &props.subtitle {
                    p { 
                        class: "pin-input-subtitle",
                        "{subtitle}"
                    }
                }
                
                // PIN dots display with animation
                div {
                    class: "pin-dots-container",
                    for i in 0..pin_length {
                        div {
                            class: if i < pin().len() { 
                                if i == pin().len() - 1 && !submitted() {
                                    "pin-dot filled just-added"
                                } else {
                                    "pin-dot filled"
                                }
                            } else { 
                                "pin-dot" 
                            },
                            // Add subtle animation delay
                            style: format!("animation-delay: {}ms", i * 50)
                        }
                    }
                }
                
                // PIN strength indicator - always reserve space when show_strength is true
                if props.show_strength.unwrap_or(false) {
                    div {
                        class: if !pin().is_empty() {
                            format!("pin-strength pin-strength-{}", pin_strength.0)
                        } else {
                            "pin-strength pin-strength-placeholder".to_string()
                        },
                        if !pin_strength.1.is_empty() {
                            "{pin_strength.1}"
                        } else {
                            "\u{00A0}" // Non-breaking space to maintain height
                        }
                    }
                }
                
                // Success checkmark when submitted
                if submitted() && props.error_message.is_none() {
                    div {
                        class: "pin-success-indicator",
                        "✓"
                    }
                }
                
                // Error message
                if let Some(error) = &props.error_message {
                    div {
                        class: "pin-error-message shake-animation",
                        "{error}"
                    }
                }
                
                // Number pad with enhanced interactions
                div {
                    class: "pin-number-pad",
                    
                    // Rows 1-3
                    for row in 0..3 {
                        div {
                            class: "pin-number-row",
                            for col in 0..3 {
                                {
                                    let digit = (row * 3 + col + 1).to_string();
                                    let digit_char = digit.chars().next().unwrap();
                                    rsx! {
                                        button {
                                            class: "pin-number-button",
                                            onclick: move |_| handle_digit(digit_char),
                                            onmousedown: move |_| {
                                                // Visual feedback on press
                                            },
                                            "{digit}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    // Bottom row: Cancel / 0 / Backspace
                    div {
                        class: "pin-number-row",
                        
                        if has_cancel {
                            button {
                                class: "pin-action-button pin-cancel-button",
                                onclick: move |_| {
                                    if let Some(ref cancel) = on_cancel_clone {
                                        cancel.call(());
                                    }
                                },
                                "×"
                            }
                        } else {
                            div { class: "pin-spacer" }
                        }
                        
                        button {
                            class: "pin-number-button",
                            onclick: move |_| handle_digit('0'),
                            "0"
                        }
                        
                        if !pin().is_empty() {
                            button {
                                class: "pin-action-button",
                                onclick: handle_backspace,
                                "⌫"
                            }
                        } else {
                            div { class: "pin-spacer" }
                        }
                    }
                }
            }
        }
    }
}