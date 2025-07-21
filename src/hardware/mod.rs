// src/hardware/mod.rs
#[cfg(not(target_os = "android"))]
pub mod serial;
#[cfg(target_os = "android")]
pub mod android_usb;

pub mod protocol;

use protocol::{Command, Response};
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Hardware wallet connection manager
#[derive(Clone)]
pub struct HardwareWallet {
    #[cfg(not(target_os = "android"))]
    connection: Arc<Mutex<Option<serial::SerialConnection>>>,
    #[cfg(target_os = "android")]
    connection: Arc<Mutex<Option<android_usb::AndroidUsbSerial>>>,
    public_key: Arc<Mutex<Option<String>>>,
}

// Implement PartialEq manually for HardwareWallet
impl PartialEq for HardwareWallet {
    fn eq(&self, other: &Self) -> bool {
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
        #[cfg(not(target_os = "android"))]
        {
            serial::SerialConnection::check_device_presence()
        }
        #[cfg(target_os = "android")]
        {
            // For Android, we can't do a sync check since it's async
            // We'll return false here and let the UI handle async checking
            false
        }
    }
    
    /// Connect to the hardware wallet
    pub async fn connect(&self) -> Result<(), Box<dyn Error>> {
        let mut conn_guard = self.connection.lock().await;
        
        #[cfg(not(target_os = "android"))]
        {
            // Find and connect to the device using SerialConnection
            let connection = serial::SerialConnection::find_and_connect().await?;
            
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
        }
        
        #[cfg(target_os = "android")]
        {
            // Find and connect to the device using AndroidUsbSerial
            let mut connection = android_usb::AndroidUsbSerial::new();
            connection.find_and_connect().await
                .map_err(|e| format!("Failed to connect to hardware wallet: {}", e))?;
            
            // Get the public key
            let response = connection.send_command(Command::GetPubkey).await
                .map_err(|e| format!("Failed to get public key: {}", e))?;
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
        }
        
        Ok(())
    }
    
    /// Disconnect from the hardware wallet
    pub async fn disconnect(&self) {
        let mut conn_guard = self.connection.lock().await;
        
        #[cfg(target_os = "android")]
        {
            if let Some(mut connection) = conn_guard.take() {
                connection.disconnect().await;
            }
        }
        
        #[cfg(not(target_os = "android"))]
        {
            *conn_guard = None;
        }
        
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
                #[cfg(not(target_os = "android"))]
                {
                    let response = connection.send_command(Command::SignMessage(message.to_vec())).await?;
                    match response {
                        Response::Signature(sig) => Ok(sig),
                        Response::Error(e) => Err(format!("Hardware wallet error: {}", e).into()),
                        _ => Err("Unexpected response from hardware wallet".into())
                    }
                }
                #[cfg(target_os = "android")]
                {
                    let response = connection.send_command(Command::SignMessage(message.to_vec())).await
                        .map_err(|e| format!("Failed to send command: {}", e))?;
                    match response {
                        Response::Signature(sig) => Ok(sig),
                        Response::Error(e) => Err(format!("Hardware wallet error: {}", e).into()),
                        _ => Err("Unexpected response from hardware wallet".into())
                    }
                }
            }
            None => Err("Not connected to hardware wallet".into()),
        }
    }
}