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
    #[props(optional)]
    pub on_input: Option<EventHandler<()>>,
    #[props(optional)]
    pub is_processing: Option<bool>,
    #[props(optional)]
    pub processing_label: Option<String>,
    #[props(optional)]
    pub reset_key: Option<String>,
}

#[component]
pub fn PinInput(props: PinInputProps) -> Element {
    let mut pin = use_signal(|| String::new());
    let mut submitted = use_signal(|| false);
    let mut submitted_pin_len = use_signal(|| 0usize);
    let pin_length = 6;
    let processing = props.is_processing.unwrap_or(false);
    let processing_label = props
        .processing_label
        .clone()
        .unwrap_or_else(|| "Processing securely...".to_string());
    let clear_on_complete = props.clear_on_complete.unwrap_or(false);

    {
        let reset_key = props.reset_key.clone();
        use_effect(move || {
            let _ = reset_key.as_deref();
            {
                let mut pin_value = pin.write();
                if !pin_value.is_empty() {
                    crate::pin::wipe_secret_string(&mut pin_value);
                }
            }
            submitted.set(false);
            submitted_pin_len.set(0);
        });
    }

    {
        let error_message = props.error_message.clone();
        use_effect(move || {
            if error_message.is_some() && !processing {
                let mut cleared_pin = pin();
                if !cleared_pin.is_empty() {
                    crate::pin::wipe_secret_string(&mut cleared_pin);
                }
                pin.set(cleared_pin);
                submitted.set(false);
                submitted_pin_len.set(0);
            }
        });
    }

    {
        let current_pin = pin();
        let is_submitted = submitted();
        use_effect(move || {
            if processing && is_submitted && !current_pin.is_empty() {
                let mut cleared_pin = current_pin.clone();
                crate::pin::wipe_secret_string(&mut cleared_pin);
                pin.set(cleared_pin);
            }
        });
    }

    // Calculate PIN strength
    let pin_strength = {
        let pin_str = pin();
        if pin_str.is_empty() {
            ("", "")
        } else if pin_str.len() < 3 {
            ("weak", "Weak")
        } else if pin_str
            .chars()
            .collect::<std::collections::HashSet<_>>()
            .len()
            < 3
        {
            ("weak", "Too repetitive")
        } else if pin_str == "123456" || pin_str == "000000" || pin_str == "111111" {
            ("weak", "Too common")
        } else if pin_str
            .chars()
            .collect::<std::collections::HashSet<_>>()
            .len()
            < 4
        {
            ("medium", "Fair")
        } else {
            ("strong", "Strong")
        }
    };

    let on_complete = props.on_complete.clone();
    let on_input_digit = props.on_input.clone();
    let on_input_backspace = props.on_input.clone();
    let has_error = props.error_message.is_some();
    let current_pin_len = if processing && submitted() {
        submitted_pin_len().max(pin_length)
    } else {
        pin().len()
    };

    let mut handle_digit = move |digit: char| {
        if processing || submitted() {
            return;
        }

        if let Some(ref on_input) = on_input_digit {
            on_input.call(());
        }

        let current_pin = pin();
        if current_pin.len() < pin_length {
            let new_pin = format!("{}{}", current_pin, digit);
            if new_pin.len() == pin_length {
                submitted_pin_len.set(new_pin.len());
                submitted.set(true);
                on_complete.call(new_pin.clone());

                if clear_on_complete {
                    let mut cleared_pin = new_pin;
                    crate::pin::wipe_secret_string(&mut cleared_pin);
                    pin.set(String::new());
                } else {
                    pin.set(new_pin);
                }
            } else {
                pin.set(new_pin);
            }
        }
    };

    let handle_backspace = move |_| {
        if processing {
            return;
        }
        if let Some(ref on_input) = on_input_backspace {
            on_input.call(());
        }
        if submitted() {
            submitted.set(false);
            submitted_pin_len.set(0);
        }
        let current_pin = pin();
        if !current_pin.is_empty() {
            pin.set(current_pin[..current_pin.len() - 1].to_string());
        }
    };

    let has_cancel = props.on_cancel.is_some();
    let on_cancel_clone = props.on_cancel.clone();

    let _handle_clear = move |_: dioxus::events::MouseEvent| {
        pin.set(String::new());
        submitted.set(false);
        submitted_pin_len.set(0);
    };

    rsx! {
        div {
            class: "pin-input-overlay",

            div {
                class: if processing {
                    "pin-input-container pin-input-container-processing"
                } else {
                    "pin-input-container"
                },

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

                // PIN dots display
                div {
                    class: "pin-dots-container",
                    for i in 0..pin_length {
                        {
                            let dot_style = if processing && i < current_pin_len {
                                format!("--pin-dot-delay: {}ms;", i * 90)
                            } else {
                                String::new()
                            };
                            let dot_class = if i < current_pin_len {
                                if processing {
                                    "pin-dot filled processing"
                                } else if i + 1 == current_pin_len {
                                    "pin-dot filled just-added"
                                } else {
                                    "pin-dot filled"
                                }
                            } else {
                                "pin-dot"
                            };
                            rsx! {
                                div {
                                    class: "{dot_class}",
                                    style: "{dot_style}",
                                }
                            }
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

                if processing {
                    div {
                        class: "pin-processing-indicator",
                        div { class: "pin-processing-spinner" }
                        span {
                            class: "pin-processing-label",
                            "{processing_label}"
                        }
                    }
                } else if submitted() && !has_error {
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
                                    let button_id = format!("pin-button-{}", digit);
                                    rsx! {
                                        div {
                                            id: "{button_id}",
                                            class: "pin-button-wrapper",
                                            button {
                                                class: if processing || submitted() {
                                                    "pin-number-button pin-button-disabled"
                                                } else {
                                                    "pin-number-button"
                                                },
                                                disabled: processing || submitted(),
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
                    }

                    // Bottom row: Cancel / 0 / Backspace
                    div {
                        class: "pin-number-row",

                        if has_cancel {
                            div {
                                id: "pin-button-cancel",
                                class: "pin-button-wrapper",
                                button {
                                    class: if processing || submitted() {
                                        "pin-action-button pin-cancel-button pin-button-disabled"
                                    } else {
                                        "pin-action-button pin-cancel-button"
                                    },
                                    disabled: processing || submitted(),
                                    onclick: move |_| {
                                        if let Some(ref cancel) = on_cancel_clone {
                                            cancel.call(());
                                        }
                                    },
                                    "×"
                                }
                            }
                        } else {
                            div { class: "pin-spacer" }
                        }

                        div {
                            id: "pin-button-0",
                            class: "pin-button-wrapper",
                            button {
                                class: if processing || submitted() {
                                    "pin-number-button pin-button-disabled"
                                } else {
                                    "pin-number-button"
                                },
                                disabled: processing || submitted(),
                                onclick: move |_| handle_digit('0'),
                                "0"
                            }
                        }

                        if !pin().is_empty() {
                            div {
                                id: "pin-button-backspace",
                                class: "pin-button-wrapper",
                                button {
                                    class: if processing {
                                        "pin-action-button pin-button-disabled"
                                    } else {
                                        "pin-action-button"
                                    },
                                    disabled: processing,
                                    onclick: handle_backspace,
                                    "⌫"
                                }
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
