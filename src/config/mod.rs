pub mod tokens;

use serde::{Deserialize, Serialize};

/// TPU configuration for parallel transaction sending
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TpuConfig {
    /// Enable TPU sending alongside RPC
    pub enabled: bool,
    /// Yellowstone gRPC endpoint
    pub grpc_endpoint: String,
    /// Yellowstone gRPC auth token (optional)
    pub grpc_token: Option<String>,
    /// Number of leaders to fanout to (current + next N-1)
    pub fanout_count: usize,
}

impl Default for TpuConfig {
    fn default() -> Self {
        Self {
            // TPU enabled by default with hardcoded Triton configuration
            enabled: true,
            grpc_endpoint: "https://vassilio-mainnet-50da.mainnet.rpcpool.com/cc44bf77-cdc3-4bf9-8c55-4b76f9dced99".to_string(),
            grpc_token: Some("cc44bf77-cdc3-4bf9-8c55-4b76f9dced99".to_string()),
            fanout_count: 3,
        }
    }
}

impl TpuConfig {
    /// Load TPU configuration - now uses hardcoded defaults, env vars can override
    pub fn from_env() -> Self {
        let default_config = Self::default();
        
        Self {
            enabled: std::env::var("TPU_ENABLED")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(default_config.enabled),
            grpc_endpoint: std::env::var("TPU_GRPC_ENDPOINT")
                .unwrap_or(default_config.grpc_endpoint),
            grpc_token: std::env::var("TPU_GRPC_TOKEN")
                .ok()
                .or(default_config.grpc_token),
            fanout_count: std::env::var("TPU_FANOUT_COUNT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default_config.fanout_count),
        }
    }
    
    /// Check if TPU is properly configured
    pub fn is_valid(&self) -> bool {
        self.enabled && !self.grpc_endpoint.is_empty()
    }
}