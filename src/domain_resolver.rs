// src/domain_resolver.rs - Unified domain resolver supporting both SNS (.sol) and ANS (.abc, .bonk, etc.)
use solana_sdk::pubkey::Pubkey;
use solana_client::nonblocking::rpc_client::RpcClient;
use std::str::FromStr;
use std::sync::Arc;
use std::collections::HashMap;
use std::sync::Mutex;
use serde::{Deserialize, Serialize};

use crate::ans_resolver::resolve_ans_domain;

// Cloudflare worker response format for SNS
#[derive(Debug, Deserialize, Serialize)]
struct CloudflareResponse {
    s: String,
    result: Option<String>,
    error: Option<String>,
}

// Minimal error type
#[derive(Debug, Clone)]
pub enum DomainError {
    InvalidDomain,
    NetworkError(String),
    InvalidPubkey,
    NotFound,
}

impl From<reqwest::Error> for DomainError {
    fn from(e: reqwest::Error) -> Self {
        Self::NetworkError(format!("{:?}", e))
    }
}

impl From<Box<dyn std::error::Error>> for DomainError {
    fn from(e: Box<dyn std::error::Error>) -> Self {
        Self::NetworkError(format!("{:?}", e))
    }
}

// Main unified domain resolver
pub struct DomainResolver {
    // SNS (Cloudflare worker)
    sns_client: reqwest::Client,
    sns_base_url: String,
    sns_cache: Arc<Mutex<HashMap<String, Pubkey>>>,
    
    // ANS (local RPC)
    rpc_client: Arc<RpcClient>,
    ans_cache: Arc<Mutex<HashMap<String, Pubkey>>>,
}

