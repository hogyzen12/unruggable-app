use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use chrono::Utc;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use std::sync::OnceLock;

// API Constants
const PYTH_HISTORY_URL: &str = "https://benchmarks.pyth.network/v1/shims/tradingview/history";
const JUPITER_PRICE_API_URL: &str = "https://lite-api.jup.ag/price/v3";
const PRICE_CACHE_TIMEOUT: u64 = 120; // 2 minutes

// Token mint addresses for Jupiter API
pub const TOKEN_MINTS: &[(&str, &str)] = &[
    ("SOL", "So11111111111111111111111111111111111111112"),
    ("USDC", "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"),
    ("USDT", "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB"),
    ("JUP", "JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN"),
    ("JTO", "jtojtomepa8beP8AuQc6eXt5FriJwfFMwQx2v2f9mCL"),
    ("JLP", "27G8MtK7VtTcCHkpASjSDdkWWYfoqT6ggEuKidVJidD4"),
    ("BONK", "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263"),
];

// Multi-timeframe price data structure
#[derive(Debug, Clone)]
pub struct MultiTimeframePriceData {
    pub current_price: f64,
    pub change_1d_amount: Option<f64>,
    pub change_1d_percentage: Option<f64>,
    pub change_3d_amount: Option<f64>,
    pub change_3d_percentage: Option<f64>,
    pub change_7d_amount: Option<f64>,
    pub change_7d_percentage: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CandlestickData {
    pub timestamp: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: Option<f64>,
}

// Jupiter API V3 response structure
#[derive(Debug, Deserialize)]
struct JupiterTokenPrice {
    #[serde(rename = "usdPrice")]
    usd_price: f64,
    #[serde(rename = "blockId")]
    block_id: Option<u64>,
    decimals: Option<u8>,
    #[serde(rename = "priceChange24h")]
    price_change_24h: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct TradingViewHistoryResponse {
    s: String, // Status
    t: Option<Vec<i64>>, // Timestamps
    o: Option<Vec<f64>>, // Open prices
    h: Option<Vec<f64>>, // High prices
    l: Option<Vec<f64>>, // Low prices
    c: Option<Vec<f64>>, // Close prices
    v: Option<Vec<f64>>, // Volume (optional)
}

// Cache for price data
static PRICE_CACHE: OnceLock<Mutex<(HashMap<String, f64>, HashMap<String, MultiTimeframePriceData>, Instant)>> = OnceLock::new();

fn get_price_cache() -> &'static Mutex<(HashMap<String, f64>, HashMap<String, MultiTimeframePriceData>, Instant)> {
    PRICE_CACHE.get_or_init(|| Mutex::new((HashMap::new(), HashMap::new(), Instant::now())))
}

/// Fetch prices from Jupiter API for specific mint addresses
pub async fn get_jupiter_prices_for_mints(mint_addresses: Vec<String>) -> Result<HashMap<String, f64>, Box<dyn Error>> {
    println!("Fetching prices from Jupiter API for {} mints...", mint_addresses.len());
    
    let client = Client::new();
    
    // Build comma-separated mint addresses
    let ids_param = mint_addresses.join(",");
    
    println!("Jupiter API request: {} with IDs: {}", JUPITER_PRICE_API_URL, ids_param);
    
    let response = client
        .get(JUPITER_PRICE_API_URL)
        .query(&[("ids", &ids_param)])
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("Jupiter API request failed: {}", e))?;

    let status = response.status();
    println!("Jupiter API response status: {}", status);

    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Jupiter API error {}: {}", status, error_text).into());
    }

    let response_text = response.text().await?;
    println!("Jupiter API raw response: {}", response_text);

    let jupiter_response: HashMap<String, JupiterTokenPrice> = serde_json::from_str(&response_text)
        .map_err(|e| format!("Failed to parse Jupiter response: {} - Response: {}", e, response_text))?;

    let mut prices = HashMap::new();
    
    // Map mint addresses to prices
    for (mint_address, token_data) in jupiter_response {
        prices.insert(mint_address.clone(), token_data.usd_price);
        println!("Jupiter: {} = ${:.4}", mint_address, token_data.usd_price);
    }
    
    println!("Jupiter API returned {} prices", prices.len());
    Ok(prices)
}

