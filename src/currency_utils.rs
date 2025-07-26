// src/currency_utils.rs
use dioxus::prelude::*;
use dioxus::prelude::Readable; // Add this import to fix .read() method
use crate::currency::{
    SELECTED_CURRENCY, 
    EXCHANGE_RATES, 
    convert_from_usd, 
    get_current_currency_symbol,
    format_currency_amount
};

/// Convert and format a USD price to the selected currency
pub fn format_price_in_selected_currency(usd_price: f64) -> String {
    let selected_currency = SELECTED_CURRENCY.read().clone();
    let converted_amount = convert_from_usd(usd_price, &selected_currency);
    format_currency_amount(converted_amount, &selected_currency)
}

/// Convert and format a USD price with a specific precision
pub fn format_price_with_precision(usd_price: f64, precision: usize) -> String {
    let selected_currency = SELECTED_CURRENCY.read().clone();
    let converted_amount = convert_from_usd(usd_price, &selected_currency);
    let symbol = get_current_currency_symbol();
    
    format!("{}{:.precision$}", symbol, converted_amount, precision = precision)
}

/// Format balance amount (SOL * price) in selected currency
pub fn format_balance_value(sol_amount: f64, sol_usd_price: f64) -> String {
    let usd_value = sol_amount * sol_usd_price;
    format_price_in_selected_currency(usd_value)
}

/// Format token value in selected currency
pub fn format_token_value(token_amount: f64, token_usd_price: f64) -> String {
    let usd_value = token_amount * token_usd_price;
    format_price_in_selected_currency(usd_value)
}

/// Format price change in selected currency
pub fn format_price_change(usd_change: f64) -> String {
    let selected_currency = SELECTED_CURRENCY.read().clone();
    let converted_change = convert_from_usd(usd_change, &selected_currency);
    let symbol = get_current_currency_symbol();
    
    let sign = if converted_change >= 0.0 { "+" } else { "" };
    format!("{}{}{:.2}", sign, symbol, converted_change)
}

/// Get current currency code for display
pub fn get_current_currency_code() -> String {
    SELECTED_CURRENCY.read().clone()
}

/// Check if exchange rates are available
pub fn are_exchange_rates_available() -> bool {
    !EXCHANGE_RATES.read().is_empty()
}

/// Get exchange rate for current currency (for debugging/display)
pub fn get_current_exchange_rate() -> f64 {
    let current_currency = SELECTED_CURRENCY.read().clone();
    let rates = EXCHANGE_RATES.read();
    *rates.get(&current_currency).unwrap_or(&1.0)
}

/// Format percentage change (doesn't need currency conversion)
pub fn format_percentage_change(percentage: f64) -> String {
    let sign = if percentage >= 0.0 { "+" } else { "" };
    format!("{}{:.2}%", sign, percentage)
}

/// Format large numbers with appropriate abbreviations (K, M, B)
pub fn format_large_currency_amount(usd_amount: f64) -> String {
    let selected_currency = SELECTED_CURRENCY.read().clone();
    let converted_amount = convert_from_usd(usd_amount, &selected_currency);
    let symbol = get_current_currency_symbol();
    
    let (value, suffix) = if converted_amount >= 1_000_000_000.0 {
        (converted_amount / 1_000_000_000.0, "B")
    } else if converted_amount >= 1_000_000.0 {
        (converted_amount / 1_000_000.0, "M")
    } else if converted_amount >= 1_000.0 {
        (converted_amount / 1_000.0, "K")
    } else {
        (converted_amount, "")
    };
    
    if suffix.is_empty() {
        format!("{}{:.2}", symbol, value)
    } else {
        format!("{}{:.1}{}", symbol, value, suffix)
    }
}

/// Create a currency context provider hook for components
pub fn use_currency_context() -> (String, String, f64) {
    let currency_code = SELECTED_CURRENCY.read().clone();
    let symbol = get_current_currency_symbol();
    let rate = get_current_exchange_rate();
    
    (currency_code, symbol, rate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_format_price_conversion() {
        // Set up test data
        let mut rates = HashMap::new();
        rates.insert("USD".to_string(), 1.0);
        rates.insert("EUR".to_string(), 0.85);
        *EXCHANGE_RATES.write() = rates;
        
        // Test USD (should remain the same)
        *SELECTED_CURRENCY.write() = "USD".to_string();
        assert_eq!(format_price_in_selected_currency(100.0), "$100.00");
        
        // Test EUR conversion
        *SELECTED_CURRENCY.write() = "EUR".to_string();
        let result = format_price_in_selected_currency(100.0);
        assert!(result.contains("85.00")); // 100 * 0.85
    }
    
    #[test]
    fn test_large_amount_formatting() {
        *SELECTED_CURRENCY.write() = "USD".to_string();
        let mut rates = HashMap::new();
        rates.insert("USD".to_string(), 1.0);
        *EXCHANGE_RATES.write() = rates;
        
        assert_eq!(format_large_currency_amount(1_500_000_000.0), "$1.5B");
        assert_eq!(format_large_currency_amount(2_500_000.0), "$2.5M");
        assert_eq!(format_large_currency_amount(1_500.0), "$1.5K");
        assert_eq!(format_large_currency_amount(100.0), "$100.00");
    }
}