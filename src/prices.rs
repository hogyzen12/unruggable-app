use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use chrono::{DateTime, Utc, Datelike, TimeZone};
use std::sync::Mutex;
use std::time::{Duration, Instant};

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
    use std::sync::OnceLock;
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

async fn get_previous_day_price(symbol: &str) -> Result<Option<f64>, Box<dyn Error>> {
    println!("get_previous_day_price called for symbol: {}", symbol);
    
    // Check cache first for historical data
    #[cfg(not(target_os = "android"))]
    let cache_ref = &*HISTORICAL_CACHE;
    #[cfg(target_os = "android")]
    let cache_ref = get_historical_cache();
    
    {
        let cache = cache_ref.lock().unwrap();
        if let Some((price, timestamp)) = cache.get(symbol) {
            if timestamp.elapsed() < Duration::from_secs(HISTORICAL_CACHE_TIMEOUT) {
                println!("Using cached historical price for {}: {}, age: {:?}", 
                         symbol, price, timestamp.elapsed());
                return Ok(Some(*price));
            }
        }
    }
    
    println!("Cache miss or expired for {}, fetching historical price", symbol);
    let client = Client::new();
    
    // Calculate timestamps - just need enough for previous day's closing price
    let end_time = Utc::now();
    let start_time = end_time - chrono::Duration::days(2);
    let from_timestamp = start_time.timestamp();
    let to_timestamp = end_time.timestamp();
    
    println!("Fetching historical data for {}: from {} to {}", 
             symbol, start_time.format("%Y-%m-%d"), end_time.format("%Y-%m-%d"));
    
    // Build query
    let params = [
        ("symbol", format!("Crypto:{}/USD", symbol)),
        ("resolution", "1D".to_string()),
        ("from", from_timestamp.to_string()),
        ("to", to_timestamp.to_string()),
    ];
    
    println!("API Request params: {:#?}", params);
    
    // Fetch from API
    let response = client
        .get(PYTH_HISTORY_URL)
        .query(&params)
        .header("accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch historical prices: {}", e))?;
    
    if !response.status().is_success() {
        println!("Historical API error: {}", response.status());
        return Err(format!("Historical API error: {}", response.status()).into());
    }
    
    // Parse response
    let hist_data: HistoricalPriceResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse historical data: {}", e))?;
    
    println!("Historical response status: {}", hist_data.s);
    
    if hist_data.s != "ok" || hist_data.t.is_none() || hist_data.c.is_none() {
        println!("No valid historical data returned for {}", symbol);
        return Ok(None);
    }
    
    let timestamps = hist_data.t.unwrap();
    let prices = hist_data.c.unwrap();
    
    println!("Received timestamps: {:?}", timestamps);
    println!("Received prices: {:?}", prices);
    
    if timestamps.is_empty() || prices.is_empty() {
        println!("Empty historical data for {}", symbol);
        return Ok(None);
    }
    
    // Find price at midnight UTC today
    let now = Utc::now();
    let today_midnight = Utc
        .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
        .single()
        .unwrap_or_else(|| now);
    let midnight_ts = today_midnight.timestamp();
    
    println!("Looking for price data before {}", today_midnight.format("%Y-%m-%d %H:%M:%S"));
    
    // Find the last price before midnight
    for i in (0..timestamps.len()).rev() {
        if timestamps[i] < midnight_ts {
            let price = prices[i];
            
            println!("Found price data for {}: timestamp={}, price={}",
                     symbol, 
                     chrono::NaiveDateTime::from_timestamp_opt(timestamps[i], 0)
                        .map_or("invalid".to_string(), |dt| dt.format("%Y-%m-%d %H:%M:%S").to_string()),
                     price);
            
            // Update cache for historical data only
            {
                let mut cache = cache_ref.lock().unwrap();
                cache.insert(symbol.to_string(), (price, Instant::now()));
                println!("Updated cache for {}", symbol);
            }
            
            return Ok(Some(price));
        }
    }
    
    println!("No price data found for {} before {}", 
             symbol, today_midnight.format("%Y-%m-%d %H:%M:%S"));
    Ok(None)
}

pub async fn get_historical_changes(
    current_prices: &HashMap<String, f64>
) -> Result<HashMap<String, (Option<f64>, Option<f64>)>, Box<dyn Error>> {
    println!("get_historical_changes called with current prices: {:#?}", current_prices);
    
    // Check if current_prices is empty - if so, return early
    if current_prices.is_empty() {
        println!("Current prices is empty! Cannot fetch historical changes without current prices.");
        return Err("Current prices is empty. Cannot fetch historical changes.".into());
    }
    
    let mut changes = HashMap::new();
    
    // Only process tokens we care about to minimize API calls
    for token in &["SOL", "JUP", "JTO", "BONK", "JLP"] {
        println!("Processing historical data for token: {}", token);
        
        // Look for the token in current_prices with case-insensitive matching
        let token_key = current_prices.keys()
            .find(|k| k.to_uppercase() == token.to_string().to_uppercase())
            .cloned();
            
        if let Some(key) = token_key {
            let current_price = current_prices[&key];
            println!("Current price for {}: {}", token, current_price);
            
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
                    
                    // Add entries for multiple case variations to ensure matches
                    changes.insert(token.to_string(), (Some(change_amount), Some(change_percentage)));
                    changes.insert(token.to_string().to_uppercase(), (Some(change_amount), Some(change_percentage)));
                    changes.insert(token.to_string().to_lowercase(), (Some(change_amount), Some(change_percentage)));
                },
                Ok(None) => {
                    println!("No previous day price found for {}", token);
                    
                    // Add entries for multiple case variations
                    changes.insert(token.to_string(), (None, None));
                    changes.insert(token.to_string().to_uppercase(), (None, None));
                    changes.insert(token.to_string().to_lowercase(), (None, None));
                },
                Err(e) => {
                    println!("Error getting previous day price for {}: {}", token, e);
                    
                    // Add entries for multiple case variations
                    changes.insert(token.to_string(), (None, None));
                    changes.insert(token.to_string().to_uppercase(), (None, None));
                    changes.insert(token.to_string().to_lowercase(), (None, None));
                }
            }
        } else {
            println!("No current price for token: {}", token);
        }
    }
    
    // Add zero changes for stablecoins
    let stablecoin_entry = (Some(0.0), Some(0.0));
    
    // Add entries for multiple case variations of stablecoins
    changes.insert("USDC".to_string(), stablecoin_entry);
    changes.insert("usdc".to_string(), stablecoin_entry);
    changes.insert("Usdc".to_string(), stablecoin_entry);
    
    changes.insert("USDT".to_string(), stablecoin_entry);
    changes.insert("usdt".to_string(), stablecoin_entry);
    changes.insert("Usdt".to_string(), stablecoin_entry);
    
    println!("Final historical changes: {:#?}", changes);
    Ok(changes)
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