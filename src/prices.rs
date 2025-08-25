use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use chrono::{DateTime, Utc, Datelike, TimeZone};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use std::sync::OnceLock;

// Constants for the APIs
const PYTH_HERMES_URL: &str = "https://hermes.pyth.network/v2/updates/price/latest";
const PYTH_HISTORY_URL: &str = "https://benchmarks.pyth.network/v1/shims/tradingview/history";
const HISTORICAL_CACHE_TIMEOUT: u64 = 3600; // 1 hour for historical data

// Token IDs for Pyth Network
pub const TOKEN_IDS: &[(&str, &str)] = &[
    ("SOL", "ef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d"),
    ("JUP", "0a0408d619e9380abad35060f9192039ed5042fa6f82301d0e48bb52be830996"),
    ("JTO", "b43660a5f790c69354b0729a5ef9d50d68f1df92107540210b9cccba1f947cc2"),
    ("JLP", "c811abc82b4bad1f9bd711a2773ccaa935b03ecef974236942cec5e0eb845a3a"),
    ("BONK", "72b021217ca3fe68922a19aaf990109cb9d84e9ad004b4d2025ad6f529314419"),
];

// Only use lazy_static on non-Android platforms
#[cfg(not(target_os = "android"))]
lazy_static::lazy_static! {
    static ref HISTORICAL_CACHE: Mutex<HashMap<String, (f64, Instant)>> = 
        Mutex::new(HashMap::new());
}

// For Android, use a simple function that returns a new HashMap each time
#[cfg(target_os = "android")]
fn get_historical_cache() -> &'static Mutex<HashMap<String, (f64, Instant)>> {
    static CACHE: OnceLock<Mutex<HashMap<String, (f64, Instant)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

// API response structs
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

#[derive(Debug, Deserialize)]
struct HistoricalPriceResponse {
    s: String, // Status
    t: Option<Vec<i64>>, // Timestamps
    c: Option<Vec<f64>>, // Close prices
}

// Public structs
#[derive(Debug, Clone, Serialize)]
pub struct TokenPriceData {
    pub current_price: f64,
    pub previous_day_price: Option<f64>,
    pub change_amount: Option<f64>,
    pub change_percentage: Option<f64>,
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

/// Gets current prices - UNCHANGED from original implementation
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

pub async fn get_candlestick_data(symbol: &str, days: i64) -> Result<Vec<CandlestickData>, Box<dyn Error>> {
    println!("Fetching {}-day candlestick data for {}", days, symbol);
    
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
    let volumes = hist_data.v; // Optional
    
    if timestamps.len() != opens.len() || timestamps.len() != highs.len() 
        || timestamps.len() != lows.len() || timestamps.len() != closes.len() {
        return Err("Mismatched data array lengths".into());
    }
    
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
    
    println!("✅ Fetched {} candlesticks for {}", candlesticks.len(), symbol);
    Ok(candlesticks)
}

pub async fn get_candlestick_data_with_resolution(
    symbol: &str, 
    days: i64, 
    resolution: &str
) -> Result<Vec<CandlestickData>, Box<dyn Error>> {
    println!("Fetching {}-{} candlestick data for {}", days, resolution, symbol);
    
    let client = Client::new();
    let end_time = Utc::now();
    let start_time = match resolution {
        "1H" => end_time - chrono::Duration::days(days),
        "1D" => end_time - chrono::Duration::days(days),
        _ => end_time - chrono::Duration::days(days),
    };
    
    let params = [
        ("symbol", format!("Crypto.{}/USD", symbol)),
        ("resolution", resolution.to_string()),
        ("from", start_time.timestamp().to_string()),
        ("to", end_time.timestamp().to_string()),
    ];
    
    // Rest of the function is the same as your existing get_candlestick_data
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
    
    if timestamps.len() != opens.len() || timestamps.len() != highs.len() 
        || timestamps.len() != lows.len() || timestamps.len() != closes.len() {
        return Err("Mismatched data array lengths".into());
    }
    
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
    
    println!("✅ Fetched {} candlesticks for {} ({})", candlesticks.len(), symbol, resolution);
    Ok(candlesticks)
}

// Get previous day's closing price (24h ago) - NO CACHING for debugging
async fn get_previous_day_price(symbol: &str) -> Result<Option<f64>, Box<dyn Error>> {
    println!("Fetching historical price for {}", symbol);
    
    let client = Client::new();
    let end_time = Utc::now();
    let start_time = end_time - chrono::Duration::days(3);
    
    let params = [
        ("symbol", format!("Crypto.{}/USD", symbol)), // Fixed: Use dot instead of colon
        ("resolution", "1D".to_string()),
        ("from", start_time.timestamp().to_string()),
        ("to", end_time.timestamp().to_string()),
    ];
    
    println!("API call: {} with params {:?}", PYTH_HISTORY_URL, params);
    
    let response = client
        .get(PYTH_HISTORY_URL)
        .query(&params)
        .header("accept", "application/json")
        .send()
        .await?;
    
    let status = response.status();
    println!("API response status for {}: {}", symbol, status);
    
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_else(|_| "Could not read error".to_string());
        println!("API error for {}: {}", symbol, error_text);
        return Ok(None);
    }
    
