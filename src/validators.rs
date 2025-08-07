use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use reqwest::Client;

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

// RPC response structures for getVoteAccounts
#[derive(Debug, Deserialize)]
struct VoteAccountsResponse {
    current: Vec<VoteAccountInfo>,
    delinquent: Vec<VoteAccountInfo>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VoteAccountInfo {
    vote_pubkey: String,
    node_pubkey: String,
    activated_stake: u64,
    commission: u8,
    epoch_vote_account: bool,
    epoch_credits: Vec<(u64, u64, u64)>, // (epoch, credits, previous_credits)
    last_vote: u64,
    root_slot: u64,
}

#[derive(Debug, Serialize)]
struct RpcRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    params: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct RpcResponse<T> {
    result: T,
}

// Hardcoded high-quality validators with static fallback data
fn get_static_validators() -> Vec<ValidatorInfo> {
    vec![
        ValidatorInfo {
            identity: "UNrgBLmc8JT6A3dxXY9DWeHvDezt2DZQbhg1KPQfqEL".to_string(), // Replace with your actual validator identity
            vote_account: "unRgBLTLNXdBmenHXNPAg3AMn3KWcV3Mk4eoZBmTrdk".to_string(), // Replace with your actual vote account
            name: "Unruggable Validator".to_string(),
            description: "Support us and keep Unruggable open source and free.".to_string(),
            commission: 5.0,
            active_stake: 100.0,
            skip_rate: 0.5,
            is_default: true,
        },
        ValidatorInfo {
            identity: "6xWLi1TDSh65fWsSqE1zdvANTSuVDRMx4ghsGJwgunS8".to_string(),
            vote_account: "BbM5kJgrwEj3tYFfBPnjcARB54wDUHkXmLUTkazUmt2x".to_string(),
            name: "Jito Validator".to_string(),
            description: "High-performance Jito validator with 99.99% voting rate and MEV optimization".to_string(),
            commission: 0.0,
            active_stake: 253219.0, // From the data you provided
            skip_rate: 1.0, // Very low estimate given 99.99% voting rate
            is_default: false,
        },
        ValidatorInfo {
            identity: "HEL1USMZKAL2odpNBj2oCjffnFGaYwmbGmyewGv1e2TU".to_string(),
            vote_account: "he1iusunGwqrNtafDtLdhsUQDFvo13z9sUa36PauBtk".to_string(),
            name: "Helius".to_string(),
            description: "High-performance RPC provider and validator".to_string(),
            commission: 0.0,
            active_stake: 13453011.453622909,
            skip_rate: 2.5, // Static estimate
            is_default: false,
        },
        // Love validator
        ValidatorInfo {
            identity: "Love31pnbDJNVzZZVbtV4h2ftvTPVcBpXW11BSTCa6s".to_string(),
            vote_account: "Love31JHTTweTzCu3BjyjhXJadjRrd57hiNZn7M1fLj".to_string(),
            name: "Love Validator".to_string(),
            description: "You are loved, and you have a home here.".to_string(),
            commission: 0.0,
            active_stake: 0.0,
            skip_rate: 2.0, // Static estimate
            is_default: false,
        },
        ValidatorInfo {
            identity: "DRpbCBMxVnDK7maPM5tGv6MvB3v1sRMC86PZ8okm21hy".to_string(),
            vote_account: "3N7s9zXMZ4QqvHQR15t5GNHyqc89KduzMP7423eWiD5g".to_string(),
            name: "Solana Foundation".to_string(),
            description: "Official Solana Foundation validator".to_string(),
            commission: 2.0,
            active_stake: 13061017.501494104,
            skip_rate: 1.5, // Static estimate - typically very good
            is_default: false,
        },
        // Main Phase Labs node
        ValidatorInfo {
            identity: "phz1CRbEsCtFCh2Ro5tjyu588VU1WPMwW9BJS9yFNn2".to_string(),
            vote_account: "phz34EcgWRCT9otPzRS2JtSzVHxQJk4SovqJvV1TQk8".to_string(),
            name: "Main Phase Labs".to_string(),
            description: "Innovative blockchain infrastructure provider".to_string(),
            commission: 5.0,
            active_stake: 0.0,
            skip_rate: 3.0, // Static estimate
            is_default: false,
        },        
        ValidatorInfo {
            identity: "radM7PKUpZwJ9bYPAJ7V8FXHeUmH1zim6iaXUKkftP9".to_string(),
            vote_account: "radYEig9KGrMTMWbWRFV7LStotQbnLgPaEFHVDsudQz".to_string(),
            name: "Radiants".to_string(),
            description: "Community-focused validator with reliable performance".to_string(),
            commission: 5.0,
            active_stake: 0.0,
            skip_rate: 2.5, // Static estimate
            is_default: false,
        },
        // Institutional Validator for SOC2 secured staking
        //ValidatorInfo {
        //    identity: "ciTyjzN9iyobidMycjyqRRM7vXAHXkFzH3m8vEr6cQj".to_string(),
        //    vote_account: "CiTYUYPAPHdcri5yEfsmqVcs54J6j8X1QaiFLgYqMVe".to_string(),
        //    name: "Institutional Validator".to_string(),
        //    description: "SOC2 secured institutional-grade staking validator".to_string(),
        //    commission: 5.0,
        //    active_stake: 0.0,
        //    skip_rate: 2.0, // Static estimate
        //    is_default: false,
        //},
        // Validator supporting creatives
        //ValidatorInfo {
        //    identity: "YE11a5nVJtUNqsojkphYuWc7StqBzbCeFH6BjhAAUEV".to_string(),
        //    vote_account: "YE111yizdzBA7JQKMXjy9VSx1shKAczUbs3b3e6vKQH".to_string(),
        //    name: "Creative Arts Validator".to_string(),
        //    description: "Supporting creative communities and artists in the Solana ecosystem".to_string(),
        //    commission: 5.0,
        //    active_stake: 0.0,
        //    skip_rate: 3.0, // Static estimate
        //    is_default: false,
        //},
        // 0% commission, 0% MEV node
        //ValidatorInfo {
        //    identity: "gojir4WnhS7VS1JdbnanJMzaMfr4UD7KeX1ixWAHEmw".to_string(),
        //    vote_account: "goJiRADNdmfnJ4iWEyft7KaYMPTVsRba2Ee1akDEBXb".to_string(),
        //    name: "Zero Commission Validator".to_string(),
        //    description: "High-quality validator with 0% commission and 0% MEV extraction".to_string(),
        //    commission: 0.0,
        //    active_stake: 0.0,
        //    skip_rate: 1.8, // Static estimate - likely very good
        //    is_default: false,
        //},
    ]
}

/// Main function to get recommended validators with live data
/// This should be called whenever the stake modal is opened
pub async fn get_recommended_validators() -> Vec<ValidatorInfo> {
    println!("🔍 Fetching live validator data...");
    
    match fetch_live_validator_data(None).await {
        Ok(validators) => {
            println!("✅ Successfully fetched live validator data for {} validators", validators.len());
            validators
        },
        Err(e) => {
            println!("❌ Failed to fetch live validator data: {}", e);
            println!("📋 Falling back to static validator data");
            get_static_validators()
        }
    }
}

/// Simplified validator data fetching - only use direct RPC values
async fn fetch_live_validator_data(rpc_url: Option<&str>) -> Result<Vec<ValidatorInfo>, Box<dyn std::error::Error>> {
    let client = Client::new();
    let url = rpc_url.unwrap_or("https://serene-stylish-mound.solana-mainnet.quiknode.pro/5489821bcd1547d9cd7b2d81f90c086e36e0e9f7/");
    
    println!("🌐 Calling getVoteAccounts RPC method...");
    
    // Get all vote accounts from the network
    let request = RpcRequest {
        jsonrpc: "2.0".to_string(),
        id: 1,
        method: "getVoteAccounts".to_string(),
        params: vec![
            serde_json::json!({
                "commitment": "finalized"
            })
        ],
    };
    
    let response = client
        .post(url)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await?;
    
    if !response.status().is_success() {
        return Err(format!("RPC error: {}", response.status()).into());
    }
    
    let json: serde_json::Value = response.json().await?;
    
    // Check for errors in the response
    if let Some(error) = json.get("error") {
        return Err(format!("RPC error: {:?}", error).into());
    }
    
    // Parse the vote accounts
    let rpc_response: RpcResponse<VoteAccountsResponse> = serde_json::from_value(json)?;
    
    println!("📊 Found {} current validators and {} delinquent validators", 
        rpc_response.result.current.len(), 
        rpc_response.result.delinquent.len()
    );
    
    // Create a HashMap for quick lookup of live data by vote account
    let mut live_data: HashMap<String, VoteAccountInfo> = HashMap::new();
    
    // Add both current and delinquent validators to our lookup
    for vote_account in rpc_response.result.current {
        live_data.insert(vote_account.vote_pubkey.clone(), vote_account);
    }
    for vote_account in rpc_response.result.delinquent {
        live_data.insert(vote_account.vote_pubkey.clone(), vote_account);
    }
    
    // Get our curated validator list
    let mut validators = get_static_validators();
    
    println!("🔄 Updating {} curated validators with live data:", validators.len());
    
    // Update each validator with ONLY direct RPC data
    for validator in &mut validators {
        if let Some(live_info) = live_data.get(&validator.vote_account) {
            // Store old values for comparison
            let old_commission = validator.commission;
            let old_stake = validator.active_stake;
            
            // Update with ONLY direct RPC data - no calculations
            validator.commission = live_info.commission as f64;
            validator.active_stake = live_info.activated_stake as f64 / 1_000_000_000.0; // Convert lamports to SOL
            // Keep skip_rate as static value from our list (or set to 0 if you want to remove it)
            
            //println!("  ✅ {} ({})", validator.name, validator.vote_account);
            //println!("     Commission: {:.1}% -> {:.1}%", old_commission, validator.commission);
            //println!("     Active Stake: {:.2} SOL -> {:.2} SOL", old_stake, validator.active_stake);
            //println!("     Skip Rate: Using static value {:.1}% (no live data available)", validator.skip_rate);
        } else {
            println!("  ⚠️  {} ({}): No live data found - keeping static values", 
                validator.name, validator.vote_account);
        }
    }
    
    println!("🎯 Live validator data update completed!");
    Ok(validators)
}

// Legacy function for backward compatibility - now just calls the async version
// This can be removed once you update all calling code
pub fn get_recommended_validators_sync() -> Vec<ValidatorInfo> {
    println!("⚠️  Warning: Using synchronous validator data (static fallback)");
    get_static_validators()
}

// Function to fetch live validator data - this replaces the old implementation
pub async fn fetch_live_validators(rpc_url: Option<&str>) -> Result<Vec<ValidatorInfo>, Box<dyn std::error::Error>> {
    // get_recommended_validators already handles errors internally and returns Vec<ValidatorInfo>
    // It falls back to static data if live data fails, so it never fails
    Ok(get_recommended_validators().await)
}