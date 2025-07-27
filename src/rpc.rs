use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::error::Error;

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
            serde_json::json!({ "commitment": "finalized" }),
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

    let json: Value = response.json().await.map_err(|e| format!("Failed to parse response: {}", e))?;

    if let Some(error) = json.get("error") {
        return Err(format!("RPC error: {:?}", error));
    }

    if let Some(result) = json.get("result") {
        if let Some(value) = result.get("value") {
            if let Some(val) = value.as_u64() {
                return Ok(val as f64 / 1_000_000_000.0);
            }
        }
    }

    Err(format!("Failed to parse balance from response: {:?}", json))
}

pub async fn get_minimum_balance_for_rent_exemption(
    account_size: usize,
    rpc_url: Option<&str>,
) -> Result<u64, Box<dyn Error>> {
    let client = Client::new();
    let url = rpc_url.unwrap_or(DEFAULT_RPC_URL);

    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getMinimumBalanceForRentExemption",
        "params": [account_size]
    });

    let response = client
        .post(url)
        .json(&request)
        .send()
        .await?;

    let json: Value = response.json().await?;
    Ok(json["result"].as_u64().ok_or("Invalid rent exemption response")?)
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

/// Transaction history related structs
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransactionHistoryItem {
    pub signature: String,
    pub slot: u64,
    #[serde(rename = "blockTime")]
    pub block_time: Option<i64>,
    #[serde(rename = "confirmationStatus")]
    pub confirmation_status: Option<String>,
    pub err: Option<serde_json::Value>,
    pub memo: Option<String>,
}

/// Convert a timestamp to a human-readable date/time
pub fn format_timestamp(timestamp: i64) -> String {
    let datetime = chrono::NaiveDateTime::from_timestamp_opt(timestamp, 0)
        .unwrap_or_else(|| chrono::NaiveDateTime::from_timestamp_opt(0, 0).unwrap());
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Gets a simplified transaction item with decoded info useful for UI display
#[derive(Debug, Clone, Serialize)]
pub struct TransactionInfo {
    pub signature: String,
    pub timestamp: String,
    pub time_ago: String,
    pub status: String,
    pub raw_status: String,
    pub memo: Option<String>,
    pub error: Option<String>,
}

/// Fetches transactions history for a given address
pub async fn get_transaction_history(
    address: &str,
    limit: usize,
    rpc_url: Option<&str>,
) -> Result<Vec<TransactionInfo>, String> {
    let client = Client::new();
    let url = rpc_url.unwrap_or(DEFAULT_RPC_URL);
    
    // Default to 20 transactions or user-requested limit (max 50 to avoid too much data)
    let limit = limit.min(50).max(1);
    
    let request = RpcRequest {
        jsonrpc: "2.0".to_string(),
        id: 1,
        method: "getSignaturesForAddress".to_string(),
        params: vec![
            serde_json::Value::String(address.to_string()),
            serde_json::json!({
                "limit": limit,
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
    
    // Get the result
    if let Some(result) = json.get("result") {
        // Parse the result as a Vec<TransactionHistoryItem>
        let transactions: Vec<TransactionHistoryItem> = serde_json::from_value(result.clone())
            .map_err(|e| format!("Failed to parse transactions: {}", e))?;
        
        // Get current timestamp for "time ago" calculations
        let current_time = chrono::Utc::now().timestamp();
        
        // Convert to TransactionInfo
        let transactions_info = transactions
            .into_iter()
            .map(|tx| {
                let timestamp = if let Some(block_time) = tx.block_time {
                    let formatted = format_timestamp(block_time);
                    formatted
                } else {
                    "Unknown time".to_string()
                };
                
                // Calculate time ago
                let time_ago = if let Some(block_time) = tx.block_time {
                    let diff = current_time - block_time;
                    if diff < 60 {
                        format!("{} seconds ago", diff)
                    } else if diff < 3600 {
                        format!("{} minutes ago", diff / 60)
                    } else if diff < 86400 {
                        format!("{} hours ago", diff / 3600)
                    } else {
                        format!("{} days ago", diff / 86400)
                    }
                } else {
                    "Unknown time".to_string()
                };
                
                // Determine status
                let status = if let Some(_err) = &tx.err {
                    "Failed".to_string()
                } else {
                    "Success".to_string()
                };
                
                let raw_status = tx.confirmation_status
                    .unwrap_or_else(|| "unknown".to_string());
                
                // Extract error message if any
                let error = if let Some(err) = tx.err {
                    let err_str = format!("{:?}", err);
                    if err_str.len() > 100 {
                        Some(format!("{}...", &err_str[..100]))
                    } else {
                        Some(err_str)
                    }
                } else {
                    None
                };
                
                TransactionInfo {
                    signature: tx.signature,
                    timestamp,
                    time_ago,
                    status,
                    raw_status,
                    memo: tx.memo,
                    error,
                }
            })
            .collect();
        
        Ok(transactions_info)
    } else {
        Err("Failed to get transactions from response".to_string())
    }
}

/// Gets detailed information about a specific transaction
pub async fn get_transaction_details(
    signature: &str,
    rpc_url: Option<&str>,
) -> Result<HashMap<String, serde_json::Value>, String> {
    let client = Client::new();
    let url = rpc_url.unwrap_or(DEFAULT_RPC_URL);
    
    let request = RpcRequest {
        jsonrpc: "2.0".to_string(),
        id: 1,
        method: "getTransaction".to_string(),
        params: vec![
            serde_json::Value::String(signature.to_string()),
            serde_json::json!({
                "encoding": "jsonParsed",
                "commitment": "finalized",
                "maxSupportedTransactionVersion": 0
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
    
    // Extract the result
    if let Some(result) = json.get("result") {
        if result.is_null() {
            return Err("Transaction not found".to_string());
        }
        
        // Extract useful information to show in UI
        let mut details = HashMap::new();
        
        // Add basic transaction info
        if let Some(slot) = result.get("slot") {
            details.insert("slot".to_string(), slot.clone());
        }
        
        if let Some(block_time) = result.get("blockTime") {
            if let Some(time) = block_time.as_i64() {
                details.insert("blockTime".to_string(), block_time.clone());
                details.insert("formattedTime".to_string(), 
                    serde_json::Value::String(format_timestamp(time)));
            }
        }
        
        // Add transaction data
        if let Some(meta) = result.get("meta") {
            details.insert("meta".to_string(), meta.clone());
            
            // Extract fee
            if let Some(fee) = meta.get("fee") {
                if let Some(fee_val) = fee.as_u64() {
                    details.insert("feeSOL".to_string(), 
                        serde_json::Value::String(format!("{:.9}", fee_val as f64 / 1_000_000_000.0)));
                }
            }
            
            // Extract status
            if let Some(err) = meta.get("err") {
                if err.is_null() {
                    details.insert("status".to_string(), 
                        serde_json::Value::String("Success".to_string()));
                } else {
                    details.insert("status".to_string(), 
                        serde_json::Value::String("Failed".to_string()));
                    details.insert("error".to_string(), err.clone());
                }
            } else {
                details.insert("status".to_string(), 
                    serde_json::Value::String("Unknown".to_string()));
            }
        }
        
        // Add transaction instructions
        if let Some(transaction) = result.get("transaction") {
            if let Some(message) = transaction.get("message") {
                details.insert("message".to_string(), message.clone());
                
                // Extract instructions
                if let Some(instructions) = message.get("instructions") {
                    details.insert("instructions".to_string(), instructions.clone());
                }
            }
        }
        
        Ok(details)
    } else {
        Err("Failed to get transaction details from response".to_string())
    }
}