// src/components/modals/currency_modal.rs
use dioxus::prelude::*;
use crate::currency::{
    get_supported_currencies, 
    SELECTED_CURRENCY, 
    EXCHANGE_RATES,
    save_currency_to_storage,
    fetch_exchange_rates,
    CurrencyInfo
};

#[component]
pub fn CurrencyModal(onclose: EventHandler<()>) -> Element {
    let mut loading = use_signal(|| false);
    let mut error_message = use_signal(|| None as Option<String>);
    let currencies = get_supported_currencies();
    let current_currency = SELECTED_CURRENCY.read().clone();
    let exchange_rates = EXCHANGE_RATES.read().clone();
    
    // Function to handle currency selection
    let handle_currency_selection = move |currency_code: String| {
        // Update global state
        *SELECTED_CURRENCY.write() = currency_code.clone();
        
        // Save to storage
        save_currency_to_storage(&currency_code);
        
        // Close modal
        onclose.call(());
    };
    
    // Function to refresh exchange rates
    let refresh_rates = move |_| {
        loading.set(true);
        error_message.set(None);
        
        spawn(async move {
            match fetch_exchange_rates().await {
                Ok(rates) => {
                    *EXCHANGE_RATES.write() = rates;
                    loading.set(false);
                    error_message.set(None);
                }
                Err(e) => {
                    loading.set(false);
                    error_message.set(Some(format!("Failed to update rates: {}", e)));
                }
            }
        });
    };
    
    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| onclose.call(()),
            
            div {
                class: "modal-content currency-modal",
                onclick: move |e| e.stop_propagation(),
                
                div {
                    class: "modal-header",
                    h2 { class: "modal-title", "Select Currency" }
                    button {
                        class: "refresh-button",
                        onclick: refresh_rates,
                        disabled: loading(),
                        title: "Refresh exchange rates",
                        if loading() {
                            "🔄"
                        } else {
                            "🔃"
                        }
                    }
                }
                
                // Show error if any
                if let Some(error) = error_message() {
                    div {
                        class: "error-message",
                        "{error}"
                    }
                }
                
                // Loading indicator
                if loading() {
                    div {
                        class: "loading-indicator",
                        "Updating exchange rates..."
                    }
                }
                
                div {
                    class: "currency-list",
                    for currency in currencies {
                        {
                            let is_selected = currency.code == current_currency;
                            let rate = exchange_rates.get(&currency.code).unwrap_or(&1.0);
                            let currency_code = currency.code.clone();
                            
                            rsx! {
                                button {
                                    class: if is_selected { 
                                        "currency-item selected" 
                                    } else { 
                                        "currency-item" 
                                    },
                                    onclick: move |_| {
                                        handle_currency_selection(currency_code.clone());
                                    },
                                    
                                    div {
                                        class: "currency-info",
                                        div {
                                            class: "currency-symbol",
                                            "{currency.symbol}"
                                        }
                                        div {
                                            class: "currency-details",
                                            div { class: "currency-code", "{currency.code}" }
                                            div { class: "currency-name", "{currency.name}" }
                                        }
                                    }
                                    
                                    div {
                                        class: "currency-rate",
                                        if currency.code == "USD" {
                                            span { class: "base-currency", "Base" }
                                        } else {
                                            span { 
                                                class: "rate-value",
                                                "1 USD = {rate:.4} {currency.code}"
                                            }
                                        }
                                    }
                                    
                                    if is_selected {
                                        div {
                                            class: "selected-indicator",
                                            "✓"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                
                div {
                    class: "modal-footer",
                    div {
                        class: "rate-info",
                        "Exchange rates from Pyth Network"
                    }
                    
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