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
            enabled: false,
            grpc_endpoint: String::new(),
            grpc_token: None,
            fanout_count: 3,
        }
    }
}

impl TpuConfig {
    /// Load TPU configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            enabled: std::env::var("TPU_ENABLED")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(false),
            grpc_endpoint: std::env::var("TPU_GRPC_ENDPOINT")
                .unwrap_or_default(),
            grpc_token: std::env::var("TPU_GRPC_TOKEN").ok(),
            fanout_count: std::env::var("TPU_FANOUT_COUNT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3),
        }
    }
    
    /// Check if TPU is properly configured
    pub fn is_valid(&self) -> bool {
        self.enabled && !self.grpc_endpoint.is_empty()
    }
}