/// Token structure for wallet holdings
#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub mint: String,      // Added to store the unique mint address
    pub symbol: String,
    pub name: String,
    pub icon_type: String,
    pub balance: f64,
    pub value_usd: f64,
    pub price: f64,
    pub price_change: f64,
    pub price_change_1d: f64,
    pub price_change_3d: f64,
    pub price_change_7d: f64,
    pub decimals: u8,      // Token decimals for proper amount conversion
}

// Add after the existing Token struct

/// Enhanced token data for display and sorting
#[derive(Clone, Debug, PartialEq)]
pub struct TokenDisplayData {
    pub token: Token,
    pub has_price_data: bool,
    pub has_icon: bool,
    pub token_category: TokenCategory,
    pub sort_priority: u32,
}

/// Token categories for organization
#[derive(Clone, Debug, PartialEq)]
pub enum TokenCategory {
    Native,      // SOL
    Stablecoin,  // USDC, USDT
    DeFi,        // JUP, JTO, JLP
    Meme,        // BONK
    Unknown,
}

/// Sorting criteria options
#[derive(Clone, Debug, PartialEq)]
pub enum SortCriteria {
    ValueUsd,           // By USD value (default)
    Balance,            // By token balance
    PriceChange24h,     // By 24h price change
    Alphabetical,       // By symbol
    HasPrice,           // Tokens with price data first
}

/// Configuration for token sorting
#[derive(Clone, Debug)]
pub struct TokenSortConfig {
    pub primary: SortCriteria,
    pub ascending: bool,
}

impl Default for TokenSortConfig {
    fn default() -> Self {
        Self {
            primary: SortCriteria::ValueUsd,
            ascending: false, // Highest value first
        }
    }
}

/// Configuration for token filtering
#[derive(Clone, Debug)]
pub struct TokenFilter {
    pub show_without_price: bool,
    pub min_value_usd: Option<f64>,
    pub search_query: Option<String>,
}

impl Default for TokenFilter {
    fn default() -> Self {
        Self {
            show_without_price: true,
            min_value_usd: Some(0.01), // Hide dust by default
            search_query: None,
        }
    }
}