/// Fetch prices from Jupiter API for all hardcoded tokens
pub async fn get_jupiter_prices() -> Result<HashMap<String, f64>, Box<dyn Error>> {
    println!("Fetching prices from Jupiter API...");
    
    let client = Client::new();
    
    // Build comma-separated mint addresses for all tokens
    let mint_addresses: Vec<&str> = TOKEN_MINTS.iter().map(|(_, mint)| *mint).collect();
    let ids_param = mint_addresses.join(",");
    
    println!("Jupiter API request: {} with IDs: {}", JUPITER_PRICE_API_URL, ids_param);
    
    let response = client
        .get(JUPITER_PRICE_API_URL)
        .query(&[("ids", &ids_param)])
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("Jupiter API request failed: {}", e))?;

    let status = response.status();
    println!("Jupiter API response status: {}", status);

    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Jupiter API error {}: {}", status, error_text).into());
    }

    let response_text = response.text().await?;
    println!("Jupiter API raw response: {}", response_text);

    let jupiter_response: HashMap<String, JupiterTokenPrice> = serde_json::from_str(&response_text)
        .map_err(|e| format!("Failed to parse Jupiter response: {} - Response: {}", e, response_text))?;

    let mut prices = HashMap::new();
    
    // Map mint addresses back to token symbols
    for (token_symbol, mint_address) in TOKEN_MINTS {
        if let Some(token_data) = jupiter_response.get(*mint_address) {
            prices.insert(token_symbol.to_string(), token_data.usd_price);
            println!("Jupiter: {} = ${:.4}", token_symbol, token_data.usd_price);
        } else {
            println!("Warning: No price data for {} ({})", token_symbol, mint_address);
        }
    }
    
    // Ensure stablecoins have prices
    if !prices.contains_key("USDC") {
        prices.insert("USDC".to_string(), 1.0);
        println!("Using fixed price for USDC: $1.00");
    }
    if !prices.contains_key("USDT") {
        prices.insert("USDT".to_string(), 1.0);
        println!("Using fixed price for USDT: $1.00");
    }
    
    println!("Jupiter API returned {} prices", prices.len());
    Ok(prices)
}

/// Fetch prices for discovered tokens (with symbol mapping)
pub async fn get_prices_for_tokens(token_mint_to_symbol: HashMap<String, String>) -> Result<HashMap<String, f64>, Box<dyn Error>> {
    println!("Fetching prices for {} discovered tokens...", token_mint_to_symbol.len());
    
    // Always include SOL
    let mut all_mints = vec!["So11111111111111111111111111111111111111112".to_string()];
    let mut symbol_to_mint = HashMap::new();
    symbol_to_mint.insert("SOL".to_string(), "So11111111111111111111111111111111111111112".to_string());
    
    // Add discovered tokens
    for (mint, symbol) in &token_mint_to_symbol {
        all_mints.push(mint.clone());
        symbol_to_mint.insert(symbol.clone(), mint.clone());
    }
    
    println!("Requesting prices for mints: {:?}", all_mints);
    
    // Fetch prices by mint addresses
    let mint_prices = get_jupiter_prices_for_mints(all_mints).await?;
    
    // Convert from mint->price to symbol->price
    let mut symbol_prices = HashMap::new();
    
    for (symbol, mint) in symbol_to_mint {
        if let Some(price) = mint_prices.get(&mint) {
            symbol_prices.insert(symbol.clone(), *price);
            println!("Mapped: {} ({}) = ${:.4}", symbol, mint, price);
        } else {
            // Fallback for stablecoins
            match symbol.as_str() {
                "USDC" | "USDT" => {
                    symbol_prices.insert(symbol.clone(), 1.0);
                    println!("Using fixed price for {}: $1.00", symbol);
                }
                _ => {
                    println!("No price found for {} ({})", symbol, mint);
                }
            }
        }
    }
    
    println!("Final symbol prices: {} tokens", symbol_prices.len());
    Ok(symbol_prices)
}

fn create_dummy_multi_data(prices: &HashMap<String, f64>) -> HashMap<String, MultiTimeframePriceData> {
    let mut multi_data = HashMap::new();
    
    for (token, price) in prices {
        multi_data.insert(token.clone(), MultiTimeframePriceData {
            current_price: *price,
            change_1d_amount: Some(0.0),
            change_1d_percentage: Some(0.0),
            change_3d_amount: Some(0.0),
            change_3d_percentage: Some(0.0),
            change_7d_amount: Some(0.0),
            change_7d_percentage: Some(0.0),
        });
    }
    
    multi_data
}

/// Main function to get cached prices and changes
pub async fn get_cached_prices_and_changes() -> Result<(HashMap<String, f64>, HashMap<String, MultiTimeframePriceData>), Box<dyn Error>> {
    // Check cache first
    {
        let cache = get_price_cache().lock().unwrap();
        let (current_prices, historical_data, timestamp) = &*cache;
        
        if timestamp.elapsed() < Duration::from_secs(PRICE_CACHE_TIMEOUT) && !current_prices.is_empty() {
            println!("Using cached price data (age: {:?})", timestamp.elapsed());
            return Ok((current_prices.clone(), historical_data.clone()));
        }
    }
    
    println!("Cache expired, fetching fresh data...");
    
    // Fetch fresh data from Jupiter
    let current_prices = get_jupiter_prices().await?;
    let historical_data = create_dummy_multi_data(&current_prices);
    
    // Update cache
    {
        let mut cache = get_price_cache().lock().unwrap();
        *cache = (current_prices.clone(), historical_data.clone(), Instant::now());
    }
    
    println!("Updated price cache with fresh data: {} tokens", current_prices.len());
    Ok((current_prices, historical_data))
}

