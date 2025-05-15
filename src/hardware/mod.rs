// src/hardware/mod.rs
pub mod serial;
pub mod protocol;

use serial::SerialConnection;
use protocol::{Command, Response};
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Hardware wallet connection manager
#[derive(Clone)]
pub struct HardwareWallet {
    connection: Arc<Mutex<Option<SerialConnection>>>,
    public_key: Arc<Mutex<Option<String>>>,
}

// Implement PartialEq manually for HardwareWallet
// We can't derive it automatically because SerialConnection likely doesn't implement PartialEq
impl PartialEq for HardwareWallet {
    fn eq(&self, other: &Self) -> bool {
        // Since we can't compare the connections directly, we'll use pointer equality
        // This checks if both Arc's point to the same underlying data
        Arc::ptr_eq(&self.connection, &other.connection) && 
        Arc::ptr_eq(&self.public_key, &other.public_key)
    }
}

impl HardwareWallet {
    /// Create a new hardware wallet instance
    pub fn new() -> Self {
        Self {
            connection: Arc::new(Mutex::new(None)),
            public_key: Arc::new(Mutex::new(None)),
        }
    }
    
    /// Check if a hardware wallet device is present (without connecting)
    pub fn is_device_present() -> bool {
        SerialConnection::check_device_presence()
    }
    
    /// Connect to the hardware wallet
    pub async fn connect(&self) -> Result<(), Box<dyn Error>> {
        let mut conn_guard = self.connection.lock().await;
        
        // Find and connect to the device
        let connection = SerialConnection::find_and_connect().await?;
        
        // Get the public key
        let response = connection.send_command(Command::GetPubkey).await?;
        match response {
            Response::Pubkey(pubkey) => {
                // Validate that the pubkey is a valid Solana address
                if let Err(e) = bs58::decode(&pubkey).into_vec() {
                    return Err(format!("Invalid public key format: {}", e).into());
                }
                *self.public_key.lock().await = Some(pubkey);
            }
            Response::Error(e) => {
                return Err(format!("Hardware wallet error: {}", e).into());
            }
            _ => {
                return Err("Unexpected response from hardware wallet".into());
            }
        }
        
        *conn_guard = Some(connection);
        Ok(())
    }
    
    /// Disconnect from the hardware wallet
    pub async fn disconnect(&self) {
        *self.connection.lock().await = None;
        *self.public_key.lock().await = None;
    }
    
    /// Check if connected
    pub async fn is_connected(&self) -> bool {
        self.connection.lock().await.is_some()
    }
    
    /// Get the public key
    pub async fn get_public_key(&self) -> Result<String, Box<dyn Error>> {
        match &*self.public_key.lock().await {
            Some(key) => Ok(key.clone()),
            None => Err("Not connected to hardware wallet".into()),
        }
    }
    
    /// Sign a message
    pub async fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        let conn_guard = self.connection.lock().await;
        match &*conn_guard {
            Some(connection) => {
                let response = connection.send_command(Command::SignMessage(message.to_vec())).await?;
                match response {
                    Response::Signature(sig) => Ok(sig),
                    Response::Error(e) => Err(format!("Hardware wallet error: {}", e).into()),
                    _ => Err("Unexpected response from hardware wallet".into())
                }
            }
            None => Err("Not connected to hardware wallet".into()),
        }
    }
}