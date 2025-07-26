// src/components/modals/background_modal.rs
use dioxus::prelude::*;
use crate::components::background_themes::BackgroundTheme;

#[component]
pub fn BackgroundModal(
    current_background: BackgroundTheme,
    onclose: EventHandler<()>,
    onselect: EventHandler<BackgroundTheme>,
) -> Element {
    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| onclose.call(()),
            
            div {
                class: "modal-content background-selector-modal",
                onclick: move |e| e.stop_propagation(),
                
                h2 {
                    class: "modal-title",
                    "Choose Your Background"
                }
                
                div {
                    class: "background-grid",
                    for theme in BackgroundTheme::get_presets() {
                        div {
                            class: if theme.url == current_background.url { "background-option selected" } else { "background-option" },
                            onclick: {
                                let theme = theme.clone();
                                move |_| {
                                    onselect.call(theme.clone());
                                }
                            },
                            
                            div {
                                class: "background-preview",
                                style: "background-image: url('{theme.url}'); background-size: cover; background-position: center;",
                            }
                            
                            div {
                                class: "background-info",
                                h4 { "{theme.name}" }
                                p { "{theme.description}" }
                            }
                            
                            if theme.url == current_background.url {
                                div {
                                    class: "selected-indicator",
                                    "✓"
                                }
                            }
                        }
                    }
                }
                
                div {
                    class: "modal-buttons",
                    button {
                        class: "modal-button cancel",
                        onclick: move |_| onclose.call(()),
                        "Close"
                    }
                }
            }
        }
    }
}