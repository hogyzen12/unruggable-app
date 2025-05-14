// src/rpc.rs
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