    let hist_data: HistoricalPriceResponse = response.json().await?;
    
    println!("API response for {}: status='{}', has_timestamps={}, has_prices={}", 
             symbol, hist_data.s, hist_data.t.is_some(), hist_data.c.is_some());
    
    if hist_data.s != "ok" || hist_data.t.is_none() || hist_data.c.is_none() {
        return Ok(None);
    }
    
    let timestamps = hist_data.t.unwrap();
    let prices = hist_data.c.unwrap();
    
    println!("Got {} data points for {}", timestamps.len(), symbol);
    
    if timestamps.is_empty() || prices.is_empty() {
        return Ok(None);
    }
    
    // Print ALL data points for debugging
    println!("=== ALL HISTORICAL DATA FOR {} ===", symbol);
    for i in 0..timestamps.len() {
        let dt = chrono::NaiveDateTime::from_timestamp_opt(timestamps[i], 0)
            .map_or("invalid".to_string(), |dt| dt.format("%Y-%m-%d %H:%M:%S").to_string());
        println!("  [{}] {} = ${:.4}", i, dt, prices[i]);
    }
    
    // Find most recent price that's at least 20 hours old
    let cutoff_time = end_time - chrono::Duration::hours(20);
    let cutoff_ts = cutoff_time.timestamp();
    
    println!("Looking for price before: {}", cutoff_time.format("%Y-%m-%d %H:%M:%S"));
    
    for i in (0..timestamps.len()).rev() {
        if timestamps[i] <= cutoff_ts {
            let price = prices[i];
            let dt = chrono::NaiveDateTime::from_timestamp_opt(timestamps[i], 0)
                .map_or("invalid".to_string(), |dt| dt.format("%Y-%m-%d %H:%M:%S").to_string());
            
            println!("âœ“ SELECTED historical price for {}: {} = ${:.4}", symbol, dt, price);
            return Ok(Some(price));
        } else {
            let dt = chrono::NaiveDateTime::from_timestamp_opt(timestamps[i], 0)
                .map_or("invalid".to_string(), |dt| dt.format("%Y-%m-%d %H:%M:%S").to_string());
            println!("  Skipping (too recent): {} = ${:.4}", dt, prices[i]);
        }
    }
    
    println!("âœ— No price older than 20h found for {}", symbol);
    Ok(None)
}

// Get week-ago price (7 days ago)
async fn get_week_ago_price(symbol: &str) -> Result<Option<f64>, Box<dyn Error>> {
    println!("Fetching week-ago price for {}", symbol);
    
    let client = Client::new();
    let end_time = Utc::now();
    let start_time = end_time - chrono::Duration::days(10); // 10 days to ensure coverage
    
    let params = [
        ("symbol", format!("Crypto.{}/USD", symbol)), // Fixed: Use dot instead of colon
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
        println!("Week-ago API error for {}: {}", symbol, response.status());
        return Ok(None);
    }
    
    let hist_data: HistoricalPriceResponse = response.json().await?;
    
    if hist_data.s != "ok" || hist_data.t.is_none() || hist_data.c.is_none() {
        return Ok(None);
    }
    
    let timestamps = hist_data.t.unwrap();
    let prices = hist_data.c.unwrap();
    
    // Target: 7 days ago
    let target_time = end_time - chrono::Duration::days(7);
    let target_ts = target_time.timestamp();
    
    println!("Looking for price around 7 days ago: {}", target_time.format("%Y-%m-%d %H:%M:%S"));
    
    // Find closest price to 7 days ago
    let mut best_price = None;
    let mut best_diff = i64::MAX;
    
    for (timestamp, price) in timestamps.iter().zip(prices.iter()) {
        let diff = (timestamp - target_ts).abs();
        if diff < best_diff {
            best_diff = diff;
            best_price = Some(*price);
            
            let dt = chrono::NaiveDateTime::from_timestamp_opt(*timestamp, 0)
                .map_or("invalid".to_string(), |dt| dt.format("%Y-%m-%d %H:%M:%S").to_string());
            println!("Better week-ago candidate for {}: {} = ${:.4} (diff: {}h)", 
                     symbol, dt, price, diff / 3600);
        }
    }
    
    if let Some(price) = best_price {
        println!("âœ“ SELECTED week-ago price for {}: ${:.4}", symbol, price);
    } else {
        println!("âœ— No week-ago price found for {}", symbol);
    }
    
    Ok(best_price)
}

