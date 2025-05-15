use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Embed the tokens.json file at compile time
const TOKENS_JSON: &str = include_str!("../../assets/tokens.json");

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct JupiterToken {
    pub address: String,
    pub name: String,
    pub symbol: String,
    #[serde(rename = "logoURI")]
    pub logo_uri: String,
    pub tags: Vec<String>,
}

pub fn get_verified_tokens() -> HashMap<String, JupiterToken> {
    // Parse the JSON at runtime
    let tokens: Vec<JupiterToken> = serde_json::from_str(TOKENS_JSON)
        .unwrap_or_else(|e| {
            println!("Failed to parse tokens.json: {}", e);
            Vec::new()
        });

    tokens
        .into_iter()
        .map(|token| (token.address.clone(), token))
        .collect()
}