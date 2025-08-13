// src/hardware/mod.rs
#[cfg(not(target_os = "android"))]
pub mod serial;
#[cfg(target_os = "android")]
pub mod android_usb;

pub mod protocol;
// Only include ledger module on desktop platforms (not mobile)
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod ledger;

use protocol::{Command, Response};
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use async_trait::async_trait;

// Add these new types for future Ledger support
#[derive(Debug, Clone, PartialEq)]
pub enum HardwareDeviceType {
    ESP32,
    Ledger,  // For future use
}

impl std::fmt::Display for HardwareDeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HardwareDeviceType::ESP32 => write!(f, "ESP32 Hardware Wallet"),
            HardwareDeviceType::Ledger => write!(f, "Ledger Hardware Wallet"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HardwareDeviceInfo {
    pub device_type: HardwareDeviceType,
    pub name: String,
    pub connected: bool,
}

/// Hardware wallet connection manager (enhanced but backward compatible)
#[derive(Clone)]
pub struct HardwareWallet {
    #[cfg(not(target_os = "android"))]
    esp32_connection: Arc<Mutex<Option<serial::SerialConnection>>>,
    #[cfg(target_os = "android")]
    esp32_connection: Arc<Mutex<Option<android_usb::AndroidUsbSerial>>>,
    
    // Only include Ledger support on desktop platforms
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    ledger_connection: Arc<Mutex<Option<ledger::LedgerConnection>>>,
    
    public_key: Arc<Mutex<Option<String>>>,
    device_type: Arc<Mutex<Option<HardwareDeviceType>>>,
}

// Implement PartialEq manually for HardwareWallet
impl PartialEq for HardwareWallet {
    fn eq(&self, other: &Self) -> bool {
        let esp32_match = Arc::ptr_eq(&self.esp32_connection, &other.esp32_connection);
        let pubkey_match = Arc::ptr_eq(&self.public_key, &other.public_key);
        
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        let ledger_match = Arc::ptr_eq(&self.ledger_connection, &other.ledger_connection);
        #[cfg(any(target_os = "android", target_os = "ios"))]
        let ledger_match = true; // Always true on mobile since there's no Ledger connection
        
        esp32_match && ledger_match && pubkey_match
    }
}

impl HardwareWallet {
    /// Create a new hardware wallet instance
    pub fn new() -> Self {
        Self {
            esp32_connection: Arc::new(Mutex::new(None)),
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            ledger_connection: Arc::new(Mutex::new(None)),
            public_key: Arc::new(Mutex::new(None)),
            device_type: Arc::new(Mutex::new(None)),
        }
    }
    
    /// Check if a hardware wallet device is present (without connecting)
    pub fn is_device_present() -> bool {
        Self::is_esp32_present() || Self::is_ledger_present()
    }

    /// Check if ESP32 devices are present
    pub fn is_esp32_present() -> bool {
        #[cfg(not(target_os = "android"))]
        {
            serial::SerialConnection::check_device_presence()
        }
        #[cfg(target_os = "android")]
        {
            // For Android, we can't do a sync check since it's async
            false
        }
    }

    /// Check if Ledger devices are present
    pub fn is_ledger_present() -> bool {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            ledger::LedgerConnection::check_device_presence()
        }
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            false // Ledger not supported on mobile
        }
    }

    /// Get detailed information about available devices
    pub async fn scan_available_devices() -> Vec<HardwareDeviceInfo> {
        let mut devices = Vec::new();

        // Check for ESP32 devices
        #[cfg(not(target_os = "android"))]
        {
            if serial::SerialConnection::check_device_presence() {
                devices.push(HardwareDeviceInfo {
                    device_type: HardwareDeviceType::ESP32,
                    name: "ESP32 Hardware Wallet".to_string(),
                    connected: false,
                });
            }
        }

        #[cfg(target_os = "android")]
        {
            match android_usb::AndroidUsbSerial::scan_for_devices().await {
                Ok(esp32_devices) => {
                    for device in esp32_devices {
                        devices.push(HardwareDeviceInfo {
                            device_type: HardwareDeviceType::ESP32,
                            name: device.device_name,
                            connected: false,
                        });
                    }
                }
                Err(_) => {}
            }
        }

        // Check for Ledger devices (desktop only)
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            match ledger::LedgerConnection::scan_for_devices() {
                Ok(ledger_devices) => {
                    for device in ledger_devices {
                        devices.push(HardwareDeviceInfo {
                            device_type: HardwareDeviceType::Ledger,
                            name: format!("{} {}", device.manufacturer, device.product),
                            connected: false,
                        });
                    }
                }
                Err(_) => {}
            }
        }

        devices
    }
    
    /// Connect to the hardware wallet (enhanced - tries Ledger first, then ESP32)
    pub async fn connect(&self) -> Result<(), Box<dyn Error>> {
        // Try Ledger first
        if let Ok(()) = self.connect_ledger().await {
            return Ok(());
        }

        // Fall back to ESP32
        self.connect_esp32().await
    }

    /// Connect specifically to an ESP32 device (enhanced)
    pub async fn connect_esp32(&self) -> Result<(), Box<dyn Error>> {
        let mut esp32_guard = self.esp32_connection.lock().await;
        
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
                    *self.device_type.lock().await = Some(HardwareDeviceType::ESP32);
                }
                Response::Error(e) => {
                    return Err(format!("Hardware wallet error: {}", e).into());
                }
                _ => {
                    return Err("Unexpected response from hardware wallet".into());
                }
            }
            
            *esp32_guard = Some(connection);
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
                    *self.device_type.lock().await = Some(HardwareDeviceType::ESP32);
                }
                Response::Error(e) => {
                    return Err(format!("Hardware wallet error: {}", e).into());
                }
                _ => {
                    return Err("Unexpected response from hardware wallet".into());
                }
            }
            
            *esp32_guard = Some(connection);
        }

        Ok(())
    }

    /// Connect specifically to a Ledger device (desktop only)
    pub async fn connect_ledger(&self) -> Result<(), Box<dyn Error>> {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            let mut ledger_guard = self.ledger_connection.lock().await;
            
            let mut connection = ledger::LedgerConnection::new();
            connection.find_and_connect().await
                .map_err(|e| format!("Failed to connect to Ledger: {}", e))?;

            let pubkey = connection.get_public_key()
                .map_err(|e| format!("Failed to get Ledger public key: {}", e))?;

            *self.public_key.lock().await = Some(pubkey);
            *self.device_type.lock().await = Some(HardwareDeviceType::Ledger);
            *ledger_guard = Some(connection);

            log::info!("✅ Connected to Ledger hardware wallet");
            Ok(())
        }
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            Err("Ledger support not available on mobile platforms".into())
        }
    }
    
    /// Get the public key from the connected device
    pub async fn get_public_key(&self) -> Result<String, Box<dyn Error>> {
        match self.public_key.lock().await.as_ref() {
            Some(key) => Ok(key.clone()),
            None => Err("No hardware wallet connected".into()),
        }
    }

    /// Get the type of connected device
    pub async fn get_device_type(&self) -> Option<HardwareDeviceType> {
        self.device_type.lock().await.clone()
    }

    /// Get a display name for the connected device
    pub async fn get_device_name(&self) -> String {
        match self.get_device_type().await {
            Some(device_type) => device_type.to_string(),
            None => "No Device Connected".to_string(),
        }
    }
    
    /// Check if currently connected
    pub async fn is_connected(&self) -> bool {
        self.public_key.lock().await.is_some()
    }

    /// Send a command to the connected device (enhanced - supports both ESP32 and Ledger)
    pub async fn send_command(&self, command: Command) -> Result<Response, Box<dyn Error>> {
        let device_type = self.device_type.lock().await.clone();
        
        match device_type {
            Some(HardwareDeviceType::ESP32) => {
                let esp32_guard = self.esp32_connection.lock().await;
                match esp32_guard.as_ref() {
                    Some(connection) => connection.send_command(command).await.map_err(|e| e.into()),
                    None => Err("ESP32 not connected".into()),
                }
            }
            Some(HardwareDeviceType::Ledger) => {
                // For Ledger, we can't use the same command protocol as ESP32
                // This method is primarily for ESP32 compatibility
                Err("Use specific Ledger methods for Ledger operations".into())
            }
            None => Err("No hardware wallet connected".into()),
        }
    }

    /// Sign a message with the connected device (enhanced - supports both devices)
    pub async fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        let device_type = self.device_type.lock().await.clone();
        
        match device_type {
            Some(HardwareDeviceType::ESP32) => {
                let response = self.send_command(Command::SignMessage(message.to_vec())).await?;
                match response {
                    Response::Signature(sig) => Ok(sig),
                    Response::Error(e) => Err(format!("Hardware wallet error: {}", e).into()),
                    _ => Err("Unexpected response from hardware wallet".into())
                }
            }
            Some(HardwareDeviceType::Ledger) => {
                #[cfg(not(any(target_os = "android", target_os = "ios")))]
                {
                    let ledger_guard = self.ledger_connection.lock().await;
                    match ledger_guard.as_ref() {
                        Some(connection) => {
                            connection.sign_message(message).await.map_err(|e| e.into())
                        }
                        None => Err("Ledger not connected".into()),
                    }
                }
                #[cfg(any(target_os = "android", target_os = "ios"))]
                {
                    Err("Ledger signing not available on mobile platforms".into())
                }
            }
            None => Err("No hardware wallet connected".into()),
        }
    }
    
    /// Disconnect from the device (enhanced - supports both devices)
    pub async fn disconnect(&self) -> Result<(), Box<dyn Error>> {
        // Disconnect ESP32
        #[cfg(not(target_os = "android"))]
        {
            let mut esp32_guard = self.esp32_connection.lock().await;
            *esp32_guard = None;
        }
        
        #[cfg(target_os = "android")]
        {
            let mut esp32_guard = self.esp32_connection.lock().await;
            if let Some(mut connection) = esp32_guard.take() {
                connection.disconnect().await;
            }
        }

        // Disconnect Ledger (desktop only)
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            let mut ledger_guard = self.ledger_connection.lock().await;
            if let Some(mut connection) = ledger_guard.take() {
                connection.disconnect();
            }
        }

        *self.public_key.lock().await = None;
        *self.device_type.lock().await = None;
        log::info!("🔌 Disconnected from all hardware wallets");
        Ok(())
    }
}