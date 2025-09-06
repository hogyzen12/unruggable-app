// src/token_utils.rs
use crate::components::common::{Token, TokenDisplayData, TokenCategory, SortCriteria, TokenSortConfig, TokenFilter};
use std::collections::HashMap;

/// Default icon for tokens without specific icons
const ICON_32: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/solana-mobile@main/assets/icons/32x32.png";

/// Enhance token with display metadata
pub fn enhance_token_data(token: Token, token_prices: &HashMap<String, f64>) -> TokenDisplayData {
    let has_price_data = token_prices.contains_key(&token.symbol) && token.price > 0.0;
    let has_icon = !token.icon_type.is_empty() && token.icon_type != ICON_32;
    
    let token_category = categorize_token(&token.symbol);
    let sort_priority = get_sort_priority(&token.symbol, &token_category);
    
    TokenDisplayData {
        token,
        has_price_data,
        has_icon,
        token_category,
        sort_priority,
    }
}

/// Categorize tokens by type
fn categorize_token(symbol: &str) -> TokenCategory {
    match symbol {
        "SOL" => TokenCategory::Native,
        "USDC" | "USDT" => TokenCategory::Stablecoin,
        "JUP" | "JTO" | "JLP" => TokenCategory::DeFi,
        "BONK" => TokenCategory::Meme,
        _ => TokenCategory::Unknown,
    }
}

/// Get sort priority for initial ordering (lower = higher priority)
fn get_sort_priority(symbol: &str, category: &TokenCategory) -> u32 {
    match symbol {
        "SOL" => 0,      // Always first
        "USDC" => 1,     // Stable coins next
        "USDT" => 2,
        "JUP" => 10,     // DeFi tokens
        "JTO" => 11,
        "JLP" => 12,
        "BONK" => 20,    // Meme tokens
        _ => 100,        // Unknown tokens last
    }
}

/// Sort tokens based on configuration
pub fn sort_tokens(tokens: &mut Vec<TokenDisplayData>, config: &TokenSortConfig) {
    tokens.sort_by(|a, b| {
        // Always put SOL first regardless of sort criteria
        if a.token.symbol == "SOL" && b.token.symbol != "SOL" {
            return std::cmp::Ordering::Less;
        }
        if b.token.symbol == "SOL" && a.token.symbol != "SOL" {
            return std::cmp::Ordering::Greater;
        }
        
        // Then prioritize tokens with price data
        match (a.has_price_data, b.has_price_data) {
            (true, false) => return std::cmp::Ordering::Less,
            (false, true) => return std::cmp::Ordering::Greater,
            _ => {}
        }
        
        // Apply main sorting criteria
        let cmp = match config.primary {
            SortCriteria::ValueUsd => {
                b.token.value_usd.partial_cmp(&a.token.value_usd).unwrap_or(std::cmp::Ordering::Equal)
            }
            SortCriteria::Balance => {
                b.token.balance.partial_cmp(&a.token.balance).unwrap_or(std::cmp::Ordering::Equal)
            }
            SortCriteria::PriceChange24h => {
                b.token.price_change_1d.partial_cmp(&a.token.price_change_1d).unwrap_or(std::cmp::Ordering::Equal)
            }
            SortCriteria::Alphabetical => {
                a.token.symbol.cmp(&b.token.symbol)
            }
            SortCriteria::HasPrice => {
                // Already handled above
                std::cmp::Ordering::Equal
            }
        };
        
        if config.ascending { cmp.reverse() } else { cmp }
    });
}

/// Filter tokens based on criteria
pub fn filter_tokens(tokens: &[TokenDisplayData], filter: &TokenFilter) -> Vec<TokenDisplayData> {
    tokens.iter()
        .filter(|token_data| {
            // Price filter
            if !filter.show_without_price && !token_data.has_price_data {
                return false;
            }
            
            // Minimum value filter
            if let Some(min_value) = filter.min_value_usd {
                if token_data.token.value_usd < min_value {
                    return false;
                }
            }
            
            // Search query filter
            if let Some(query) = &filter.search_query {
                if !query.is_empty() {
                    let query_lower = query.to_lowercase();
                    let symbol_matches = token_data.token.symbol.to_lowercase().contains(&query_lower);
                    let name_matches = token_data.token.name.to_lowercase().contains(&query_lower);
                    
                    if !symbol_matches && !name_matches {
                        return false;
                    }
                }
            }
            
            true
        })
        .cloned()
        .collect()
}

/// Process tokens for display (main function)
pub fn process_tokens_for_display(
    tokens: Vec<Token>,
    token_prices: &HashMap<String, f64>,
    sort_config: &TokenSortConfig,
    filter: &TokenFilter,
) -> Vec<TokenDisplayData> {
    // 1. Enhance tokens with metadata
    let mut enhanced_tokens: Vec<TokenDisplayData> = tokens
        .into_iter()
        .map(|token| enhance_token_data(token, token_prices))
        .collect();
    
    // 2. Apply sorting
    sort_tokens(&mut enhanced_tokens, sort_config);
    
    // 3. Apply filtering
    filter_tokens(&enhanced_tokens, filter)
}