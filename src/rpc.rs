use reqwest::Client;
use serde::{Deserialize, Serialize};

const DEFAULT_RPC_URL: &str = "https://serene-stylish-mound.solana-mainnet.quiknode.pro/5489821bcd1547d9cd7b2d81f90c086e36e0e9f7/";

#[derive(Debug, Serialize)]
struct RpcRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    params: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct RpcResponse<T> {
    jsonrpc: String,
    result: T,
    id: u64,
}

#[derive(Debug, Deserialize)]
struct BalanceResult {
    context: RpcContext,
    value: u64,
}

#[derive(Debug, Deserialize)]
struct RpcContext {
    #[allow(dead_code)]
    slot: u64,
}

pub async fn get_balance(address: &str, rpc_url: Option<&str>) -> Result<f64, String> {
    let client = Client::new();
    let url = rpc_url.unwrap_or(DEFAULT_RPC_URL);
    
    let request = RpcRequest {
        jsonrpc: "2.0".to_string(),
        id: 1,
        method: "getBalance".to_string(),
        params: vec![
            serde_json::Value::String(address.to_string()),
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
        .await
        .map_err(|e| format!("Failed to send request: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("RPC error: {}", response.status()));
    }
    
    let json: serde_json::Value = response.json().await
        .map_err(|e| format!("Failed to parse response: {}", e))?;
    
    // Check for errors in the response
    if let Some(error) = json.get("error") {
        return Err(format!("RPC error: {:?}", error));
    }
    
    // Parse the result
    if let Some(result) = json.get("result") {
        if let Some(value) = result.get("value") {
            if let Some(val) = value.as_u64() {
                return Ok(val as f64 / 1_000_000_000.0);
            }
        }
    }
    
    Err(format!("Failed to parse balance from response: {:?}", json))
}

#[derive(Debug, Deserialize)]
struct TokenAccountsResult {
    context: RpcContext,
    value: Vec<TokenAccount>,
}

#[derive(Debug, Deserialize)]
struct TokenAccount {
    account: AccountData,
    pubkey: String,
}

#[derive(Debug, Deserialize)]
struct AccountData {
    data: ParsedData,
    executable: bool,
    lamports: u64,
    owner: String,
    #[serde(rename = "rentEpoch", default)]
    rent_epoch: Option<u64>, // Made optional with default value
    space: u64,
}

#[derive(Debug, Deserialize)]
struct ParsedData {
    parsed: ParsedInfo,
    program: String,
    space: u64,
}

#[derive(Debug, Deserialize)]
struct ParsedInfo {
    info: TokenInfo,
    #[serde(rename = "type")]
    account_type: String,
}

#[derive(Debug, Deserialize)]
struct TokenInfo {
    #[serde(rename = "isNative")]
    is_native: bool,
    mint: String,
    owner: String,
    state: String,
    #[serde(rename = "tokenAmount")]
    token_amount: TokenAmount,
}

#[derive(Debug, Deserialize)]
struct TokenAmount {
    amount: String,
    decimals: u8,
    #[serde(rename = "uiAmount")]
    ui_amount: f64,
    #[serde(rename = "uiAmountString")]
    ui_amount_string: String,
}

/// Parameters for filtering token accounts by mint or program ID.
#[derive(Debug, Serialize)]
pub enum TokenAccountFilter {
    Mint(String),
    ProgramId(String),
}

/// Struct to return token account details in a user-friendly format.
#[derive(Debug, Serialize)]
pub struct TokenAccountInfo {
    pub pubkey: String,
    pub mint: String,
    pub owner: String,
    pub amount: f64,
    pub decimals: u8,
    pub state: String,
}

/// Fetches token accounts owned by the specified address, filtered by mint or program ID.
pub async fn get_token_accounts_by_owner(
    address: &str,
    filter: Option<TokenAccountFilter>,
    rpc_url: Option<&str>,
) -> Result<Vec<TokenAccountInfo>, String> {
    let client = Client::new();
    let url = rpc_url.unwrap_or(DEFAULT_RPC_URL);

    let filter_param = match filter {
        Some(TokenAccountFilter::Mint(mint)) => serde_json::json!({ "mint": mint }),
        Some(TokenAccountFilter::ProgramId(program_id)) => serde_json::json!({ "programId": program_id }),
        None => serde_json::json!({}),
    };

    let request = RpcRequest {
        jsonrpc: "2.0".to_string(),
        id: 1,
        method: "getTokenAccountsByOwner".to_string(),
        params: vec![
            serde_json::Value::String(address.to_string()),
            filter_param,
            serde_json::json!({
                "encoding": "jsonParsed",
                "commitment": "finalized"
            }),
        ],
    };

    let response = client
        .post(url)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Failed to send request: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("RPC error: {}", response.status()));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    // Check for errors in the response
    if let Some(error) = json.get("error") {
        return Err(format!("RPC error: {:?}", error));
    }

    // Deserialize the result
    let rpc_response: RpcResponse<TokenAccountsResult> = serde_json::from_value(json)
        .map_err(|e| format!("Failed to deserialize response: {}", e))?;

    // Map the results to a user-friendly format
    let token_accounts = rpc_response
        .result
        .value
        .into_iter()
        .map(|account| TokenAccountInfo {
            pubkey: account.pubkey,
            mint: account.account.data.parsed.info.mint,
            owner: account.account.data.parsed.info.owner,
            amount: account.account.data.parsed.info.token_amount.ui_amount,
            decimals: account.account.data.parsed.info.token_amount.decimals,
            state: account.account.data.parsed.info.state,
        })
        .collect();

    Ok(token_accounts)
}