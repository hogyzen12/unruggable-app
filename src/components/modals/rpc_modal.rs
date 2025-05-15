use dioxus::prelude::*;
use crate::rpc;

#[component]
pub fn RpcModal(current_rpc: Option<String>, onclose: EventHandler<()>, onsave: EventHandler<String>) -> Element {
    let mut rpc_url = use_signal(|| current_rpc.clone().unwrap_or_default());
    let mut error_message = use_signal(|| None as Option<String>);
    let mut testing = use_signal(|| false);
    
    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| onclose.call(()),
            
            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                
                h2 { class: "modal-title", "RPC Settings" }
                
                // Show error if any
                if let Some(error) = error_message() {
                    div {
                        class: "error-message",
                        "{error}"
                    }
                }
                
                div {
                    class: "wallet-field",
                    label { "RPC URL:" }
                    input {
                        value: "{rpc_url}",
                        oninput: move |e| rpc_url.set(e.value()),
                        placeholder: "https://your-rpc-url.com"
                    }
                    div {
                        class: "help-text",
                        "Leave empty to use default RPC"
                    }
                }
                
                if let Some(current) = current_rpc {
                    div {
                        class: "info-message",
                        "Current RPC: {current}"
                    }
                }
                
                div { class: "modal-buttons",
                    button {
                        class: "modal-button cancel",
                        onclick: move |_| onclose.call(()),
                        "Cancel"
                    }
                    button {
                        class: "modal-button secondary",
                        onclick: move |_| {
                            testing.set(true);
                            error_message.set(None);
                            let test_rpc = rpc_url();
                            
                            spawn(async move {
                                // Test the RPC with a known address
                                match rpc::get_balance("11111111111111111111111111111111", 
                                    if test_rpc.is_empty() { None } else { Some(&test_rpc) }).await {
                                    Ok(_) => {
                                        error_message.set(None);
                                        testing.set(false);
                                    }
                                    Err(e) => {
                                        error_message.set(Some(format!("RPC test failed: {}", e)));
                                        testing.set(false);
                                    }
                                }
                            });
                        },
                        disabled: testing(),
                        if testing() { "Testing..." } else { "Test RPC" }
                    }
                    button {
                        class: "modal-button primary",
                        onclick: move |_| {
                            onsave.call(rpc_url());
                        },
                        "Save"
                    }
                }
            }
        }
    }
}