impl DomainResolver {
    pub fn new(rpc_endpoint: String) -> Self {
        println!("Creating unified domain resolver (SNS + ANS)");
        Self {
            // SNS setup
            sns_client: reqwest::Client::new(),
            sns_base_url: "https://sns-sdk-proxy.bonfida.workers.dev".to_string(),
            sns_cache: Arc::new(Mutex::new(HashMap::new())),
            
            // ANS setup
            rpc_client: Arc::new(RpcClient::new(rpc_endpoint)),
            ans_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check if input looks like a domain (SNS or ANS)
    pub fn is_domain(&self, input: &str) -> bool {
        let trimmed = input.trim().to_lowercase();
        
        // Check for known TLDs
        if trimmed.ends_with(".sol") || 
           trimmed.ends_with(".abc") ||
           trimmed.ends_with(".bonk") ||
           trimmed.ends_with(".poor") ||
           trimmed.ends_with(".superteam") {
            return true;
        }
        
        // If no dot and not a pubkey, might be SNS domain without TLD
        !trimmed.contains('.') && trimmed.len() > 0 && !self.is_solana_pubkey(&trimmed)
    }

    /// Check if input is SNS domain specifically
    pub fn is_sns_domain(&self, input: &str) -> bool {
        let trimmed = input.trim().to_lowercase();
        trimmed.ends_with(".sol") || 
        (!trimmed.contains('.') && trimmed.len() > 0 && !self.is_solana_pubkey(&trimmed))
    }

    /// Check if input is ANS domain specifically
    pub fn is_ans_domain(&self, input: &str) -> bool {
        let trimmed = input.trim().to_lowercase();
        trimmed.ends_with(".abc") ||
        trimmed.ends_with(".bonk") ||
        trimmed.ends_with(".poor") ||
        trimmed.ends_with(".superteam")
    }

    /// Check if input is valid Solana pubkey
    pub fn is_solana_pubkey(&self, input: &str) -> bool {
        let trimmed = input.trim();
        trimmed.len() >= 32 && trimmed.len() <= 44 && Pubkey::from_str(trimmed).is_ok()
    }

    /// Trim .sol suffix for SNS API call
    fn trim_sol_tld(&self, domain: &str) -> String {
        domain.strip_suffix(".sol").unwrap_or(domain).to_lowercase()
    }

    /// Resolve SNS domain using Cloudflare worker
    async fn resolve_sns_domain_async(&self, domain: &str) -> Result<Pubkey, DomainError> {
        let clean_domain = self.trim_sol_tld(domain);
        let cache_key = format!("sns:{}", clean_domain);
        
        // Check cache first
        if let Ok(cache) = self.sns_cache.lock() {
            if let Some(cached_pubkey) = cache.get(&cache_key) {
                return Ok(*cached_pubkey);
            }
        }

        let url = format!("{}/resolve/{}", self.sns_base_url, clean_domain);

        let response = self.sns_client
            .get(&url)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(DomainError::NetworkError(format!("HTTP {}", response.status())));
        }

        let cloudflare_response: CloudflareResponse = response.json().await?;

        match cloudflare_response.s.as_str() {
            "ok" => {
                if let Some(result) = cloudflare_response.result {
                    match Pubkey::from_str(&result) {
                        Ok(pubkey) => {
                            // Cache the result
                            if let Ok(mut cache) = self.sns_cache.lock() {
                                cache.insert(cache_key, pubkey);
                            }
                            Ok(pubkey)
                        }
                        Err(_) => Err(DomainError::InvalidPubkey)
                    }
                } else {
                    Err(DomainError::NotFound)
                }
            }
            "error" => {
                let error_msg = cloudflare_response.error.unwrap_or_else(|| "Unknown error".to_string());
                Err(DomainError::NetworkError(error_msg))
            }
            _ => Err(DomainError::NetworkError("Unexpected response".to_string()))
        }
    }

    /// Resolve ANS domain using local RPC
    async fn resolve_ans_domain_async(&self, domain: &str) -> Result<Pubkey, DomainError> {
        let cache_key = format!("ans:{}", domain.to_lowercase());
        
        // Check cache first
        if let Ok(cache) = self.ans_cache.lock() {
            if let Some(cached_pubkey) = cache.get(&cache_key) {
                return Ok(*cached_pubkey);
            }
        }

        // Use the ANS resolver we created
        match resolve_ans_domain(&self.rpc_client, domain).await {
            Ok(pubkey) => {
                // Cache the result
                if let Ok(mut cache) = self.ans_cache.lock() {
                    cache.insert(cache_key, pubkey);
                }
                Ok(pubkey)
            }
            Err(e) => Err(DomainError::NetworkError(format!("{:?}", e)))
        }
    }

    /// Main async resolution function - automatically routes to correct resolver
    pub async fn resolve_domain_async(&self, domain: &str) -> Result<Pubkey, DomainError> {
        if self.is_sns_domain(domain) {
            self.resolve_sns_domain_async(domain).await
        } else if self.is_ans_domain(domain) {
            self.resolve_ans_domain_async(domain).await
        } else {
            Err(DomainError::InvalidDomain)
        }
    }

    /// Main function to resolve any address input (domain or pubkey) - SYNC version for compatibility
    pub fn resolve_address(&self, input: &str) -> Result<Pubkey, String> {
        let trimmed_input = input.trim();
        
        if trimmed_input.is_empty() {
            return Err("Address cannot be empty".to_string());
        }

        // If it's already a valid pubkey, return it
        if self.is_solana_pubkey(trimmed_input) {
            return Pubkey::from_str(trimmed_input)
                .map_err(|e| format!("Invalid public key: {}", e));
        }
        
        // If it looks like a domain, resolve it
        if self.is_domain(trimmed_input) {
            let resolver = self.clone();
            let domain = trimmed_input.to_string();
            let domain_for_error = trimmed_input.to_string();
            
            // Spawn async resolution in blocking context
            let rt = tokio::runtime::Handle::current();
            match std::thread::spawn(move || {
                rt.block_on(async {
                    resolver.resolve_domain_async(&domain).await
                })
            }).join() {
                Ok(result) => match result {
                    Ok(pubkey) => Ok(pubkey),
                    Err(DomainError::NotFound) => Err(format!("Domain '{}' not found", domain_for_error)),
                    Err(e) => Err(format!("Failed to resolve domain '{}': {:?}", domain_for_error, e)),
                },
                Err(_) => Err(format!("Resolution task panicked for domain '{}'", domain_for_error)),
            }
        } else {
            Err("Input must be a valid Solana address or domain (e.g., 'domain.sol', 'domain.abc')".to_string())
        }
    }

    /// Resolve with additional details for better UX
    pub fn resolve_address_with_details(&self, input: &str) -> Result<(Pubkey, String), String> {
        let trimmed_input = input.trim();
        
        if self.is_solana_pubkey(trimmed_input) {
            let pubkey = Pubkey::from_str(trimmed_input)
                .map_err(|_| "Invalid Solana address format")?;
            Ok((pubkey, "Direct address".to_string()))
        } else if self.is_domain(trimmed_input) {
            match self.resolve_address(trimmed_input) {
                Ok(pubkey) => {
                    let domain_type = if self.is_sns_domain(trimmed_input) {
                        "SNS Domain"
                    } else {
                        "ANS Domain"
                    };
                    Ok((pubkey, format!("{}: {}", domain_type, trimmed_input.to_lowercase())))
                },
                Err(e) => Err(e),
            }
        } else {
            Err("Enter a valid Solana address or domain (.sol, .abc, .bonk, etc.)".to_string())
        }
    }

    /// Clear all caches
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.sns_cache.lock() {
            cache.clear();
        }
        if let Ok(mut cache) = self.ans_cache.lock() {
            cache.clear();
        }
    }

    /// Get total cache size (for debugging/stats)
    pub fn get_cache_size(&self) -> usize {
        let sns_size = self.sns_cache.lock().map(|c| c.len()).unwrap_or(0);
        let ans_size = self.ans_cache.lock().map(|c| c.len()).unwrap_or(0);
        sns_size + ans_size
    }
}

// Implement Clone for DomainResolver
impl Clone for DomainResolver {
    fn clone(&self) -> Self {
        Self {
            sns_client: self.sns_client.clone(),
            sns_base_url: self.sns_base_url.clone(),
            sns_cache: self.sns_cache.clone(),
            rpc_client: self.rpc_client.clone(),
            ans_cache: self.ans_cache.clone(),
        }
    }
}