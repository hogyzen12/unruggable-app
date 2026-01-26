// src/components/pin_input.rs
use dioxus::prelude::*;
use dioxus::document::eval;

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
    
    // Initialize ALL liquid metal instances on mount (buttons + dots)
    // Using visibility toggle instead of create/dispose to prevent WebGL context loss
    use_effect(move || {
        spawn(async move {
            // Wait longer for DOM and shader components to be ready
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            
            let _ = eval(
                r#"
                // Store all instances globally
                if (!window.pinShaderInstances) {
                    window.pinShaderInstances = {
                        buttonBorders: {},
                        dotBorders: {},
                        dotFills: {},
                        initialized: false
                    };
                }
                
                // Check if all required instances are created (buttons + dot borders only)
                const checkAllCreated = () => {
                    const buttons = window.pinShaderInstances.buttonBorders;
                    const dotBorders = window.pinShaderInstances.dotBorders;
                    
                    // Check buttons 0-9 only
                    for (let i = 0; i <= 9; i++) {
                        if (!buttons[`pin-button-${i}`]) return false;
                    }
                    // Check all 6 dot borders (fills created on demand)
                    for (let i = 0; i < 6; i++) {
                        if (!dotBorders[`pin-dot-${i}`]) return false;
                    }
                    return true;
                };
                
                // Initialize all PIN screen shader instances
                const initAllPinShaders = () => {
                    if (!window.LiquidMetalCircleBorder || !window.LiquidMetalComponent) {
                        console.log('PIN: Shader components not loaded yet');
                        return false;
                    }
                    
                    let createdCount = 0;
                    
                    // Initialize button borders (0-9)
                    for (let i = 0; i <= 9; i++) {
                        const elementId = `pin-button-${i}`;
                        const elem = document.getElementById(elementId);
                        if (elem && !window.pinShaderInstances.buttonBorders[elementId]) {
                            try {
                                window.pinShaderInstances.buttonBorders[elementId] = window.LiquidMetalCircleBorder.create(elementId, {
                                    borderWidth: 3,
                                });
                                createdCount++;
                            } catch (e) {
                                console.error(`PIN: Failed button ${elementId}:`, e);
                            }
                        }
                    }
                    
                    // Initialize dot borders only (fills created on demand to stay under WebGL limit)
                    for (let i = 0; i < 6; i++) {
                        const dotId = `pin-dot-${i}`;
                        const dotElem = document.getElementById(dotId);
                        
                        if (dotElem) {
                            // Create border instance (for empty state)
                            if (!window.pinShaderInstances.dotBorders[dotId]) {
                                try {
                                    window.pinShaderInstances.dotBorders[dotId] = window.LiquidMetalCircleBorder.create(dotId, {
                                        borderWidth: 2,
                                    });
                                    createdCount++;
                                } catch (e) {
                                    console.error(`PIN: Failed dot border ${dotId}:`, e);
                                }
                            }
                            // Note: dot fills are created on demand when PIN is entered
                        }
                    }
                    
                    if (createdCount > 0) {
                        console.log(`PIN: Created ${createdCount} shader instances`);
                    }
                    
                    // Check if all are created
                    if (checkAllCreated()) {
                        window.pinShaderInstances.initialized = true;
                        console.log('PIN: All shader instances initialized successfully');
                        return true;
                    }
                    
                    return false;
                };
                
                // Aggressive retry strategy
                const tryInit = () => {
                    if (window.pinShaderInstances.initialized) return;
                    initAllPinShaders();
                };
                
                // Initial attempts with increasing delays
                tryInit();
                setTimeout(tryInit, 100);
                setTimeout(tryInit, 200);
                setTimeout(tryInit, 400);
                setTimeout(tryInit, 800);
                setTimeout(tryInit, 1200);
                setTimeout(tryInit, 2000);
                
                // Periodic check every 500ms for 5 seconds to catch stragglers
                let checkCount = 0;
                const periodicCheck = setInterval(() => {
                    checkCount++;
                    if (window.pinShaderInstances.initialized || checkCount > 10) {
                        clearInterval(periodicCheck);
                        return;
                    }
                    tryInit();
                }, 500);
                "#
            );
        });
    });
    
    // Update liquid metal PIN dots based on PIN entry
    // Swap between border and fill instances to stay under WebGL context limit
    use_effect(move || {
        let current_pin_len = pin().len();
        
        spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            
            let _ = eval(&format!(
                r#"
                const updatePinDots = () => {{
                    if (!window.pinShaderInstances) return;
                    if (!window.LiquidMetalCircleBorder || !window.LiquidMetalComponent) return;
                    
                    const pinLength = {};
                    
                    // Update all 6 dots - swap between border and fill
                    for (let i = 0; i < 6; i++) {{
                        const dotId = `pin-dot-${{i}}`;
                        const dotElem = document.getElementById(dotId);
                        if (!dotElem) continue;
                        
                        const hasBorder = !!window.pinShaderInstances.dotBorders[dotId];
                        const hasFill = !!window.pinShaderInstances.dotFills[dotId];
                        
                        if (i < pinLength) {{
                            // Should be filled - need fill, dispose border
                            if (hasBorder) {{
                                try {{
                                    window.pinShaderInstances.dotBorders[dotId].dispose();
                                    delete window.pinShaderInstances.dotBorders[dotId];
                                }} catch(e) {{}}
                            }}
                            if (!hasFill) {{
                                try {{
                                    window.pinShaderInstances.dotFills[dotId] = window.LiquidMetalComponent.create(dotId, 20);
                                }} catch(e) {{
                                    console.error(`PIN: Failed to create fill for ${{dotId}}:`, e);
                                }}
                            }}
                        }} else {{
                            // Should be empty - need border, dispose fill
                            if (hasFill) {{
                                try {{
                                    window.pinShaderInstances.dotFills[dotId].dispose();
                                    delete window.pinShaderInstances.dotFills[dotId];
                                }} catch(e) {{}}
                            }}
                            if (!hasBorder) {{
                                try {{
                                    window.pinShaderInstances.dotBorders[dotId] = window.LiquidMetalCircleBorder.create(dotId, {{
                                        borderWidth: 2,
                                    }});
                                }} catch(e) {{
                                    console.error(`PIN: Failed to create border for ${{dotId}}:`, e);
                                }}
                            }}
                        }}
                    }}
                }};
                
                updatePinDots();
                "#,
                current_pin_len
            ));
        });
    });
    
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
                
                // PIN dots display with liquid metal
                div {
                    class: "pin-dots-container",
                    for i in 0..pin_length {
                        div {
                            id: format!("pin-dot-{}", i),
                            class: "pin-dot-liquid-metal",
                            style: "width: 24px; height: 24px; position: relative; border-radius: 50%;"
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
                                    let button_id = format!("pin-button-{}", digit);
                                    rsx! {
                                        div {
                                            id: "{button_id}",
                                            class: "pin-button-wrapper",
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
                    }
                    
                    // Bottom row: Cancel / 0 / Backspace
                    div {
                        class: "pin-number-row",
                        
                        if has_cancel {
                            div {
                                id: "pin-button-cancel",
                                class: "pin-button-wrapper",
                                button {
                                    class: "pin-action-button pin-cancel-button",
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
                                class: "pin-number-button",
                                onclick: move |_| handle_digit('0'),
                                "0"
                            }
                        }
                        
                        if !pin().is_empty() {
                            div {
                                id: "pin-button-backspace",
                                class: "pin-button-wrapper",
                                button {
                                    class: "pin-action-button",
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