// Replace your get_previous_day_price function with this multi-timeframe version
async fn get_historical_price_at_offset(symbol: &str, days_ago: i64) -> Result<Option<f64>, Box<dyn Error>> {
    println!("Fetching {}-day ago price for {}", days_ago, symbol);
    
    let client = Client::new();
    let end_time = Utc::now();
    let start_time = end_time - chrono::Duration::days(days_ago + 2); // Get extra days for safety
    
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
        println!("API error for {} ({}d): {}", symbol, days_ago, response.status());
        return Ok(None);
    }
    
    let hist_data: HistoricalPriceResponse = response.json().await?;
    
    if hist_data.s != "ok" || hist_data.t.is_none() || hist_data.c.is_none() {
        return Ok(None);
    }
    
    let timestamps = hist_data.t.unwrap();
    let prices = hist_data.c.unwrap();
    
    if timestamps.is_empty() || prices.is_empty() {
        return Ok(None);
    }
    
    // Target time: X days ago
    let target_time = end_time - chrono::Duration::days(days_ago);
    let target_ts = target_time.timestamp();
    
    println!("Looking for {}-day ago price around: {}", days_ago, target_time.format("%Y-%m-%d %H:%M:%S"));
    
    // Find closest price to target time (but prefer slightly older)
    let mut best_price = None;
    let mut best_diff = i64::MAX;
    
    for (timestamp, price) in timestamps.iter().zip(prices.iter()) {
        // Prefer prices that are older than target time
        let diff = if *timestamp <= target_ts {
            target_ts - timestamp  // Positive value for older prices
        } else {
            (*timestamp - target_ts) * 2  // Penalty for newer prices
        };
        
        if diff < best_diff {
            best_diff = diff;
            best_price = Some(*price);
            
            let dt = chrono::NaiveDateTime::from_timestamp_opt(*timestamp, 0)
                .map_or("invalid".to_string(), |dt| dt.format("%Y-%m-%d %H:%M:%S").to_string());
            println!("Better {}-day candidate for {}: {} = ${:.4} (diff: {}h)", 
                     days_ago, symbol, dt, price, diff / 3600);
        }
    }
    
    if let Some(price) = best_price {
        println!("âœ“ SELECTED {}-day ago price for {}: ${:.4}", days_ago, symbol, price);
    } else {
        println!("âœ— No {}-day ago price found for {}", days_ago, symbol);
    }
    
    Ok(best_price)
}

// New structure for multiple timeframe data
#[derive(Debug, Clone)]
pub struct MultiTimeframePriceData {
    pub current_price: f64,
    pub day_1_price: Option<f64>,
    pub day_3_price: Option<f64>, 
    pub day_7_price: Option<f64>,
    pub change_1d_amount: Option<f64>,
    pub change_1d_percentage: Option<f64>,
    pub change_3d_amount: Option<f64>,
    pub change_3d_percentage: Option<f64>,
    pub change_7d_amount: Option<f64>,
    pub change_7d_percentage: Option<f64>,
}

