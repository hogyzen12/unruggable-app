use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorInfo {
    pub identity: String,
    pub vote_account: String,
    pub name: String,
    pub description: String,
    pub commission: f64,
    pub active_stake: f64,
    pub skip_rate: f64,
    pub is_default: bool,
}

// Hardcoded high-quality validators including your validator as default
pub fn get_recommended_validators() -> Vec<ValidatorInfo> {
    vec![
        ValidatorInfo {
            identity: "YOUR_VALIDATOR_IDENTITY_HERE".to_string(), // Replace with your actual validator identity
            vote_account: "YOUR_VOTE_ACCOUNT_HERE".to_string(), // Replace with your actual vote account
            name: "Unruggable Validator".to_string(),
            description: "The official Unruggable wallet validator - high performance, low commission".to_string(),
            commission: 5.0,
            active_stake: 0.0, // Will be updated when we fetch live data
            skip_rate: 0.05,
            is_default: true,
        },
        ValidatorInfo {
            identity: "Fd7btgySsrjuo25CJCj7oE7VPMyezDhnx7pZkj2v69Nk".to_string(),
            vote_account: "CcaHc2L43ZWjwCHART3oZoJvHLAe9hzT2DJNUpBzoTN1".to_string(),
            name: "Stakewiz".to_string(),
            description: "Professional validator with excellent performance".to_string(),
            commission: 7.0,
            active_stake: 9353968.275006589,
            skip_rate: 0.04,
            is_default: false,
        },
        ValidatorInfo {
            identity: "DRpbCBMxVnDK7maPM5tGv6MvB3v1sRMC86PZ8okm21hy".to_string(),
            vote_account: "3N7s9zXMZ4QqvHQR15t5GNHyqc89KduzMP7423eWiD5g".to_string(),
            name: "Solana Foundation".to_string(),
            description: "Official Solana Foundation validator".to_string(),
            commission: 2.0,
            active_stake: 13061017.501494104,
            skip_rate: 0.02,
            is_default: false,
        },
        ValidatorInfo {
            identity: "HEL1USMZKAL2odpNBj2oCjffnFGaYwmbGmyewGv1e2TU".to_string(),
            vote_account: "he1iusunGwqrNtafDtLdhsUQDFvo13z9sUa36PauBtk".to_string(),
            name: "Helius".to_string(),
            description: "High-performance RPC provider and validator".to_string(),
            commission: 0.0,
            active_stake: 13453011.453622909,
            skip_rate: 0.08,
            is_default: false,
        },
    ]
}

// Function to fetch live validator data (optional enhancement for later)
pub async fn fetch_live_validators(rpc_url: Option<&str>) -> Result<Vec<ValidatorInfo>, Box<dyn std::error::Error>> {
    // For now, return the hardcoded list
    // Later we can implement actual RPC calls to get live data
    Ok(get_recommended_validators())
}