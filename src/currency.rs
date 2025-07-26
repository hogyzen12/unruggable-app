// src/currency.rs
use dioxus::prelude::*;
use dioxus::prelude::Readable; // Add this import to fix .read() method
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use reqwest::Client;

/// Global currency state using Dioxus GlobalSignal
pub static SELECTED_CURRENCY: GlobalSignal<String> = Signal::global(|| "USD".to_string());
pub static EXCHANGE_RATES: GlobalSignal<HashMap<String, f64>> = Signal::global(HashMap::new);

/// Supported currencies with their display information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrencyInfo {
    pub code: String,
    pub name: String,
    pub symbol: String,
    pub pyth_id: Option<String>, // Pyth price feed ID for FX rates
}

/// Pyth FX API response structures
#[derive(Debug, Deserialize)]
struct PythFxResponse {
    parsed: Vec<FxPriceItem>,
}

#[derive(Debug, Deserialize)]
struct FxPriceItem {
    id: String,
    price: FxPriceData,
}

#[derive(Debug, Deserialize)]
struct FxPriceData {
    price: String,
    expo: i32,
}

/// Get all supported currencies
pub fn get_supported_currencies() -> Vec<CurrencyInfo> {
    vec![
        CurrencyInfo {
            code: "USD".to_string(),
            name: "US Dollar".to_string(),
            symbol: "$".to_string(),
            pyth_id: None, // USD is base currency
        },
        CurrencyInfo {
            code: "EUR".to_string(),
            name: "Euro".to_string(),
            symbol: "€".to_string(),
            pyth_id: Some("a995d00bb36a63cef7fd2c287dc105fc8f3d93779f062f09551b0af3e81ec30b".to_string()),
        },
        CurrencyInfo {
            code: "GBP".to_string(),
            name: "British Pound".to_string(),
            symbol: "£".to_string(),
            pyth_id: Some("84c2dde9633d93d1bcad84e7dc41c9d56578b7ec52fabedc1f335d673df0a7c1".to_string()),
        },
        CurrencyInfo {
            code: "CAD".to_string(),
            name: "Canadian Dollar".to_string(),
            symbol: "C$".to_string(),
            pyth_id: Some("3112b03a41c910ed446852aacf67118cb1bec67b2cd0b9a214c58cc0eaa2ecca".to_string()),
        },
        CurrencyInfo {
            code: "AUD".to_string(),
            name: "Australian Dollar".to_string(),
            symbol: "A$".to_string(),
            pyth_id: Some("67a6f93030420c1c9e3fe37c1ab6b77966af82f995944a9fefce357a22854a80".to_string()),
        },
        CurrencyInfo {
            code: "JPY".to_string(),
            name: "Japanese Yen".to_string(),
            symbol: "¥".to_string(),
            pyth_id: Some("ef2c98c804ba503c6a707e38be4dfbb16683775f195b091252bf24693042fd52".to_string()),
        },
        CurrencyInfo {
            code: "CHF".to_string(),
            name: "Swiss Franc".to_string(),
            symbol: "CHF".to_string(),
            pyth_id: Some("0b1e3297e69f162877b577b0d6a47a0d63b2392bc8499e6540da4187a63e28f8".to_string()),
        },
        CurrencyInfo {
            code: "CNH".to_string(),
            name: "Chinese Yuan".to_string(),
            symbol: "¥".to_string(),
            pyth_id: Some("eef52e09c878ad41f6a81803e3640fe04dceea727de894edd4ea117e2e332e66".to_string()),
        },
        CurrencyInfo {
            code: "BRL".to_string(),
            name: "Brazilian Real".to_string(),
            symbol: "R$".to_string(),
            pyth_id: Some("d2db4dbf1aea74e0f666b0e8f73b9580d407f5e5cf931940b06dc633d7a95906".to_string()),
        },
        CurrencyInfo {
            code: "MXN".to_string(),
            name: "Mexican Peso".to_string(),
            symbol: "MX$".to_string(),
            pyth_id: Some("e13b1c1ffb32f34e1be9545583f01ef385fde7f42ee66049d30570dc866b77ca".to_string()),
        },
    ]
}