/// Get candlestick data for charts
pub async fn get_candlestick_data(symbol: &str, days: i64) -> Result<Vec<CandlestickData>, Box<dyn Error>> {
    let client = Client::new();
    let end_time = Utc::now();
    let start_time = end_time - chrono::Duration::days(days);
    
    let params = [
        ("symbol", format!("Crypto.{}/USD", symbol)),
        ("resolution", "1D".to_string()),
        ("from", start_time.timestamp().to_string()),
        ("to", end_time.timestamp().to_string()),
    ];
    
    let response = client
        .get(PYTH_HISTORY_URL)
        .query(&params)
        .header("accept", "application/json")
        .send()
        .await?;
    
    if !response.status().is_success() {
        return Err(format!("API error for {}: {}", symbol, response.status()).into());
    }
    
    let hist_data: TradingViewHistoryResponse = response.json().await?;
    
    if hist_data.s != "ok" {
        return Err(format!("API returned error status: {}", hist_data.s).into());
    }
    
    let timestamps = hist_data.t.ok_or("No timestamp data")?;
    let opens = hist_data.o.ok_or("No open price data")?;
    let highs = hist_data.h.ok_or("No high price data")?;
    let lows = hist_data.l.ok_or("No low price data")?;
    let closes = hist_data.c.ok_or("No close price data")?;
    let volumes = hist_data.v;
    
    let mut candlesticks = Vec::new();
    for i in 0..timestamps.len() {
        candlesticks.push(CandlestickData {
            timestamp: timestamps[i],
            open: opens[i],
            high: highs[i],
            low: lows[i],
            close: closes[i],
            volume: volumes.as_ref().map(|v| v[i]),
        });
    }
    
    Ok(candlesticks)
}

/// Get candlestick data with custom resolution
pub async fn get_candlestick_data_with_resolution(
    symbol: &str, 
    days: i64, 
    resolution: &str
) -> Result<Vec<CandlestickData>, Box<dyn Error>> {
    let client = Client::new();
    let end_time = Utc::now();
    let start_time = end_time - chrono::Duration::days(days);
    
    let params = [
        ("symbol", format!("Crypto.{}/USD", symbol)),
        ("resolution", resolution.to_string()),
        ("from", start_time.timestamp().to_string()),
        ("to", end_time.timestamp().to_string()),
    ];
    
    let response = client
        .get(PYTH_HISTORY_URL)
        .query(&params)
        .header("accept", "application/json")
        .send()
        .await?;
    
    if !response.status().is_success() {
        return Err(format!("API error for {}: {}", symbol, response.status()).into());
    }
    
    let hist_data: TradingViewHistoryResponse = response.json().await?;
    
    if hist_data.s != "ok" {
        return Err(format!("API returned error status: {}", hist_data.s).into());
    }
    
    let timestamps = hist_data.t.ok_or("No timestamp data")?;
    let opens = hist_data.o.ok_or("No open price data")?;
    let highs = hist_data.h.ok_or("No high price data")?;
    let lows = hist_data.l.ok_or("No low price data")?;
    let closes = hist_data.c.ok_or("No close price data")?;
    let volumes = hist_data.v;
    
    let mut candlesticks = Vec::new();
    for i in 0..timestamps.len() {
        candlesticks.push(CandlestickData {
            timestamp: timestamps[i],
            open: opens[i],
            high: highs[i],
            low: lows[i],
            close: closes[i],
            volume: volumes.as_ref().map(|v| v[i]),
        });
    }
    
    Ok(candlesticks)
}

// Legacy compatibility functions
pub async fn get_prices() -> Result<HashMap<String, f64>, Box<dyn Error>> {
    get_jupiter_prices().await
}

pub async fn get_enhanced_cached_prices_and_changes() -> Result<(HashMap<String, f64>, HashMap<String, MultiTimeframePriceData>), Box<dyn Error>> {
    get_cached_prices_and_changes().await
}

// Helper function for backward compatibility
pub fn get_token_price_change_from_multi(
    symbol: &str,
    multi_data: &HashMap<String, MultiTimeframePriceData>
) -> f64 {
    if let Some(data) = multi_data.get(symbol) {
        data.change_1d_percentage.unwrap_or(0.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_jupiter_price_api() {
        match get_jupiter_prices().await {
            Ok(prices) => {
                println!("Jupiter API test successful!");
                println!("Fetched prices: {:?}", prices);
                assert!(prices.contains_key("SOL"), "SOL price should be available");
                if let Some(sol_price) = prices.get("SOL") {
                    assert!(*sol_price > 10.0 && *sol_price < 1000.0, "SOL price should be reasonable");
                }
            }
            Err(e) => {
                println!("Jupiter API test failed: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_cached_prices() {
        match get_cached_prices_and_changes().await {
            Ok((prices, _)) => {
                println!("Cached prices test successful!");
                assert!(prices.len() >= 5, "Should have at least 5 token prices");
                assert!(prices.contains_key("SOL"), "SOL price should be available");
            }
            Err(e) => {
                println!("Cached prices test failed: {}", e);
            }
        }
    }
}