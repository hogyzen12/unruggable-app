use dioxus::prelude::*;
use solana_sdk::message::VersionedMessage;
use solana_sdk::transaction::VersionedTransaction;

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android"), not(target_os = "ios")))]
use crate::bridge::PendingBridgeRequest;
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android"), not(target_os = "ios")))]
use crate::bridge::protocol::BridgeRequest;

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android"), not(target_os = "ios")))]
#[derive(Clone, Debug)]
struct TxPreview {
    instruction_count: usize,
    account_count: usize,
    blockhash: String,
    byte_size: usize,
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android"), not(target_os = "ios")))]
fn decode_transaction_preview(encoded: &str) -> Result<TxPreview, String> {
    let tx_bytes = bs58::decode(encoded)
        .into_vec()
        .map_err(|e| format!("Failed to decode transaction: {}", e))?;

    let tx: VersionedTransaction = bincode::deserialize(&tx_bytes)
        .map_err(|e| format!("Failed to parse transaction: {}", e))?;

    let (instruction_count, account_count, blockhash) = match &tx.message {
        VersionedMessage::Legacy(message) => (
            message.instructions.len(),
            message.account_keys.len(),
            message.recent_blockhash.to_string(),
        ),
        VersionedMessage::V0(message) => (
            message.instructions.len(),
            message.account_keys.len(),
            message.recent_blockhash.to_string(),
        ),
    };

    Ok(TxPreview {
        instruction_count,
        account_count,
        blockhash,
        byte_size: tx_bytes.len(),
    })
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android"), not(target_os = "ios")))]
#[component]
pub fn BridgeSignModal(
    request: PendingBridgeRequest,
    onapprove: EventHandler<u64>,
    onreject: EventHandler<u64>,
) -> Element {
    let mut preview = use_signal(|| None as Option<TxPreview>);
    let mut decode_error = use_signal(|| None as Option<String>);

    {
        let request_clone = request.clone();
        use_effect(move || {
            match &request_clone.request {
                BridgeRequest::SignTransaction { transaction, .. }
                | BridgeRequest::SignAndSendTransaction { transaction, .. } => {
                    match decode_transaction_preview(transaction) {
                        Ok(value) => preview.set(Some(value)),
                        Err(err) => decode_error.set(Some(err)),
                    }
                }
                _ => {}
            }
        });
    }

    let (title, description, primary_label) = match request.request {
        BridgeRequest::SignTransaction { .. } => (
            "Approve Transaction",
            "This transaction will be signed by your desktop wallet.",
            "Approve & Sign",
        ),
        BridgeRequest::SignAndSendTransaction { .. } => (
            "Approve Transaction",
            "This transaction will be signed and sent from your desktop wallet.",
            "Approve & Send",
        ),
        BridgeRequest::SignMessage { .. } => (
            "Approve Message Signature",
            "This message will be signed by your desktop wallet.",
            "Approve & Sign",
        ),
        _ => ("Approve Request", "Review the request before approving.", "Approve"),
    };

    rsx! {
        div {
            class: "modal-backdrop",

            div {
                class: "modal-content",
                style: "max-width: 520px;",
                onclick: move |e| e.stop_propagation(),

                div {
                    style: "display: flex; justify-content: space-between; align-items: center; padding: 18px 24px 8px;",
                    h2 {
                        style: "color: #f8fafc; font-size: 20px; font-weight: 700; margin: 0;",
                        "{title}"
                    }
                    button {
                        style: "background: none; border: none; color: white; font-size: 26px; cursor: pointer;",
                        onclick: move |_| onreject.call(request.id),
                        "×"
                    }
                }

                div {
                    class: "info-message",
                    "{description}"
                }

                div {
                    style: "padding: 0 24px 16px;",
                    div {
                        style: "margin-bottom: 8px; color: #cbd5f5; font-size: 14px;",
                        "Origin"
                    }
                    div {
                        style: "color: #f8fafc; font-weight: 600; font-size: 15px; word-break: break-all;",
                        "{request.origin}"
                    }
                }

                if let BridgeRequest::SignTransaction { .. } = request.request {
                    div {
                        style: "padding: 0 24px 16px;",
                        if let Some(err) = decode_error() {
                            div {
                                style: "color: #fca5a5; font-size: 13px;",
                                "Unable to decode transaction: {err}"
                            }
                        } else if let Some(info) = preview() {
                            div {
                                style: "display: grid; gap: 10px; background: rgba(15, 23, 42, 0.6); padding: 12px; border-radius: 12px;",
                                div { style: "color: #cbd5f5; font-size: 13px;", "Instructions: {info.instruction_count}" }
                                div { style: "color: #cbd5f5; font-size: 13px;", "Accounts: {info.account_count}" }
                                div { style: "color: #cbd5f5; font-size: 13px; word-break: break-all;", "Blockhash: {info.blockhash}" }
                                div { style: "color: #cbd5f5; font-size: 13px;", "Size: {info.byte_size} bytes" }
                            }
                        } else {
                            div {
                                style: "color: #cbd5f5; font-size: 13px;",
                                "Parsing transaction details..."
                            }
                        }
                    }
                }

                if let BridgeRequest::SignAndSendTransaction { .. } = request.request {
                    div {
                        style: "padding: 0 24px 16px;",
                        if let Some(err) = decode_error() {
                            div {
                                style: "color: #fca5a5; font-size: 13px;",
                                "Unable to decode transaction: {err}"
                            }
                        } else if let Some(info) = preview() {
                            div {
                                style: "display: grid; gap: 10px; background: rgba(15, 23, 42, 0.6); padding: 12px; border-radius: 12px;",
                                div { style: "color: #cbd5f5; font-size: 13px;", "Instructions: {info.instruction_count}" }
                                div { style: "color: #cbd5f5; font-size: 13px;", "Accounts: {info.account_count}" }
                                div { style: "color: #cbd5f5; font-size: 13px; word-break: break-all;", "Blockhash: {info.blockhash}" }
                                div { style: "color: #cbd5f5; font-size: 13px;", "Size: {info.byte_size} bytes" }
                            }
                        } else {
                            div {
                                style: "color: #cbd5f5; font-size: 13px;",
                                "Parsing transaction details..."
                            }
                        }
                    }
                }

                if let BridgeRequest::SignMessage { message, .. } = &request.request {
                    div {
                        style: "padding: 0 24px 16px;",
                        div {
                            style: "color: #cbd5f5; font-size: 13px; margin-bottom: 6px;",
                            "Message (base58)"
                        }
                        div {
                            style: "color: #f8fafc; font-size: 12px; word-break: break-all; background: rgba(15, 23, 42, 0.6); padding: 10px; border-radius: 12px;",
                            "{message}"
                        }
                    }
                }

                div {
                    style: "display: flex; gap: 12px; justify-content: flex-end; padding: 0 24px 20px;",
                    button {
                        class: "button-secondary",
                        onclick: move |_| onreject.call(request.id),
                        "Reject"
                    }
                    button {
                        class: "button-primary",
                        onclick: move |_| onapprove.call(request.id),
                        "{primary_label}"
                    }
                }
            }
        }
    }
}

#[cfg(not(all(not(target_arch = "wasm32"), not(target_os = "android"), not(target_os = "ios"))))]
#[component]
pub fn BridgeSignModal(
    _request: (),
    _onapprove: EventHandler<u64>,
    _onreject: EventHandler<u64>,
) -> Element {
    rsx! { div {} }
}
