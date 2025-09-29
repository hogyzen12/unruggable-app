use dioxus::prelude::*;
use crate::wallet::WalletInfo;

#[component]
pub fn DeleteWalletModal(
    wallet: Option<WalletInfo>,
    onconfirm: EventHandler<()>,
    onclose: EventHandler<()>
) -> Element {
    rsx! {
        div { class: "modal-backdrop",
            onclick: move |_| onclose.call(()),
            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                
                div { class: "modal-header",
                    h2 { class: "modal-title", "Delete Wallet" }
                    button {
                        class: "modal-close",
                        onclick: move |_| onclose.call(()),
                        "×"
                    }
                }
                
                div { class: "modal-body",
                    if let Some(wallet_info) = wallet {
                        div {
                            div { class: "warning-message danger",
                                "⚠️ You are about to permanently delete this wallet:"
                            }
                            div { class: "wallet-delete-info",
                                div { class: "wallet-name", "{wallet_info.name}" }
                                div { class: "wallet-address", "{wallet_info.address}" }
                            }
                            div { class: "warning-message danger",
                                "This action cannot be undone. Make sure you have backed up your private key!"
                            }
                        }
                    } else {
                        div { class: "error-message", "No wallet selected" }
                    }
                }
                
                div { class: "modal-buttons",
                    button {
                        class: "modal-button cancel",
                        onclick: move |_| onclose.call(()),
                        "Cancel"
                    }
                    button {
                        class: "modal-button primary danger",
                        onclick: move |_| onconfirm.call(()),
                        "Delete Wallet"
                    }
                }
            }
        }
    }
}