// Updated function to get multi-timeframe data
pub async fn get_multi_timeframe_changes(
    current_prices: &HashMap<String, f64>
) -> Result<HashMap<String, MultiTimeframePriceData>, Box<dyn Error>> {
    println!("=== FETCHING MULTI-TIMEFRAME PRICE CHANGES ===");
    
    if current_prices.is_empty() {
        return Err("Current prices is empty".into());
    }
    
    let mut results = HashMap::new();
    
    // Process supported tokens
    for token in &["SOL", "JUP", "JTO", "BONK", "JLP"] {
        if let Some(current_price) = current_prices.get(*token) {
            println!("\n--- Processing {}: current_price = ${:.4} ---", token, current_price);
            
            // Fetch all timeframes concurrently for speed
            let (day_1_result, day_3_result, day_7_result) = tokio::join!(
                get_historical_price_at_offset(token, 1),
                get_historical_price_at_offset(token, 3), 
                get_historical_price_at_offset(token, 7)
            );
            
            let day_1_price = day_1_result.ok().flatten();
            let day_3_price = day_3_result.ok().flatten();
            let day_7_price = day_7_result.ok().flatten();
            
            // Calculate changes for each timeframe
            let (change_1d_amount, change_1d_percentage) = if let Some(prev_price) = day_1_price {
                let amount = current_price - prev_price;
                let percentage = if prev_price > 0.0 { (amount / prev_price) * 100.0 } else { 0.0 };
                println!("ðŸ“Š {} 1D: NOW=${:.4}, 1D_AGO=${:.4}, CHANGE={:+.2}%", token, current_price, prev_price, percentage);
                (Some(amount), Some(percentage))
            } else {
                println!("âŒ {} 1D: No data", token);
                (None, None)
            };
            
            let (change_3d_amount, change_3d_percentage) = if let Some(prev_price) = day_3_price {
                let amount = current_price - prev_price;
                let percentage = if prev_price > 0.0 { (amount / prev_price) * 100.0 } else { 0.0 };
                println!("ðŸ“Š {} 3D: NOW=${:.4}, 3D_AGO=${:.4}, CHANGE={:+.2}%", token, current_price, prev_price, percentage);
                (Some(amount), Some(percentage))
            } else {
                println!("âŒ {} 3D: No data", token);
                (None, None)
            };
            
            let (change_7d_amount, change_7d_percentage) = if let Some(prev_price) = day_7_price {
                let amount = current_price - prev_price;
                let percentage = if prev_price > 0.0 { (amount / prev_price) * 100.0 } else { 0.0 };
                println!("ðŸ“Š {} 7D: NOW=${:.4}, 7D_AGO=${:.4}, CHANGE={:+.2}%", token, current_price, prev_price, percentage);
                (Some(amount), Some(percentage))
            } else {
                println!("âŒ {} 7D: No data", token);
                (None, None)
            };
            
            results.insert(token.to_string(), MultiTimeframePriceData {
                current_price: *current_price,
                day_1_price,
                day_3_price,
                day_7_price,
                change_1d_amount,
                change_1d_percentage,
                change_3d_amount,
                change_3d_percentage,
                change_7d_amount,
                change_7d_percentage,
            });
        }
    }
    
    // Add stablecoins with zero changes
    for stablecoin in &["USDC", "USDT"] {
        if let Some(current_price) = current_prices.get(*stablecoin) {
            results.insert(stablecoin.to_string(), MultiTimeframePriceData {
                current_price: *current_price,
                day_1_price: Some(*current_price),
                day_3_price: Some(*current_price),
                day_7_price: Some(*current_price),
                change_1d_amount: Some(0.0),
                change_1d_percentage: Some(0.0),
                change_3d_amount: Some(0.0),
                change_3d_percentage: Some(0.0),
                change_7d_amount: Some(0.0),
                change_7d_percentage: Some(0.0),
            });
            println!("ðŸ’° {}: Stablecoin - all changes 0%", stablecoin);
        }
    }
    
    println!("\n=== SUMMARY ===");
    for (token, data) in &results {
        println!("{}: 1D={:+.1}%, 3D={:+.1}%, 7D={:+.1}%", 
                 token,
                 data.change_1d_percentage.unwrap_or(0.0),
                 data.change_3d_percentage.unwrap_or(0.0), 
                 data.change_7d_percentage.unwrap_or(0.0));
    }
    
    Ok(results)
}

// Helper function to extract 1D percentage for backward compatibility
pub fn get_token_price_change_from_multi(
    symbol: &str,
    multi_data: &HashMap<String, MultiTimeframePriceData>
) -> f64 {
    if let Some(data) = multi_data.get(symbol) {
        data.change_1d_percentage.unwrap_or(0.0)
    } else {
        println!("âŒ {} not found in multi-timeframe data", symbol);
        0.0
    }
}

/// Gets both current and historical price data
pub async fn get_price_data() -> Result<HashMap<String, TokenPriceData>, Box<dyn Error>> {
    // Get current prices (fast)
    let current_prices = get_prices().await?;
    
    // Get historical changes (separate and cached)
    let historical_changes = get_historical_changes(&current_prices).await?;
    
    // Combine the data
    let mut price_data = HashMap::new();
    
    for (token, current_price) in current_prices.iter() {
        let (change_amount, change_percentage) = historical_changes
            .get(token)
            .cloned()
            .unwrap_or((None, None));
        
        let previous_day_price = if let Some(change) = change_amount {
            Some(current_price - change)
        } else {
            None
        };
        
        price_data.insert(token.clone(), TokenPriceData {
            current_price: *current_price,
            previous_day_price,
            change_amount,
            change_percentage,
        });
    }
    
    Ok(price_data)
}

