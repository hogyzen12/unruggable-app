use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;

// Constants for the Pyth Hermes API
const PYTH_HERMES_URL: &str = "https://hermes.pyth.network/v2/updates/price/latest";

// Token IDs for Pyth Network
pub const TOKEN_IDS: &[(&str, &str)] = &[
    ("SOL", "ef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d"),
    ("JUP", "0a0408d619e9380abad35060f9192039ed5042fa6f82301d0e48bb52be830996"),
    ("JTO", "b43660a5f790c69354b0729a5ef9d50d68f1df92107540210b9cccba1f947cc2"),
    ("JLP", "c811abc82b4bad1f9bd711a2773ccaa935b03ecef974236942cec5e0eb845a3a"),
    ("BONK", "72b021217ca3fe68922a19aaf990109cb9d84e9ad004b4d2025ad6f529314419"),
];

// Structs for deserializing the Pyth API response
#[derive(Debug, Deserialize)]
struct PythResponse {
    parsed: Vec<PriceItem>,
}

#[derive(Debug, Deserialize)]
struct PriceItem {
    id: String,
    price: PriceData,
}

#[derive(Debug, Deserialize)]
struct PriceData {
    price: String,
    expo: i32,
}

/// Struct to hold the price information in a user-friendly format
#[derive(Debug, Serialize)]
pub struct TokenPrice {
    pub token: String,
    pub price: f64,
}

/// Fetches the latest prices for the specified tokens from Pyth Network
pub async fn get_prices() -> Result<HashMap<String, f64>, Box<dyn Error>> {
    let client = Client::new();

    // Build query parameters with token IDs
    let mut params = vec![];
    for (_, id) in TOKEN_IDS {
        params.push(("ids[]", id.to_string()));
    }
    params.push(("parsed", "true".to_string()));

    // Send GET request to Pyth Hermes API
    let response = client
        .get(PYTH_HERMES_URL)
        .query(&params)
        .send()
        .await
        .map_err(|e| format!("Failed to send request: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Pyth API error: {}", response.status()).into());
    }

    // Parse the JSON response
    let pyth_response: PythResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    // Process the prices
    let mut prices = HashMap::new();
    for item in pyth_response.parsed {
        // Find the token corresponding to the price ID
        if let Some((token, _)) = TOKEN_IDS.iter().find(|(_, id)| *id == item.id) {
            // Calculate the price: price * 10^expo
            // Fix: Use directly the price string value
            let price_value = item.price.price
                .parse::<f64>()
                .map_err(|e| format!("Failed to parse price for {}: {}", token, e))?;
            let price = price_value * 10f64.powi(item.price.expo);
            prices.insert(token.to_string(), price);
        }
    }

    // Add USDC price (always 1.0 as per the JavaScript code)
    prices.insert("USDC".to_string(), 1.0);
    prices.insert("USDT".to_string(), 1.0);

    Ok(prices)
}