/// Fetch current exchange rates from Pyth Network
pub async fn fetch_exchange_rates() -> Result<HashMap<String, f64>, Box<dyn Error>> {
    let client = Client::new();
    let currencies = get_supported_currencies();
    
    // Collect Pyth IDs for FX pairs
    let mut params = vec![];
    for currency in &currencies {
        if let Some(pyth_id) = &currency.pyth_id {
            params.push(("ids[]", pyth_id.clone()));
        }
    }
    params.push(("parsed", "true".to_string()));

    // Fetch from Pyth Hermes FX endpoint
    let response = client
        .get("https://hermes.pyth.network/v2/updates/price/latest")
        .query(&params)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch FX rates: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Pyth FX API error: {}", response.status()).into());
    }

    let fx_response: PythFxResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse FX response: {}", e))?;

    // Process exchange rates
    let mut rates = HashMap::new();
    
    // USD is always 1.0 (base currency)
    rates.insert("USD".to_string(), 1.0);

    for item in fx_response.parsed {
        // Find the currency corresponding to this price feed ID
        if let Some(currency) = currencies.iter().find(|c| 
            c.pyth_id.as_ref().map_or(false, |id| *id == item.id)
        ) {
            // Calculate the rate: price * 10^expo
            let price_value = item.price.price
                .parse::<f64>()
                .map_err(|e| format!("Failed to parse FX rate for {}: {}", currency.code, e))?;
            let rate = price_value * 10f64.powi(item.price.expo);
            
            // DEBUG: Log the raw rate and currency
            println!("Raw rate for {}: {} (from price: {}, expo: {})", 
                     currency.code, rate, price_value, item.price.expo);
            
            // Based on your API data, let's check the pair format from the descriptions:
            // Looking at the paste.txt data:
            // - EUR: "EURO / US DOLLAR" (EUR/USD) - this gives USD per EUR, so we need inverse
            // - GBP: "BRITISH POUND / US DOLLAR" (GBP/USD) - this gives USD per GBP, so we need inverse  
            // - USD/JPY: "US DOLLAR / JAPANESE YEN" - this gives JPY per USD, so direct
            // - USD/CAD: "US DOLLAR / CANADIAN DOLLAR" - this gives CAD per USD, so direct
            
            let final_rate = match currency.code.as_str() {
                // These pairs are XXX/USD (foreign currency per USD), so rate gives USD per foreign currency
                // We want foreign currency per USD, so we need the inverse
                "EUR" => 1.0 / rate,  // EUR/USD rate -> need USD/EUR rate
                "GBP" => 1.0 / rate,  // GBP/USD rate -> need USD/GBP rate  
                "AUD" => 1.0 / rate,  // AUD/USD rate -> need USD/AUD rate
                
                // These pairs are USD/XXX (USD per foreign currency), so rate gives foreign currency per USD directly
                "JPY" => rate,  // USD/JPY rate is direct
                "CAD" => rate,  // USD/CAD rate is direct
                "CHF" => rate,  // USD/CHF rate is direct
                "CNH" => rate,  // USD/CNH rate is direct
                "BRL" => rate,  // USD/BRL rate is direct
                "MXN" => rate,  // USD/MXN rate is direct
                
                _ => rate,
            };
            
            println!("Final rate for {} (1 USD = {} {}): {}", 
                     currency.code, final_rate, currency.code, final_rate);
            
            rates.insert(currency.code.clone(), final_rate);
        }
    }

    println!("Fetched exchange rates: {:?}", rates);
    Ok(rates)
}

/// Convert USD amount to selected currency
pub fn convert_from_usd(usd_amount: f64, target_currency: &str) -> f64 {
    let rates = EXCHANGE_RATES.read();
    let rate = rates.get(target_currency).unwrap_or(&1.0);
    usd_amount * rate
}

/// Convert amount from any currency to USD
pub fn convert_to_usd(amount: f64, from_currency: &str) -> f64 {
    let rates = EXCHANGE_RATES.read();
    let rate = rates.get(from_currency).unwrap_or(&1.0);
    amount / rate
}

/// Format currency amount with appropriate symbol and precision
pub fn format_currency_amount(amount: f64, currency_code: &str) -> String {
    let currencies = get_supported_currencies();
    let currency = currencies.iter().find(|c| c.code == currency_code);
    
    let symbol = currency.map_or("$", |c| &c.symbol);
    let precision = match currency_code {
        "JPY" => 0, // Yen doesn't use decimal places
        _ => 2,
    };
    
    format!("{}{:.precision$}", symbol, amount, precision = precision)
}

/// Get currency symbol for the selected currency
pub fn get_current_currency_symbol() -> String {
    let current_currency = SELECTED_CURRENCY.read();
    let currencies = get_supported_currencies();
    currencies
        .iter()
        .find(|c| c.code == *current_currency)
        .map_or("$".to_string(), |c| c.symbol.clone())
}

/// Initialize currency system - fetch rates and load saved preference
pub async fn initialize_currency_system() {
    // Load saved currency preference
    if let Some(saved_currency) = load_currency_from_storage() {
        *SELECTED_CURRENCY.write() = saved_currency;
    }
    
    // Fetch initial exchange rates
    match fetch_exchange_rates().await {
        Ok(rates) => {
            *EXCHANGE_RATES.write() = rates;
        }
        Err(e) => {
            println!("Failed to fetch initial exchange rates: {}", e);
        }
    }
}

/// Save currency preference to storage
pub fn save_currency_to_storage(currency: &str) {
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        storage.set_item("selected_currency", currency).unwrap();
    }
    
    #[cfg(not(feature = "web"))]
    {
        if let Ok(_) = std::fs::create_dir_all("storage") {
            let currency_file = "storage/currency.txt";
            match std::fs::write(currency_file, currency) {
                Ok(_) => println!("✅ Currency saved to: {}", currency_file),
                Err(e) => println!("❌ Failed to write currency to {}: {}", currency_file, e),
            }
        }
    }
}

/// Load currency preference from storage
pub fn load_currency_from_storage() -> Option<String> {
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        storage.get_item("selected_currency").unwrap()
    }
    
    #[cfg(not(feature = "web"))]
    {
        let currency_file = "storage/currency.txt";
        match std::fs::read_to_string(currency_file) {
            Ok(data) => Some(data.trim().to_string()),
            Err(_) => None,
        }
    }
}

/// Update exchange rates periodically
pub async fn update_exchange_rates_loop() {
    loop {
        // Wait 10 minutes between updates
        tokio::time::sleep(std::time::Duration::from_secs(600)).await;
        
        match fetch_exchange_rates().await {
            Ok(rates) => {
                *EXCHANGE_RATES.write() = rates;
                println!("Exchange rates updated successfully");
            }
            Err(e) => {
                println!("Failed to update exchange rates: {}", e);
            }
        }
    }
}