// Cache for both current prices and historical data
static PRICE_CACHE: OnceLock<Mutex<(HashMap<String, f64>, HashMap<String, MultiTimeframePriceData>, Instant)>> = OnceLock::new();
const PRICE_CACHE_TIMEOUT: u64 = 120; // 2 minutes

fn get_price_cache() -> &'static Mutex<(HashMap<String, f64>, HashMap<String, MultiTimeframePriceData>, Instant)> {
    PRICE_CACHE.get_or_init(|| Mutex::new((HashMap::new(), HashMap::new(), Instant::now())))
}

// Modified function that checks cache first
pub async fn get_cached_prices_and_changes() -> Result<(HashMap<String, f64>, HashMap<String, MultiTimeframePriceData>), Box<dyn Error>> {
    // Check cache first
    {
        let cache = get_price_cache().lock().unwrap();
        let (current_prices, historical_data, timestamp) = &*cache;
        
        if timestamp.elapsed() < Duration::from_secs(PRICE_CACHE_TIMEOUT) && !current_prices.is_empty() {
            println!("âœ… Using cached price data (age: {:?})", timestamp.elapsed());
            return Ok((current_prices.clone(), historical_data.clone()));
        }
    }
    
    println!("ðŸ”„ Cache miss or expired, fetching fresh price data...");
    
    // Fetch fresh data
    let current_prices = get_prices().await?;
    let historical_data = get_multi_timeframe_changes(&current_prices).await?;
    
    // Update cache
    {
        let mut cache = get_price_cache().lock().unwrap();
        *cache = (current_prices.clone(), historical_data.clone(), Instant::now());
    }
    
    println!("âœ… Updated price cache with fresh data");
    Ok((current_prices, historical_data))
}

// For backward compatibility with existing code
pub async fn get_historical_changes_cached(
    current_prices: &HashMap<String, f64>
) -> Result<HashMap<String, (Option<f64>, Option<f64>)>, Box<dyn Error>> {
    let (_, multi_data) = get_cached_prices_and_changes().await?;
    
    // Convert multi-timeframe data back to old format (1D only)
    let mut old_format = HashMap::new();
    
    for (token, data) in multi_data {
        old_format.insert(token, (data.change_1d_amount, data.change_1d_percentage));
    }
    
    Ok(old_format)
}

// Get historical changes for all supported tokens
pub async fn get_historical_changes(
    current_prices: &HashMap<String, f64>
) -> Result<HashMap<String, (Option<f64>, Option<f64>)>, Box<dyn Error>> {
    println!("=== FETCHING HISTORICAL CHANGES ===");
    println!("get_historical_changes called with {} tokens", current_prices.len());
    
    if current_prices.is_empty() {
        return Err("Current prices is empty".into());
    }
    
    let mut changes = HashMap::new();
    
    // Process supported tokens
    for token in &["SOL", "JUP", "JTO", "BONK", "JLP"] {
        if let Some(current_price) = current_prices.get(*token) {
            println!("\n--- Processing {}: current_price = ${:.4} ---", token, current_price);
            
            match get_previous_day_price(token).await {
                Ok(Some(prev_price)) => {
                    println!("Found previous day price for {}: {}", token, prev_price);
                    let change_amount = current_price - prev_price;
                    let change_percentage = if prev_price > 0.0 {
                        (change_amount / prev_price) * 100.0
                    } else {
                        0.0
                    };
                    println!("Calculated changes for {}: amount={}, percentage={}%", 
                             token, change_amount, change_percentage);
                    
                    changes.insert(token.to_string(), (Some(change_amount), Some(change_percentage)));
                },
                Ok(None) => {
                    println!("No previous day price found for {}", token);
                    changes.insert(token.to_string(), (None, None));
                },
                Err(e) => {
                    println!("Error getting previous day price for {}: {}", token, e);
                    changes.insert(token.to_string(), (None, None));
                }
            }
        } else {
            println!("No current price for token: {}", token);
        }
    }
    
    // Add zero changes for stablecoins
    let stablecoin_entry = (Some(0.0), Some(0.0));
    changes.insert("USDC".to_string(), stablecoin_entry);
    changes.insert("USDT".to_string(), stablecoin_entry);
    
    println!("Final changes map has {} entries", changes.len());
    Ok(changes)
}