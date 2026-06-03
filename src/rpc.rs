#![allow(dead_code)]

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::error::Error;

const DEFAULT_RPC_URL: &str = "https://johna-k3cr1v-fast-mainnet.helius-rpc.com";
const COLLECTIBLES_PAGE_LIMIT: usize = 250;
const MAX_COLLECTIBLES_PAGES: usize = 8;

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

    let json: Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

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

pub async fn get_balances(
    addresses: &[String],
    rpc_url: Option<&str>,
) -> Result<HashMap<String, f64>, String> {
    if addresses.is_empty() {
        return Ok(HashMap::new());
    }

    let client = Client::new();
    let url = rpc_url.unwrap_or(DEFAULT_RPC_URL);
    const MAX_ACCOUNTS_PER_REQUEST: usize = 100;
    let mut balances = HashMap::with_capacity(addresses.len());

    for (batch_idx, chunk) in addresses.chunks(MAX_ACCOUNTS_PER_REQUEST).enumerate() {
        let request = RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: (batch_idx + 1) as u64,
            method: "getMultipleAccounts".to_string(),
            params: vec![
                serde_json::json!(chunk),
                serde_json::json!({
                    "commitment": "finalized",
                    "encoding": "base64"
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

        let json: Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        if let Some(error) = json.get("error") {
            return Err(format!("RPC error: {:?}", error));
        }

        let values = json
            .get("result")
            .and_then(|result| result.get("value"))
            .and_then(|value| value.as_array())
            .ok_or_else(|| format!("Failed to parse balances from response: {:?}", json))?;

        for (idx, address) in chunk.iter().enumerate() {
            let lamports = values
                .get(idx)
                .and_then(|account| account.get("lamports"))
                .and_then(|lamports| lamports.as_u64())
                .unwrap_or(0);

            balances.insert(address.clone(), lamports as f64 / 1_000_000_000.0);
        }
    }

    Ok(balances)
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

    let response = client.post(url).json(&request).send().await?;

    let json: Value = response.json().await?;
    Ok(json["result"]
        .as_u64()
        .ok_or("Invalid rent exemption response")?)
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
    #[allow(dead_code)]
    amount: String,
    decimals: u8,
    #[serde(rename = "uiAmount")]
    ui_amount: Option<f64>,
    #[serde(rename = "uiAmountString", default)]
    ui_amount_string: String,
}

fn parse_ui_token_amount(token_amount: &TokenAmount) -> f64 {
    if let Some(ui_amount) = token_amount.ui_amount {
        return ui_amount;
    }

    if let Ok(ui_amount_from_string) = token_amount.ui_amount_string.parse::<f64>() {
        return ui_amount_from_string;
    }

    if let Ok(base_units) = token_amount.amount.parse::<u128>() {
        let divisor = 10_f64.powi(i32::from(token_amount.decimals));
        if divisor > 0.0 {
            return (base_units as f64) / divisor;
        }
    }

    0.0
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
        Some(TokenAccountFilter::ProgramId(program_id)) => {
            serde_json::json!({ "programId": program_id })
        }
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
        .map(|account| {
            let token_amount = &account.account.data.parsed.info.token_amount;
            TokenAccountInfo {
                pubkey: account.pubkey,
                mint: account.account.data.parsed.info.mint,
                owner: account.account.data.parsed.info.owner,
                amount: parse_ui_token_amount(token_amount),
                decimals: token_amount.decimals,
                state: account.account.data.parsed.info.state,
            }
        })
        .collect();

    Ok(token_accounts)
}

// =================== STAKE ACCOUNT SUPPORT ===================

/// Stake account specific structures for parsing getProgramAccounts response
#[derive(Debug, Deserialize)]
pub struct StakeAccountRpcData {
    pub account: StakeAccountData,
    pub pubkey: String,
}

#[derive(Debug, Deserialize)]
pub struct StakeAccountData {
    pub data: StakeParsedData,
    pub executable: bool,
    pub lamports: u64,
    pub owner: String,
    #[serde(rename = "rentEpoch")]
    pub rent_epoch: u64,
    pub space: u64,
}

#[derive(Debug, Deserialize)]
pub struct StakeParsedData {
    pub parsed: StakeParsedInfo,
    pub program: String,
    pub space: u64,
}

#[derive(Debug, Deserialize)]
pub struct StakeParsedInfo {
    pub info: StakeInfo,
    #[serde(rename = "type")]
    pub account_type: String,
}

#[derive(Debug, Deserialize)]
pub struct StakeInfo {
    pub meta: StakeMeta,
    pub stake: Option<StakeDetails>,
}

#[derive(Debug, Deserialize)]
pub struct StakeMeta {
    pub authorized: StakeAuthorized,
    pub lockup: StakeLockup,
    #[serde(rename = "rentExemptReserve")]
    pub rent_exempt_reserve: String,
}

#[derive(Debug, Deserialize)]
pub struct StakeAuthorized {
    pub staker: String,
    pub withdrawer: String,
}

#[derive(Debug, Deserialize)]
pub struct StakeLockup {
    pub custodian: String,
    pub epoch: u64,
    #[serde(rename = "unixTimestamp")]
    pub unix_timestamp: u64,
}

#[derive(Debug, Deserialize)]
pub struct StakeDetails {
    #[serde(rename = "creditsObserved")]
    pub credits_observed: u64,
    pub delegation: StakeDelegation,
}

#[derive(Debug, Deserialize)]
pub struct StakeDelegation {
    #[serde(rename = "activationEpoch")]
    pub activation_epoch: String,
    #[serde(rename = "deactivationEpoch")]
    pub deactivation_epoch: String,
    pub stake: String,
    pub voter: String,
    #[serde(rename = "warmupCooldownRate")]
    pub warmup_cooldown_rate: f64,
}

/// Epoch information structure
#[derive(Debug, Deserialize)]
pub struct EpochInfo {
    #[serde(rename = "absoluteSlot")]
    pub absolute_slot: u64,
    #[serde(rename = "blockHeight")]
    pub block_height: u64,
    pub epoch: u64,
    #[serde(rename = "slotIndex")]
    pub slot_index: u64,
    #[serde(rename = "slotsInEpoch")]
    pub slots_in_epoch: u64,
    #[serde(rename = "transactionCount")]
    pub transaction_count: Option<u64>,
}

/// Fetches all stake accounts owned by the specified wallet address
pub async fn get_stake_accounts_by_owner(
    wallet_address: &str,
    rpc_url: Option<&str>,
) -> Result<Vec<StakeAccountRpcData>, String> {
    let client = Client::new();
    let url = rpc_url.unwrap_or(DEFAULT_RPC_URL);

    println!("🔍 Fetching stake accounts for wallet: {}", wallet_address);

    let request = RpcRequest {
        jsonrpc: "2.0".to_string(),
        id: 1,
        method: "getProgramAccounts".to_string(),
        params: vec![
            serde_json::Value::String("Stake11111111111111111111111111111111111111".to_string()),
            serde_json::json!({
                "encoding": "jsonParsed",
                "filters": [
                    {
                        "memcmp": {
                            "offset": 44,
                            "bytes": wallet_address
                        }
                    }
                ]
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
    let rpc_response: RpcResponse<Vec<StakeAccountRpcData>> = serde_json::from_value(json)
        .map_err(|e| format!("Failed to deserialize response: {}", e))?;

    println!("✅ Found {} stake accounts", rpc_response.result.len());
    Ok(rpc_response.result)
}

/// Get current epoch information (useful for determining activation status)
pub async fn get_epoch_info(rpc_url: Option<&str>) -> Result<EpochInfo, String> {
    let client = Client::new();
    let url = rpc_url.unwrap_or(DEFAULT_RPC_URL);

    let request = RpcRequest {
        jsonrpc: "2.0".to_string(),
        id: 1,
        method: "getEpochInfo".to_string(),
        params: vec![],
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
    let rpc_response: RpcResponse<EpochInfo> = serde_json::from_value(json)
        .map_err(|e| format!("Failed to deserialize response: {}", e))?;

    Ok(rpc_response.result)
}

// =================== EXISTING TRANSACTION HISTORY CODE ===================

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
    let datetime = chrono::DateTime::from_timestamp(timestamp, 0)
        .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
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

                let raw_status = tx
                    .confirmation_status
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
                details.insert(
                    "formattedTime".to_string(),
                    serde_json::Value::String(format_timestamp(time)),
                );
            }
        }

        // Add transaction data
        if let Some(meta) = result.get("meta") {
            details.insert("meta".to_string(), meta.clone());

            // Extract fee
            if let Some(fee) = meta.get("fee") {
                if let Some(fee_val) = fee.as_u64() {
                    details.insert(
                        "feeSOL".to_string(),
                        serde_json::Value::String(format!(
                            "{:.9}",
                            fee_val as f64 / 1_000_000_000.0
                        )),
                    );
                }
            }

            // Extract status
            if let Some(err) = meta.get("err") {
                if err.is_null() {
                    details.insert(
                        "status".to_string(),
                        serde_json::Value::String("Success".to_string()),
                    );
                } else {
                    details.insert(
                        "status".to_string(),
                        serde_json::Value::String("Failed".to_string()),
                    );
                    details.insert("error".to_string(), err.clone());
                }
            } else {
                details.insert(
                    "status".to_string(),
                    serde_json::Value::String("Unknown".to_string()),
                );
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

// NFT with DAS from helius Struts

#[derive(Debug, Clone, PartialEq)]
pub struct CollectibleInfo {
    pub mint: String,
    pub name: String,
    pub collection: String,
    pub image: String,
    pub description: Option<String>,
    pub verified: bool,
}

#[derive(Debug, Deserialize)]
struct DasResponse {
    jsonrpc: String,
    result: DasResult,
    id: String,
}

#[derive(Debug, Deserialize)]
struct DasResult {
    total: u32,
    limit: u32,
    page: u32,
    items: Vec<DasAsset>,
}

#[derive(Debug, Deserialize)]
struct DasAsset {
    id: String,
    content: Option<DasContent>,
    grouping: Option<Vec<DasGrouping>>,
    ownership: Option<DasOwnership>,
    burnt: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct DasContent {
    #[serde(rename = "json_uri")]
    json_uri: Option<String>,
    files: Option<Vec<DasFile>>,
    metadata: Option<DasMetadata>,
}

#[derive(Debug, Deserialize)]
struct DasFile {
    uri: Option<String>,
    #[serde(rename = "cdn_uri")]
    cdn_uri: Option<String>,
    mime: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DasMetadata {
    name: Option<String>,
    description: Option<String>,
    image: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DasGrouping {
    group_key: String,
    group_value: String,
}

#[derive(Debug, Deserialize)]
struct DasOwnership {
    frozen: Option<bool>,
    delegated: Option<bool>,
    owner: String,
}

/// Fetches collectibles (NFTs) for a wallet using Helius DAS API
pub async fn fetch_collectibles(
    wallet_address: &str,
    rpc_url: Option<&str>,
) -> Result<Vec<CollectibleInfo>, String> {
    let client = Client::new();
    let url = rpc_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_RPC_URL);

    println!("🎨 Fetching collectibles for wallet: {}", wallet_address);

    let mut page = 1usize;
    let mut seen_asset_ids = HashSet::new();
    let mut collectibles = Vec::new();
    let mut filtered_out = 0usize;

    loop {
        let request_body = json!({
            "jsonrpc": "2.0",
            "id": format!("collectibles-{page}"),
            "method": "getAssetsByOwner",
            "params": {
                "ownerAddress": wallet_address,
                "page": page,
                "limit": COLLECTIBLES_PAGE_LIMIT,
                "displayOptions": {
                    "showFungible": false,
                    "showNativeBalance": false,
                    "showInscription": false
                }
            }
        });

        let response = client
            .post(url)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch collectibles: {}", e))?;

        if !response.status().is_success() {
            return Err(format!(
                "Collectibles RPC returned HTTP {}",
                response.status()
            ));
        }

        let payload: Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to decode collectibles response: {}", e))?;

        if let Some(error) = payload.get("error") {
            return Err(format!("Collectibles RPC error: {:?}", error));
        }

        let result = payload
            .get("result")
            .ok_or_else(|| "Collectibles response was missing result".to_string())?;
        let items = result
            .get("items")
            .and_then(Value::as_array)
            .ok_or_else(|| "Collectibles response was missing items".to_string())?;

        for item in items {
            if let Some(parsed) = parse_collectible_info(item, wallet_address) {
                if seen_asset_ids.insert(parsed.mint.clone()) {
                    collectibles.push(parsed);
                }
            } else {
                filtered_out += 1;
            }
        }

        let total = result
            .get("total")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        let fetched = (page * COLLECTIBLES_PAGE_LIMIT) as u64;
        let reached_end = items.len() < COLLECTIBLES_PAGE_LIMIT
            || total > 0 && fetched >= total
            || page >= MAX_COLLECTIBLES_PAGES;

        if reached_end {
            break;
        }

        page += 1;
    }

    collectibles.sort_by(|left, right| {
        right
            .verified
            .cmp(&left.verified)
            .then_with(|| {
                left.collection
                    .to_lowercase()
                    .cmp(&right.collection.to_lowercase())
            })
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });

    println!(
        "✅ Converted to {} collectible items after filtering {} entries",
        collectibles.len(),
        filtered_out
    );
    Ok(collectibles)
}

fn parse_collectible_info(asset: &Value, wallet_address: &str) -> Option<CollectibleInfo> {
    if nested_bool(asset, &["burnt"]).unwrap_or(false) {
        return None;
    }

    let owner = nested_string(asset, &["ownership", "owner"])?;
    if !owner.eq_ignore_ascii_case(wallet_address) {
        return None;
    }

    if is_fungible_asset(asset) {
        return None;
    }

    let mint = nested_string(asset, &["id"])?;
    let name = nested_string(asset, &["content", "metadata", "name"])
        .or_else(|| nested_string(asset, &["content", "metadata", "symbol"]))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())?;
    let description = nested_string(asset, &["content", "metadata", "description"])
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let image = extract_collectible_image(asset)?;

    let (collection, verified_collection) = parse_collection_metadata(asset, &name);
    let verified_creator = has_verified_creator(asset);
    let verified = verified_collection || verified_creator;

    if is_likely_spam(asset, &name, &collection, description.as_deref(), verified) {
        return None;
    }

    Some(CollectibleInfo {
        mint,
        name,
        collection,
        image,
        description,
        verified,
    })
}

fn parse_collection_metadata(asset: &Value, name: &str) -> (String, bool) {
    let mut collection_name = None;
    let mut verified = false;

    if let Some(entries) = asset.get("grouping").and_then(Value::as_array) {
        for entry in entries {
            let group_key = nested_string(entry, &["group_key"]).unwrap_or_default();
            if group_key != "collection" {
                continue;
            }

            let group_value = nested_string(entry, &["group_value"])
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            let metadata_name = nested_string(entry, &["collection_metadata", "name"])
                .or_else(|| nested_string(entry, &["collection_metadata", "symbol"]))
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            verified = entry
                .get("verified")
                .and_then(Value::as_bool)
                .unwrap_or(false);

            if collection_name.is_none() {
                collection_name =
                    metadata_name.or_else(|| group_value.filter(|value| !looks_like_pubkey(value)));
            }
            break;
        }
    }

    if collection_name.is_none() {
        collection_name = nested_string(asset, &["content", "metadata", "collection", "name"])
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
    }

    if collection_name.is_none() {
        collection_name = derive_collection_name(name);
    }

    (
        collection_name.unwrap_or_else(|| name.to_string()),
        verified,
    )
}

fn extract_collectible_image(asset: &Value) -> Option<String> {
    let files = asset
        .get("content")
        .and_then(|content| content.get("files"))
        .and_then(Value::as_array);

    if let Some(files) = files {
        for file in files {
            let mime = file
                .get("mime")
                .and_then(Value::as_str)
                .map(|value| value.to_ascii_lowercase());
            let file_url = file
                .get("cdn_uri")
                .and_then(Value::as_str)
                .or_else(|| file.get("uri").and_then(Value::as_str))
                .map(normalize_media_url)
                .filter(|value| !value.is_empty());
            let is_image_mime = mime
                .as_deref()
                .map(|value| value.starts_with("image/"))
                .unwrap_or(false);
            if let Some(url) = file_url {
                if is_image_mime || looks_like_image_url(&url) {
                    return Some(url);
                }
            }
        }
    }

    nested_string(asset, &["content", "links", "image"])
        .map(|value| normalize_media_url(&value))
        .filter(|value| looks_like_image_url(value))
        .or_else(|| {
            nested_string(asset, &["content", "metadata", "image"])
                .map(|value| normalize_media_url(&value))
                .filter(|value| looks_like_image_url(value))
        })
}

fn has_verified_creator(asset: &Value) -> bool {
    asset
        .get("creators")
        .and_then(Value::as_array)
        .map(|creators| {
            creators.iter().any(|creator| {
                creator
                    .get("verified")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn is_likely_spam(
    asset: &Value,
    name: &str,
    collection: &str,
    description: Option<&str>,
    has_verified_signal: bool,
) -> bool {
    if asset
        .get("spam")
        .map(|spam| match spam {
            Value::Bool(flag) => *flag,
            Value::Object(object) => object
                .get("isSpam")
                .and_then(Value::as_bool)
                .or_else(|| object.get("is_spam").and_then(Value::as_bool))
                .unwrap_or(false),
            _ => false,
        })
        .unwrap_or(false)
    {
        return true;
    }

    let lowered = format!(
        "{} {} {}",
        name.to_ascii_lowercase(),
        collection.to_ascii_lowercase(),
        description.unwrap_or_default().to_ascii_lowercase()
    );
    let has_link_bait = lowered.contains("http://")
        || lowered.contains("https://")
        || lowered.contains("www.")
        || lowered.contains(".com")
        || lowered.contains(".xyz")
        || lowered.contains(".site")
        || lowered.contains(".click")
        || lowered.contains(".top")
        || lowered.contains(".live")
        || lowered.contains(".shop")
        || lowered.contains("discord.gg")
        || lowered.contains("t.me/")
        || lowered.contains("linktr.ee")
        || lowered.contains("bit.ly");
    let has_airdrop_language = lowered.contains("airdrop")
        || lowered.contains("claim")
        || lowered.contains("voucher")
        || lowered.contains("reward")
        || lowered.contains("bonus")
        || lowered.contains("visit")
        || lowered.contains("free mint")
        || lowered.contains("mint now")
        || lowered.contains("presale")
        || lowered.contains("whitelist")
        || lowered.contains("redeem")
        || lowered.contains("prize")
        || lowered.contains("winner")
        || lowered.contains("congrat");
    let has_wallet_lure_language = lowered.contains("connect wallet")
        || lowered.contains("connect your wallet")
        || lowered.contains("verify wallet")
        || lowered.contains("wallet verification")
        || lowered.contains("approve")
        || lowered.contains("unlock")
        || lowered.contains("drainer")
        || lowered.contains("drain")
        || lowered.contains("sweep")
        || lowered.contains("official site")
        || lowered.contains("link in bio")
        || lowered.contains("check bio");
    let suspicious_identity = looks_like_pubkey(name)
        || looks_like_pubkey(collection)
        || name.len() > 80
        || collection.len() > 80
        || matches!(
            name.to_ascii_lowercase().as_str(),
            "unknown nft" | "airdrop reward" | "claim rewards" | "reward"
        )
        || matches!(
            collection.to_ascii_lowercase().as_str(),
            "unknown collection" | "rewards" | "airdrop"
        );

    !has_verified_signal
        && (has_link_bait
            || has_airdrop_language
            || has_wallet_lure_language
            || suspicious_identity)
}

fn is_fungible_asset(asset: &Value) -> bool {
    let interface = nested_string(asset, &["interface"])
        .unwrap_or_default()
        .to_ascii_lowercase();

    if interface.contains("nonfungible") || interface.contains("nft") || interface.contains("mpl") {
        return false;
    }

    interface.starts_with("fungible")
}

fn nested_string(root: &Value, path: &[&str]) -> Option<String> {
    nested_value(root, path)
        .and_then(Value::as_str)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn nested_bool(root: &Value, path: &[&str]) -> Option<bool> {
    nested_value(root, path).and_then(Value::as_bool)
}

fn nested_value<'a>(root: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cursor = root;
    for segment in path {
        cursor = cursor.get(*segment)?;
    }
    Some(cursor)
}

fn normalize_media_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    if let Some(path) = trimmed.strip_prefix("ipfs://ipfs/") {
        return format!("https://ipfs.io/ipfs/{path}");
    }
    if let Some(path) = trimmed.strip_prefix("ipfs://") {
        return format!("https://ipfs.io/ipfs/{path}");
    }
    if let Some(path) = trimmed.strip_prefix("ar://") {
        return format!("https://arweave.net/{path}");
    }
    if let Some(path) = trimmed.strip_prefix("//") {
        return format!("https:{path}");
    }

    trimmed.to_string()
}

fn looks_like_image_url(url: &str) -> bool {
    let lowered = url.to_ascii_lowercase();
    if lowered.is_empty() {
        return false;
    }
    if lowered.starts_with("data:image/") {
        return true;
    }
    if lowered.ends_with(".mp4")
        || lowered.ends_with(".mov")
        || lowered.ends_with(".webm")
        || lowered.ends_with(".m3u8")
    {
        return false;
    }
    if lowered.starts_with("http://") || lowered.starts_with("https://") {
        return true;
    }

    lowered.contains(".png")
        || lowered.contains(".jpg")
        || lowered.contains(".jpeg")
        || lowered.contains(".gif")
        || lowered.contains(".webp")
        || lowered.contains(".svg")
        || lowered.contains(".avif")
        || lowered.contains("image")
}

fn derive_collection_name(name: &str) -> Option<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some((head, tail)) = trimmed.rsplit_once('#') {
        let suffix = tail.trim();
        if !head.trim().is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit()) {
            return Some(head.trim().to_string());
        }
    }

    if let Some((head, tail)) = trimmed.rsplit_once(' ') {
        let suffix = tail.trim_matches(|ch: char| ch == '#' || ch == '(' || ch == ')');
        if !head.trim().is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit()) {
            return Some(head.trim().to_string());
        }
    }

    None
}

fn looks_like_pubkey(value: &str) -> bool {
    let trimmed = value.trim();
    (32..=48).contains(&trimmed.len())
        && trimmed
            .chars()
            .all(|ch| "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".contains(ch))
}

// ALSO ADD this helper function to fetch metadata from JSON URI if needed:
pub async fn fetch_nft_metadata(
    json_uri: &str,
) -> Result<HashMap<String, serde_json::Value>, String> {
    let client = Client::new();

    let response = client
        .get(json_uri)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch metadata: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Metadata fetch error: {}", response.status()));
    }

    let metadata: HashMap<String, serde_json::Value> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse metadata JSON: {}", e))?;

    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_collectible_info_filters_unverified_airdrop_spam() {
        let asset = json!({
            "id": "spam-asset",
            "interface": "V1_NFT",
            "ownership": { "owner": "wallet123" },
            "content": {
                "metadata": {
                    "name": "Airdrop Reward",
                    "description": "Visit https://claim.example.com to claim now",
                    "image": "https://example.com/spam.png"
                }
            }
        });

        assert!(parse_collectible_info(&asset, "wallet123").is_none());
    }

    #[test]
    fn parse_collectible_info_keeps_verified_collectible_with_image() {
        let asset = json!({
            "id": "real-asset",
            "interface": "V1_NFT",
            "ownership": { "owner": "wallet123" },
            "grouping": [{
                "group_key": "collection",
                "group_value": "CoolCollection11111111111111111111111111111",
                "verified": true,
                "collection_metadata": {
                    "name": "Cool Collection"
                }
            }],
            "content": {
                "metadata": {
                    "name": "Cool Collection #12",
                    "description": "Legit collectible",
                    "image": "ipfs://QmExample/image.png"
                }
            }
        });

        let collectible = parse_collectible_info(&asset, "wallet123").unwrap();
        assert_eq!(collectible.mint, "real-asset");
        assert_eq!(collectible.name, "Cool Collection #12");
        assert_eq!(collectible.collection, "Cool Collection");
        assert!(collectible.verified);
        assert_eq!(
            collectible.image,
            "https://ipfs.io/ipfs/QmExample/image.png"
        );
    }

    #[test]
    fn parse_collectible_info_rejects_missing_image() {
        let asset = json!({
            "id": "no-image",
            "interface": "V1_NFT",
            "ownership": { "owner": "wallet123" },
            "content": {
                "metadata": {
                    "name": "Invisible NFT"
                }
            }
        });

        assert!(parse_collectible_info(&asset, "wallet123").is_none());
    }

    #[test]
    fn parse_collectible_info_filters_wallet_lure_with_suspicious_collection() {
        let asset = json!({
            "id": "wallet-lure",
            "interface": "V1_NFT",
            "ownership": { "owner": "wallet123" },
            "grouping": [{
                "group_key": "collection",
                "group_value": "9YwX5Xk6m2k2uZ2z7Y7x3S8e3uVhQ7yQ4Jp2Qz8bQ7M1",
                "verified": false
            }],
            "content": {
                "metadata": {
                    "name": "Verify Wallet Reward",
                    "description": "Connect wallet to redeem your prize",
                    "image": "https://example.com/reward.png"
                }
            }
        });

        assert!(parse_collectible_info(&asset, "wallet123").is_none());
    }
}
