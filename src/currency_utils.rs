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

/// Format token amounts with smart abbreviations and 5-character limit
pub fn format_token_amount(amount: f64, symbol: &str) -> String {
    // Handle zero or very small amounts
    if amount == 0.0 {
        return format!("0 {}", symbol);
    }
    
    if amount < 0.000001 {
        return format!("~0 {}", symbol);
    }
    
    // For amounts >= 1 billion, use B suffix
    if amount >= 1_000_000_000.0 {
        let value = amount / 1_000_000_000.0;
        if value >= 100.0 {
            return format!("{}B {}", (value as i32), symbol); // e.g., "123B SOL"
        } else if value >= 10.0 {
            return format!("{:.0}B {}", value, symbol); // e.g., "12B SOL"
        } else {
            return format!("{:.1}B {}", value, symbol); // e.g., "1.2B SOL"
        }
    }
    
    // For amounts >= 1 million, use M suffix
    if amount >= 1_000_000.0 {
        let value = amount / 1_000_000.0;
        if value >= 100.0 {
            return format!("{}M {}", (value as i32), symbol); // e.g., "123M BONK"
        } else if value >= 10.0 {
            return format!("{:.0}M {}", value, symbol); // e.g., "12M BONK"
        } else {
            return format!("{:.1}M {}", value, symbol); // e.g., "1.2M BONK"
        }
    }
    
    // For amounts >= 1 thousand, use K suffix
    if amount >= 1_000.0 {
        let value = amount / 1_000.0;
        if value >= 100.0 {
            return format!("{}K {}", (value as i32), symbol); // e.g., "123K JUP"
        } else if value >= 10.0 {
            return format!("{:.0}K {}", value, symbol); // e.g., "12K JUP"
        } else {
            return format!("{:.1}K {}", value, symbol); // e.g., "1.2K JUP"
        }
    }
    
    // For amounts >= 100, show whole numbers
    if amount >= 100.0 {
        return format!("{:.0} {}", amount, symbol); // e.g., "150 USDC"
    }
    
    // For amounts >= 10, show 1 decimal place
    if amount >= 10.0 {
        return format!("{:.1} {}", amount, symbol); // e.g., "12.5 SOL"
    }
    
    // For amounts >= 1, show 2 decimal places
    if amount >= 1.0 {
        return format!("{:.2} {}", amount, symbol); // e.g., "9.53 JTO"
    }
    
    // For amounts < 1, show up to 4 decimal places but trim trailing zeros
    if amount >= 0.01 {
        return format!("{:.2} {}", amount, symbol); // e.g., "0.12 SOL"
    }
    
    if amount >= 0.001 {
        return format!("{:.3} {}", amount, symbol); // e.g., "0.001 BTC"
    }
    
    // For very small amounts, show 4 decimal places
    format!("{:.4} {}", amount, symbol) // e.g., "0.0001 ETH"
}

/// Format token value in USD with smart formatting and length limits
pub fn format_token_value_smart(token_amount: f64, token_usd_price: f64) -> String {
    let usd_value = token_amount * token_usd_price;
    
    // Handle zero value
    if usd_value == 0.0 {
        return "$0".to_string();
    }
    
    // Get currency symbol (could be $, €, £, etc.)
    let symbol = get_current_currency_symbol();
    let converted_value = convert_from_usd(usd_value, &SELECTED_CURRENCY.read());
    
    // For very large amounts, use B/M/K abbreviations
    if converted_value >= 1_000_000_000.0 {
        let value = converted_value / 1_000_000_000.0;
        if value >= 100.0 {
            return format!("{}{}B", symbol, (value as i32)); // e.g., "$123B"
        } else if value >= 10.0 {
            return format!("{}{:.0}B", symbol, value); // e.g., "$12B"
        } else {
            return format!("{}{:.1}B", symbol, value); // e.g., "$1.2B"
        }
    }
    
    if converted_value >= 1_000_000.0 {
        let value = converted_value / 1_000_000.0;
        if value >= 100.0 {
            return format!("{}{}M", symbol, (value as i32)); // e.g., "$123M"
        } else if value >= 10.0 {
            return format!("{}{:.0}M", symbol, value); // e.g., "$12M"
        } else {
            return format!("{}{:.1}M", symbol, value); // e.g., "$1.2M"
        }
    }
    
    if converted_value >= 1_000.0 {
        let value = converted_value / 1_000.0;
        if value >= 100.0 {
            return format!("{}{}K", symbol, (value as i32)); // e.g., "$123K"
        } else if value >= 10.0 {
            return format!("{}{:.0}K", symbol, value); // e.g., "$12K"
        } else {
            return format!("{}{:.1}K", symbol, value); // e.g., "$1.2K"
        }
    }
    
    // For smaller amounts, show appropriate precision
    if converted_value >= 100.0 {
        return format!("{}{:.0}", symbol, converted_value); // e.g., "$150"
    }
    
    if converted_value >= 10.0 {
        return format!("{}{:.1}", symbol, converted_value); // e.g., "$19.1"
    }
    
    if converted_value >= 1.0 {
        return format!("{}{:.2}", symbol, converted_value); // e.g., "$4.49"
    }
    
    if converted_value >= 0.01 {
        return format!("{}{:.2}", symbol, converted_value); // e.g., "$0.12"
    }
    
    // For very small amounts
    format!("{}~0", symbol) // e.g., "$~0"
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