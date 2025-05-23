use dioxus::prelude::*;
use crate::rpc::{get_transaction_history, get_transaction_details, TransactionInfo};
use std::collections::HashMap;

#[component]
pub fn TransactionHistoryModal(
    address: String,
    custom_rpc: Option<String>,
    onclose: EventHandler<()>,
) -> Element {
    let mut transactions = use_signal(|| Vec::<TransactionInfo>::new());
    let mut loading = use_signal(|| true);
    let mut error = use_signal(|| None as Option<String>);
    let mut selected_tx = use_signal(|| None as Option<String>);
    let mut tx_details = use_signal(|| None as Option<HashMap<String, serde_json::Value>>);
    let mut loading_details = use_signal(|| false);
    let mut detail_error = use_signal(|| None as Option<String>);

    // Clone props for use in effects
    let address_for_effect = address.clone();
    let custom_rpc_for_effect = custom_rpc.clone();

    // Fetch transaction history when the modal opens
    use_effect(move || {
        let addr = address_for_effect.clone();
        let rpc_url = custom_rpc_for_effect.clone();
        loading.set(true);
        error.set(None);
        transactions.set(Vec::new());

        spawn(async move {
            match get_transaction_history(&addr, 20, rpc_url.as_deref()).await {
                Ok(txs) => {
                    transactions.set(txs);
                }
                Err(e) => {
                    error.set(Some(format!("Failed to load transactions: {}", e)));
                }
            }
            loading.set(false);
        });
    });

    // Clone needed for second effect
    let custom_rpc_for_detail = custom_rpc.clone();

    // Fetch transaction details when a transaction is selected
    use_effect(move || {
        if let Some(signature) = selected_tx() {
            let sig = signature.clone();
            let rpc_url = custom_rpc_for_detail.clone();
            loading_details.set(true);
            detail_error.set(None);

            spawn(async move {
                match get_transaction_details(&sig, rpc_url.as_deref()).await {
                    Ok(details) => {
                        tx_details.set(Some(details));
                    }
                    Err(e) => {
                        detail_error.set(Some(format!("Failed to load transaction details: {}", e)));
                    }
                }
                loading_details.set(false);
            });
        }
    });

    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| onclose.call(()),
            
            div {
                class: "modal-content transaction-history-modal",
                onclick: move |e| e.stop_propagation(),
                
                div {
                    class: "modal-header",
                    h2 { class: "modal-title", "Transaction History" }
                    button {
                        class: "modal-close-button",
                        onclick: move |_| onclose.call(()),
                        "×"
                    }
                }
                
                div {
                    class: "transaction-address",
                    "Address: ",
                    span { class: "address-text", "{address}" }
                }
                
                // Main content container
                div {
                    class: "transaction-content",
                    
                    // Left panel - transaction list
                    div {
                        class: "transaction-list-container",
                        
                        if loading() {
                            div { class: "loading-indicator", "Loading transactions..." }
                        } else if let Some(err) = error() {
                            div { class: "error-message", "{err}" }
                        } else if transactions().is_empty() {
                            div { class: "no-transactions", "No transactions found for this address." }
                        } else {
                            // Transaction list
                            div {
                                class: "transaction-list",
                                // Use transactions() to get a clone of the list
                                for tx in transactions() {
                                    div {
                                        key: "{tx.signature}",
                                        class: if Some(&tx.signature) == selected_tx.as_ref().as_deref() {
                                            "transaction-item selected"
                                        } else {
                                            "transaction-item"
                                        },
                                        onclick: move |_| {
                                            selected_tx.set(Some(tx.signature.clone()));
                                            tx_details.set(None);
                                        },
                                        
                                        div {
                                            class: "transaction-status-icon",
                                            class: if tx.status == "Success" { "success-icon" } else { "error-icon" },
                                            if tx.status == "Success" { "✓" } else { "✗" }
                                        }
                                        
                                        div {
                                            class: "transaction-item-content",
                                            div {
                                                class: "transaction-item-header",
                                                div {
                                                    class: "transaction-signature",
                                                    "{tx.signature.chars().take(8).collect::<String>()}...{tx.signature.chars().rev().take(4).collect::<String>().chars().rev().collect::<String>()}"
                                                }
                                                div {
                                                    class: "transaction-time",
                                                    title: "{tx.timestamp}",
                                                    "{tx.time_ago}"
                                                }
                                            }
                                            
                                            div {
                                                class: "transaction-status",
                                                span { 
                                                    class: if tx.status == "Success" { "success-status" } else { "error-status" },
                                                    "{tx.status}" 
                                                }
                                                if let Some(ref error_msg) = tx.error {
                                                    span { class: "transaction-error-message", "- {error_msg}" }
                                                }
                                            }
                                            
                                            if let Some(ref memo) = tx.memo {
                                                div { class: "transaction-memo", "Memo: {memo}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    // Right panel - transaction details
                    div {
                        class: "transaction-details-container",
                        
                        if let Some(ref signature) = selected_tx() {
                            div {
                                class: "transaction-details-header",
                                h3 { "Transaction Details" }
                                a {
                                    class: "external-link",
                                    href: "https://explorer.solana.com/tx/{signature}",
                                    target: "_blank",
                                    rel: "noopener noreferrer",
                                    "View in Explorer"
                                }
                            }
                            
                            if loading_details() {
                                div { class: "loading-indicator", "Loading details..." }
                            } else if let Some(err) = detail_error() {
                                div { class: "error-message", "{err}" }
                            } else if let Some(ref details) = tx_details() {
                                div {
                                    class: "transaction-details-content",
                                    
                                    // Basic information section
                                    div {
                                        class: "details-section",
                                        h4 { "Basic Information" }
                                        
                                        div { class: "detail-item",
                                            div { class: "detail-label", "Signature:" }
                                            div { class: "detail-value signature-value", "{signature}" }
                                        }
                                        
                                        if let Some(slot) = details.get("slot") {
                                            div { class: "detail-item",
                                                div { class: "detail-label", "Slot:" }
                                                div { class: "detail-value", "{slot}" }
                                            }
                                        }
                                        
                                        if let Some(time) = details.get("formattedTime") {
                                            div { class: "detail-item",
                                                div { class: "detail-label", "Time:" }
                                                div { class: "detail-value", "{time}" }
                                            }
                                        }
                                        
                                        if let Some(status) = details.get("status") {
                                            div { class: "detail-item",
                                                div { class: "detail-label", "Status:" }
                                                div { 
                                                    class: if status.as_str().unwrap_or("") == "Success" { 
                                                        "detail-value status-success" 
                                                    } else { 
                                                        "detail-value status-error" 
                                                    },
                                                    "{status}" 
                                                }
                                            }
                                        }
                                        
                                        if let Some(fee) = details.get("feeSOL") {
                                            div { class: "detail-item",
                                                div { class: "detail-label", "Fee:" }
                                                div { class: "detail-value", "{fee} SOL" }
                                            }
                                        }
                                    }
                                    
                                    // Error information if present
                                    if let Some(error) = details.get("error") {
                                        div {
                                            class: "details-section error-section",
                                            h4 { "Error Details" }
                                            div { class: "error-details", "{error}" }
                                        }
                                    }
                                    
                                    // Instructions section
                                    if let Some(instructions) = details.get("instructions") {
                                        div {
                                            class: "details-section",
                                            h4 { "Instructions" }
                                            
                                            if let Some(instructions_array) = instructions.as_array() {
                                                div {
                                                    class: "instructions-list",
                                                    for (i, instruction) in instructions_array.iter().enumerate() {
                                                        div {
                                                            key: "{i}",
                                                            class: "instruction-item",
                                                            h5 { "Instruction #{i+1}" }
                                                            
                                                            if let Some(program_id) = instruction.get("programId") {
                                                                div { class: "instruction-detail",
                                                                    div { class: "instruction-label", "Program:" }
                                                                    div { class: "instruction-value", "{program_id}" }
                                                                }
                                                            }
                                                            
                                                            // For parsed instructions
                                                            if let Some(parsed) = instruction.get("parsed") {
                                                                if let Some(parsed_type) = parsed.get("type") {
                                                                    div { class: "instruction-detail",
                                                                        div { class: "instruction-label", "Type:" }
                                                                        div { class: "instruction-value", "{parsed_type}" }
                                                                    }
                                                                }
                                                                
                                                                if let Some(info) = parsed.get("info") {
                                                                    div { class: "instruction-detail",
                                                                        div { class: "instruction-label", "Details:" }
                                                                        div { class: "instruction-value instruction-json", "{info}" }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            } else {
                                                div { "No instruction details available" }
                                            }
                                        }
                                    }
                                }
                            } else {
                                div { class: "no-details", "Select a transaction to view details" }
                            }
                        } else {
                            div { class: "no-transaction-selected", "Select a transaction to view details" }
                        }
                    }
                }
                
                // Footer with action buttons
                div { class: "modal-buttons",
                    button {
                        class: "modal-button primary",
                        onclick: move |_| onclose.call(()),
                        "Close"
                    }
                }
            }
        }
    }
}