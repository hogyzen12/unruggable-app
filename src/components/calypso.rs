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
