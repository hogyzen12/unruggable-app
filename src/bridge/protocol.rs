use serde::{Deserialize, Serialize};

/// Messages sent from browser extension to desktop app
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "method")]
pub enum BridgeRequest {
    /// Connect request from a dApp
    Connect {
        origin: String
    },

    /// Request to sign a transaction
    SignTransaction {
        transaction: String, // Base58 encoded transaction
        origin: String
    },

    /// Request to sign a message
    SignMessage {
        message: String, // Base58 encoded message
        origin: String
    },

    /// Disconnect from a dApp
    Disconnect {
        origin: String
    },

    /// Health check / ping
    Ping,

    /// Get wallet public key (for status checks)
    GetPublicKey,
}

/// Messages sent from desktop app to browser extension
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum BridgeResponse {
    /// Connection successful
    Connected {
        public_key: String
    },

    /// Transaction signed and sent successfully
    TransactionSigned {
        signature: String, // Base58 encoded on-chain transaction signature
        signed_transaction: String, // Base58 encoded full signed transaction
    },

    /// Message signed successfully
    MessageSigned {
        signature: String // Base58 encoded signature
    },

    /// User rejected the request
    Rejected {
        reason: String
    },

    /// Error occurred
    Error {
        message: String
    },

    /// Pong response to ping
    Pong,

    /// Public key response
    PublicKey {
        public_key: String
    },
}

/// Internal state for tracking pending requests
#[derive(Debug, Clone)]
pub struct PendingRequest {
    pub id: String,
    pub origin: String,
    pub request: BridgeRequest,
}
