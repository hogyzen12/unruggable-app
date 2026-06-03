// src/components/modals/currency_modal.rs
use crate::currency::{
    fetch_exchange_rates, get_supported_currencies, save_currency_to_storage, EXCHANGE_RATES,
    SELECTED_CURRENCY,
};
use dioxus::prelude::*;

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
                class: "modal-content app-modal-shell app-modal-shell-scrollable currency-modal",
                onclick: move |e| e.stop_propagation(),

                div {
                    class: "app-modal-header",
                    h2 { class: "app-modal-title", "Select Currency" }
                    button {
                        class: "app-modal-close-button",
                        onclick: move |_| onclose.call(()),
                        "×"
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
                    class: "currency-modal-actions",
                    button {
                        class: "button-standard secondary",
                        onclick: refresh_rates,
                        disabled: loading(),
                        if loading() {
                            "Refreshing..."
                        } else {
                            "Refresh Rates"
                        }
                    }
                }
            }
        }
    }
}
