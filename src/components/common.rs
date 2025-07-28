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
}