use dioxus::prelude::*;
use dioxus::document::eval;

/// Visual status states for the liquid metal hardware button.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Neutral,  // No hardware present
    Warn,     // Hardware detected
    Ok,       // Hardware connected
}

impl Status {
    pub fn class(self) -> &'static str {
        match self {
            Status::Neutral => "is-neutral",
            Status::Warn => "is-warn",
            Status::Ok => "is-ok",
        }
    }

    pub fn aria(self) -> &'static str {
        match self {
            Status::Neutral => "no hardware",
            Status::Warn => "hardware detected",
            Status::Ok => "hardware connected",
        }
    }
    
    pub fn border_width(self) -> u32 {
        match self {
            Status::Neutral => 0,
            Status::Warn => 3,
            Status::Ok => 5,
        }
    }
}

const ICON_SVG: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@solana-3x-tpu-test/assets/icons/unruggable_icon.svg";

/// Hardware button with conditional liquid metal border.
#[component]
pub fn LiquidMetalButton(
    status: Status,
    #[props(default = true)] animated: bool,
    #[props(default = true)] interactive: bool,
    #[props(optional)] onclick: Option<EventHandler<Event<MouseData>>>,
    #[props(optional)] aria_label: Option<String>,
    #[props(optional)] class: Option<String>,
    #[props(optional)] style: Option<String>,
    children: Element,
) -> Element {
    let button_id = use_signal(|| format!("hw-btn-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()));
    
    let mut class_name = String::from("liquid-metal-hardware-button ");
    class_name.push_str(status.class());
    if let Some(extra) = class.as_ref() {
        if !extra.is_empty() {
            class_name.push(' ');
            class_name.push_str(extra);
        }
    }
    
    // Simple border initialization based on status
    let button_id_clone = button_id();
    let border_width = status.border_width();
    
    use_effect(move || {
        let id = button_id_clone.clone();
        
        spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            
            if border_width > 0 {
                let _ = eval(&format!(
                    r#"
                    if (window.LiquidMetalCircleBorder && document.getElementById('{}')) {{
                        try {{
                            if (window.hardwareButtonBorder) {{
                                window.hardwareButtonBorder.dispose();
                            }}
                            window.hardwareButtonBorder = window.LiquidMetalCircleBorder.create('{}', {{
                                borderWidth: {},
                            }});
                        }} catch (e) {{
                            console.error('Hardware button border error:', e);
                        }}
                    }}
                    "#,
                    id, id, border_width
                ));
            } else {
                let _ = eval(
                    r#"
                    if (window.hardwareButtonBorder) {
                        window.hardwareButtonBorder.dispose();
                        window.hardwareButtonBorder = null;
                    }
                    "#,
                );
            }
        });
    });

    let handler = onclick.clone();
    let clickable = interactive;
    let label = aria_label.unwrap_or_else(|| format!("Hardware wallet: {}", status.aria()));
    let style_attr = style.unwrap_or_default();

    rsx! {
        div {
            id: "{button_id}",
            class: "liquid-metal-button-container {class_name}",
            style: "position: relative; width: 48px; height: 48px; border-radius: 50%; {style_attr}",
            
            button {
                class: "hardware-button-inner",
                style: "position: absolute; inset: 0; background: transparent; border: none; border-radius: 50%; cursor: pointer; display: flex; align-items: center; justify-content: center; padding: 8px;",
                r#type: "button",
                disabled: {!clickable},
                "aria-label": "{label}",
                onclick: move |evt: Event<MouseData>| {
                    evt.stop_propagation();
                    if !clickable {
                        return;
                    }
                    if let Some(cb) = handler.as_ref() {
                        cb.call(evt);
                    }
                },
                
                img {
                    src: ICON_SVG,
                    alt: "Hardware wallet",
                    style: "width: 100%; height: 100%; object-fit: contain; filter: drop-shadow(0 0 2px rgba(255,255,255,0.3));"
                }
            }
        }
    }
}