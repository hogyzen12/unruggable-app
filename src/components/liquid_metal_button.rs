use dioxus::prelude::*;

/// Visual status states for the liquid metal hardware button.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Neutral,
    Warn,
    Ok,
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
            Status::Neutral => "neutral",
            Status::Warn => "warning",
            Status::Ok => "ready",
        }
    }
}

/// Liquid metal hardware button inspired by the liquid_metal_implementation.md guide.
///
/// * `status` drives the accent colour (neutral/warn/ok)
/// * `animated` toggles idle shimmer (defaults to true)
/// * `interactive` disables the button when false
/// * `onclick` optional handler for click interactions (only fired when interactive)
/// * `aria_label` override for accessibility text (defaults to status label)
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
    let mut class_name = String::from("ball ");
    class_name.push_str(status.class());
    if animated {
        class_name.push_str(" is-animated");
    }
    if let Some(extra) = class.as_ref() {
        if !extra.is_empty() {
            class_name.push(' ');
            class_name.push_str(extra);
        }
    }

    let handler = onclick.clone();
    let clickable = interactive;
    let label = aria_label.unwrap_or_else(|| format!("Hardware wallet status: {}", status.aria()));
    let style_attr = style.unwrap_or_default();

    rsx! {
        button {
            class: "{class_name}",
            style: "{style_attr}",
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

            span { class: "core", aria_hidden: "true" }
            span { class: "rim", aria_hidden: "true" }
            span { class: "ring", aria_hidden: "true" }
            span { class: "cup", aria_hidden: "true",
                {children}
            }
            span { class: "shine", aria_hidden: "true" }
            span { class: "led", aria_hidden: "true" }
        }
    }
}