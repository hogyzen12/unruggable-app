use std::collections::HashMap;
use std::sync::LazyLock;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifiedToken {
    #[serde(rename = "id")]
    pub address: String,
    pub name: String,
    pub symbol: String,
    #[serde(rename = "icon")]
    pub logo_uri: String,
    pub tags: Vec<String>,
}

// Embed the local JSON file at compile time (mobile-safe)
static TOKENS_JSON: &str = include_str!("../../assets/tokens.json");

// Parse JSON only once when first accessed - mobile-friendly!
static VERIFIED_TOKENS: LazyLock<HashMap<String, VerifiedToken>> = LazyLock::new(|| {
    parse_tokens_from_json(TOKENS_JSON)
});

/// Parse tokens from JSON string (used by both local and remote loading)
fn parse_tokens_from_json(json_str: &str) -> HashMap<String, VerifiedToken> {
    match serde_json::from_str::<Vec<VerifiedToken>>(json_str) {
        Ok(tokens) => {
            let mut map = HashMap::with_capacity(tokens.len());
            for token in tokens {
                // Use the address (id) as the key
                map.insert(token.address.clone(), token);
            }
            println!("Successfully loaded {} verified tokens from JSON", map.len());
            map
        }
        Err(e) => {
            eprintln!("Failed to parse tokens JSON: {}", e);
            
            // Return minimal fallback tokens for critical functionality
            let mut fallback_map = HashMap::new();
            
            // SOL - most critical
            fallback_map.insert(
                "So11111111111111111111111111111111111111112".to_string(),
                VerifiedToken {
                    address: "So11111111111111111111111111111111111111112".to_string(),
                    name: "Wrapped SOL".to_string(),
                    symbol: "SOL".to_string(),
                    logo_uri: "https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/So11111111111111111111111111111111111111112/logo.png".to_string(),
                    tags: vec!["verified".to_string(), "fallback".to_string()],
                },
            );
            
            // USDC - second most critical
            fallback_map.insert(
                "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
                VerifiedToken {
                    address: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
                    name: "USD Coin".to_string(),
                    symbol: "USDC".to_string(),
                    logo_uri: "https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v/logo.png".to_string(),
                    tags: vec!["verified".to_string(), "fallback".to_string()],
                },
            );
            
            println!("Using fallback tokens due to JSON parse error");
            fallback_map
        }
    }
}

/// Get reference to the verified tokens HashMap (mobile-safe)
pub fn get_verified_tokens() -> &'static HashMap<String, VerifiedToken> {
    &VERIFIED_TOKENS
}

/// Get a cloned copy of the verified tokens HashMap
pub fn get_verified_tokens_cloned() -> HashMap<String, VerifiedToken> {
    VERIFIED_TOKENS.clone()
}

// ============================================================================
// ONLINE URL FETCHING (for flexibility) - commented out for mobile safety
// ============================================================================

/*
// Uncomment this section to enable fetching tokens from a remote URL
// Note: This requires async runtime and network permissions

use std::sync::Arc;
use tokio::sync::RwLock;

// For dynamic loading from URL (optional)
static REMOTE_TOKENS: LazyLock<Arc<RwLock<Option<HashMap<String, VerifiedToken>>>>> = 
    LazyLock::new(|| Arc::new(RwLock::new(None)));

/// Fetch and update tokens from a remote URL (async)
/// Example usage: fetch_tokens_from_url("https://api.yourservice.com/tokens.json").await
pub async fn fetch_tokens_from_url(url: &str) -> Result<HashMap<String, VerifiedToken>, Box<dyn std::error::Error>> {
    // Use your HTTP client of choice (reqwest, surf, etc.)
    let response = reqwest::get(url).await?;
    let json_text = response.text().await?;
    
    let tokens_map = parse_tokens_from_json(&json_text);
    
    // Update the global cache
    {
        let mut remote_tokens = REMOTE_TOKENS.write().await;
        *remote_tokens = Some(tokens_map.clone());
    }
    
    println!("Successfully fetched {} tokens from URL: {}", tokens_map.len(), url);
    Ok(tokens_map)
}

/// Get tokens from remote cache if available, otherwise use local
pub async fn get_tokens_with_remote_fallback() -> HashMap<String, VerifiedToken> {
    let remote_tokens = REMOTE_TOKENS.read().await;
    
    if let Some(ref remote_map) = *remote_tokens {
        println!("Using remote tokens ({} tokens)", remote_map.len());
        remote_map.clone()
    } else {
        println!("Using local tokens ({} tokens)", VERIFIED_TOKENS.len());
        VERIFIED_TOKENS.clone()
    }
}

/// Update tokens from URL in the background (fire-and-forget)
pub fn update_tokens_from_url_background(url: String) {
    tokio::spawn(async move {
        match fetch_tokens_from_url(&url).await {
            Ok(tokens) => println!("Background token update successful: {} tokens", tokens.len()),
            Err(e) => eprintln!("Background token update failed: {}", e),
        }
    });
}

// Example usage in your app:
// 
// // On app startup (optional background refresh):
// update_tokens_from_url_background("https://api.yourservice.com/tokens.json".to_string());
// 
// // In your component:
// let tokens = get_tokens_with_remote_fallback().await;
// 
// // Or force refresh:
// let fresh_tokens = fetch_tokens_from_url("https://api.yourservice.com/tokens.json").await?;
*/