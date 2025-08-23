// src/sns.rs - Cloudflare Worker-based SNS resolver (async-compatible)
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::Arc;
use std::collections::HashMap;
use std::sync::Mutex;
use serde::{Deserialize, Serialize};

// Cloudflare worker response format
#[derive(Debug, Deserialize, Serialize)]
struct CloudflareResponse {
    s: String,
    result: Option<String>,
    error: Option<String>,
}

// Minimal error type
#[derive(Debug, Clone)]
pub enum SnsError {
    InvalidDomain,
    NetworkError(String),
    InvalidPubkey,
    NotFound,
}

impl From<reqwest::Error> for SnsError {
    fn from(e: reqwest::Error) -> Self {
        Self::NetworkError(format!("{:?}", e))
    }
}

// Main SNS resolver struct using Cloudflare worker
pub struct SnsResolver {
    client: reqwest::Client,
    base_url: String,
    cache: Arc<Mutex<HashMap<String, Pubkey>>>,
}

impl SnsResolver {
    pub fn new(_rpc_endpoint: String) -> Self {
        println!("🚀 Creating SNS resolver using Cloudflare worker");
        Self {
            client: reqwest::Client::new(),
            base_url: "https://sns-sdk-proxy.bonfida.workers.dev".to_string(),
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check if input looks like an SNS domain
    pub fn is_sns_domain(&self, input: &str) -> bool {
        let trimmed = input.trim().to_lowercase();
        trimmed.ends_with(".sol") || 
        (!trimmed.contains('.') && trimmed.len() > 0 && !self.is_solana_pubkey(&trimmed))
    }

    /// Check if input is valid Solana pubkey
    pub fn is_solana_pubkey(&self, input: &str) -> bool {
        let trimmed = input.trim();
        trimmed.len() >= 32 && trimmed.len() <= 44 && Pubkey::from_str(trimmed).is_ok()
    }

    /// Trim .sol suffix for API call
    fn trim_tld(&self, domain: &str) -> String {
        domain.strip_suffix(".sol").unwrap_or(domain).to_lowercase()
    }

    /// Resolve domain using Cloudflare worker (async version)
    pub async fn resolve_domain_async(&self, domain: &str) -> Result<Pubkey, SnsError> {
        let clean_domain = self.trim_tld(domain);
        let cache_key = clean_domain.clone();
        
        // Check cache first
        if let Ok(cache) = self.cache.lock() {
            if let Some(cached_pubkey) = cache.get(&cache_key) {
                println!("💾 Found cached result for '{}'", cache_key);
                return Ok(*cached_pubkey);
            }
        }

        let url = format!("{}/resolve/{}", self.base_url, clean_domain);
        
        println!("🌐 Resolving '{}' via Cloudflare: {}", clean_domain, url);

        let response = self.client
            .get(&url)
            .send()
            .await?;

        if !response.status().is_success() {
            println!("❌ HTTP error: {}", response.status());
            return Err(SnsError::NetworkError(format!("HTTP {}", response.status())));
        }

        let cloudflare_response: CloudflareResponse = response.json().await?;
        
        println!("📡 Cloudflare response: {:?}", cloudflare_response);

        match cloudflare_response.s.as_str() {
            "ok" => {
                if let Some(result) = cloudflare_response.result {
                    match Pubkey::from_str(&result) {
                        Ok(pubkey) => {
                            println!("✅ Successfully resolved '{}' to {}", clean_domain, pubkey);
                            
                            // Cache the result
                            if let Ok(mut cache) = self.cache.lock() {
                                cache.insert(cache_key, pubkey);
                            }
                            
                            Ok(pubkey)
                        }
                        Err(e) => {
                            println!("❌ Invalid pubkey in response: {}", e);
                            Err(SnsError::InvalidPubkey)
                        }
                    }
                } else {
                    println!("❌ Domain '{}' not found", clean_domain);
                    Err(SnsError::NotFound)
                }
            }
            "error" => {
                let error_msg = cloudflare_response.error.unwrap_or_else(|| "Unknown error".to_string());
                println!("❌ Cloudflare error: {}", error_msg);
                Err(SnsError::NetworkError(error_msg))
            }
            _ => {
                println!("❌ Unexpected response status: {}", cloudflare_response.s);
                Err(SnsError::NetworkError("Unexpected response".to_string()))
            }
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
        
        // If it looks like an SNS domain, we need to use async resolution
        if self.is_sns_domain(trimmed_input) {
            // For sync compatibility, we'll spawn a task and block on it
            let resolver = self.clone();
            let domain = trimmed_input.to_string();
            let domain_for_error = trimmed_input.to_string(); // Clone for error messages
            
            // This is a workaround for sync contexts - in practice, you'd want to make everything async
            let rt = tokio::runtime::Handle::current();
            match std::thread::spawn(move || {
                rt.block_on(async {
                    resolver.resolve_domain_async(&domain).await
                })
            }).join() {
                Ok(result) => match result {
                    Ok(pubkey) => Ok(pubkey),
                    Err(SnsError::NotFound) => Err(format!("Domain '{}' not found", domain_for_error)),
                    Err(e) => Err(format!("Failed to resolve domain '{}': {:?}", domain_for_error, e)),
                },
                Err(_) => Err(format!("Resolution task panicked for domain '{}'", domain_for_error)),
            }
        } else {
            Err("Input must be a valid Solana address or SNS domain (e.g., 'domain.sol')".to_string())
        }
    }

    /// Resolve with additional details for better UX
    pub fn resolve_address_with_details(&self, input: &str) -> Result<(Pubkey, String), String> {
        let trimmed_input = input.trim();
        
        if self.is_solana_pubkey(trimmed_input) {
            let pubkey = Pubkey::from_str(trimmed_input)
                .map_err(|_| "Invalid Solana address format")?;
            Ok((pubkey, "Direct address".to_string()))
        } else if self.is_sns_domain(trimmed_input) {
            match self.resolve_address(trimmed_input) {
                Ok(pubkey) => Ok((pubkey, format!("Domain: {}", trimmed_input.to_lowercase()))),
                Err(e) => Err(e),
            }
        } else {
            Err("Enter a valid Solana address or .sol domain".to_string())
        }
    }

    /// Clear the resolution cache
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
    }

    /// Get cached domains (for debugging/stats)
    pub fn get_cache_size(&self) -> usize {
        self.cache.lock().map(|c| c.len()).unwrap_or(0)
    }
}

// Implement Clone for SnsResolver
impl Clone for SnsResolver {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            base_url: self.base_url.clone(),
            cache: self.cache.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cloudflare_resolution() {
        let resolver = SnsResolver::new("dummy".to_string());
        
        // Test known working domain
        match resolver.resolve_domain_async("bonfida").await {
            Ok(pubkey) => {
                println!("✅ bonfida -> {}", pubkey);
                // Expected result based on the documentation
                assert_eq!(pubkey.to_string(), "HKKp49qGWXd639QsuH7JiLijfVW5UtCVY4s1n2HANwEA");
            }
            Err(e) => {
                println!("❌ Error: {:?}", e);
            }
        }
    }

    #[test]
    fn test_sync_resolution() {
        let resolver = SnsResolver::new("dummy".to_string());
        
        match resolver.resolve_address("bonfida.sol") {
            Ok(pubkey) => {
                println!("✅ Sync resolution: bonfida.sol -> {}", pubkey);
            }
            Err(e) => {
                println!("❌ Sync resolution error: {}", e);
            }
        }
    }
}