use dioxus::prelude::*;
use crate::storage::{save_jito_settings_to_storage, load_jito_settings_from_storage, JitoSettings};

#[component]
pub fn JitoModal(current_settings: JitoSettings, onclose: EventHandler<()>, onsave: EventHandler<JitoSettings>) -> Element {
    let mut jito_tx = use_signal(|| current_settings.jito_tx);
    let mut jito_bundles = use_signal(|| current_settings.jito_bundles);
    
    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| onclose.call(()),
            
            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                
                h2 { class: "modal-title", "Jito Settings" }
                
                div {
                    class: "info-message",
                    "Jito MEV protection settings for your transactions. Only one option can be active at a time."
                }
                
                div {
                    class: "toggle-section",
                    
                    // JitoTx Option
                    div {
                        class: "toggle-item",
                        // Left side with label and description
                        div {
                            class: "toggle-item-content",
                            div {
                                class: "toggle-label",
                                "JitoTx"
                            }
                            div {
                                class: "toggle-description",
                                "Send transactions through Jito for MEV protection (recommended)"
                            }
                        }
                        // Right side with toggle switch
                        label {
                            class: "toggle-switch",
                            input {
                                r#type: "checkbox",
                                checked: jito_tx(),
                                oninput: move |_| {
                                    jito_tx.set(!jito_tx());
                                    // If enabling JitoTx, disable JitoBundles
                                    if jito_tx() && jito_bundles() {
                                        jito_bundles.set(false);
                                    }
                                }
                            }
                            span { class: "toggle-slider" }
                        }
                    }
                    
                    // JitoBundles Option
                    div {
                        class: "toggle-item",
                        // Left side with label and description
                        div {
                            class: "toggle-item-content",
                            div {
                                class: "toggle-label",
                                "JitoBundles"
                            }
                            div {
                                class: "toggle-description",
                                "Use Jito bundles for advanced transaction bundling (experimental)"
                            }
                        }
                        // Right side with toggle switch
                        label {
                            class: "toggle-switch",
                            input {
                                r#type: "checkbox",
                                checked: jito_bundles(),
                                oninput: move |_| {
                                    jito_bundles.set(!jito_bundles());
                                    // If enabling JitoBundles, disable JitoTx
                                    if jito_bundles() && jito_tx() {
                                        jito_tx.set(false);
                                    }
                                }
                            }
                            span { class: "toggle-slider" }
                        }
                    }
                }
                
                div { class: "modal-buttons",
                    button {
                        class: "modal-button cancel",
                        onclick: move |_| onclose.call(()),
                        "Cancel"
                    }
                    button {
                        class: "modal-button primary",
                        onclick: move |_| {
                            let settings = JitoSettings {
                                jito_tx: jito_tx(),
                                jito_bundles: jito_bundles(),
                            };
                            onsave.call(settings);
                        },
                        "Save"
                    }
                }
            }
        }
    }
}