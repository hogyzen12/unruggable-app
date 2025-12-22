// src/hardware/ledger.rs
// Only compile this module on desktop platforms (not mobile)
#![cfg(not(any(target_os = "android", target_os = "ios")))]

use hidapi::HidApi;
use parking_lot::Mutex;
use solana_derivation_path::DerivationPath;
use solana_remote_wallet::ledger::LedgerWallet;
use solana_remote_wallet::remote_wallet::{RemoteWallet, RemoteWalletManager};
use solana_sdk::pubkey::Pubkey;
use std::{rc::Rc, sync::Arc, time::Duration};

#[derive(Debug, Clone)]
pub struct LedgerError(pub String);

impl std::fmt::Display for LedgerError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for LedgerError {}

#[derive(Debug, Clone)]
pub struct LedgerDevice {
    pub device_path: String,
    pub manufacturer: String,
    pub product: String,
}

pub struct LedgerConnection {
    pubkey: Option<Pubkey>,
}

impl LedgerConnection {
    pub fn new() -> Self {
        Self {
            pubkey: None,
        }
    }

    /// Check if Ledger devices are present (without connecting)
    pub fn check_device_presence() -> bool {
        match Self::scan_for_devices() {
            Ok(devices) => !devices.is_empty(),
            Err(_) => false,
        }
    }

    /// Scan for available Ledger devices - simplified version
    pub fn scan_for_devices() -> Result<Vec<LedgerDevice>, LedgerError> {
        // Create fresh HID context (exactly like main.rs)
        let mut hidapi = HidApi::new()
            .map_err(|e| LedgerError(format!("HID init error: {}", e)))?;

        // Refresh the USB device list (important on macOS hotplug)
        hidapi.refresh_devices()
            .map_err(|e| LedgerError(format!("HID refresh failed: {}", e)))?;

        let mut ledger_devices = Vec::new();

        // Sanity: confirm we can see a Ledger VID (0x2c97) - exactly like main.rs
        if hidapi.device_list().any(|d| d.vendor_id() == 0x2c97) {
            ledger_devices.push(LedgerDevice {
                device_path: "ledger".to_string(),
                manufacturer: "Ledger".to_string(),
                product: "Hardware Wallet".to_string(),
            });
            log::info!("🔍 Found Ledger device");
        }

        Ok(ledger_devices)
    }

    /// Connect to the first available Ledger device - exactly like main.rs connect logic
    pub async fn find_and_connect(&mut self) -> Result<(), LedgerError> {
        // NOTE: Ledger support temporarily disabled on Solana 3.x test branch
        // solana-remote-wallet 2.3.7 uses Solana 2.x types that are incompatible with Solana 3.x
        // This functionality will be re-enabled when solana-remote-wallet is updated for Solana 3.x
        Err(LedgerError(
            "Ledger support temporarily disabled on Solana 3.x test branch. Switch to main branch for ledger functionality.".to_string()
        ))
        
        /* Original implementation commented out for Solana 3.x compatibility:
        log::info!("🔄 Attempting to connect to Ledger device...");

        // 1) Fresh HID context — mirrors the CLI behavior (exactly like main.rs)
        let mut hidapi = HidApi::new()
            .map_err(|e| LedgerError(format!("HID init error: {}", e)))?;

        // 2) Refresh the USB device list (important on macOS hotplug)
        hidapi.refresh_devices()
            .map_err(|e| LedgerError(format!("HID refresh failed: {}", e)))?;

        // 3) Sanity: confirm we can see a Ledger VID (0x2c97)
        if !hidapi.device_list().any(|d| d.vendor_id() == 0x2c97) {
            return Err(LedgerError(
                "No Ledger at HID layer. Use a data USB cable, direct port, unlock device, open the Solana app, and fully quit Ledger Live.".to_string()
            ));
        }

        // 4) Create the RemoteWalletManager transport over HID
        let usb = Arc::new(Mutex::new(hidapi));
        let manager: Rc<RemoteWalletManager> = RemoteWalletManager::new(usb);

        // 5) Ask the manager to actively look for compatible wallets
        let _ = manager.try_connect_polling(&Duration::from_secs(3));

        // 6) List discovered devices; if none, the Solana app likely isn't open
        let devices = manager.list_devices();
        if devices.is_empty() {
            return Err(LedgerError(
                "Ledger visible via HID but no remote wallet found.\nEnsure Solana app shows 'Application is ready' and Ledger Live is closed.".to_string()
            ));
        }

        // 7) Use the first Ledger found
        let dev = &devices[0];

        // 8) Get a LedgerWallet handle from the manager
        let ledger = manager.get_ledger(&dev.host_device_path)
            .map_err(|e| LedgerError(format!("Ledger connection error: {}", e)))?;

        // 9) Read the public key at m/44'/501'/0'/0' (no on-device confirmation)
        let path = DerivationPath::new_bip44(Some(0), Some(0));
        let pubkey = ledger.get_pubkey(&path, false)
            .map_err(|e| LedgerError(format!("Pubkey error: {}", e)))?;

        // Store just the pubkey - keep it simple
        self.pubkey = Some(pubkey);

        log::info!("✅ Successfully connected to Ledger device");
        log::info!("📋 Public key: {}", pubkey);

        Ok(())
        */
    }

    /// Connect to a specific Ledger device - just calls find_and_connect for now
    pub async fn connect_to_device(&mut self, _device: &LedgerDevice) -> Result<(), LedgerError> {
        self.find_and_connect().await
    }

    /// Get the public key from connected Ledger
    pub fn get_public_key(&self) -> Result<String, LedgerError> {
        match &self.pubkey {
            Some(pk) => Ok(pk.to_string()),
            None => Err(LedgerError("Not connected to Ledger device".to_string())),
        }
    }

    /// Get device information
    pub fn get_device_info(&self) -> Option<&LedgerDevice> {
        None // Simplified for now
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.pubkey.is_some()
    }

    /// Disconnect from Ledger
    pub fn disconnect(&mut self) {
        self.pubkey = None;
        log::info!("🔌 Disconnected from Ledger device");
    }

    /// Sign a message with the Ledger - implementing the real signing from main.rs
    pub async fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, LedgerError> {
        if self.pubkey.is_none() {
            return Err(LedgerError("Not connected to Ledger device".to_string()));
        }

        log::info!("🔄 Attempting to sign transaction with Ledger...");

        // Create fresh HID context for signing (exactly like main.rs)
        let mut hidapi = HidApi::new()
            .map_err(|e| LedgerError(format!("HID init error: {}", e)))?;

        hidapi.refresh_devices()
            .map_err(|e| LedgerError(format!("HID refresh failed: {}", e)))?;

        // Check for Ledger presence
        if !hidapi.device_list().any(|d| d.vendor_id() == 0x2c97) {
            return Err(LedgerError(
                "No Ledger at HID layer. Unlock device, open Solana app, quit Ledger Live.".to_string()
            ));
        }

        // Create RemoteWalletManager for signing
        let usb = Arc::new(Mutex::new(hidapi));
        let manager: Rc<RemoteWalletManager> = RemoteWalletManager::new(usb);
        let _ = manager.try_connect_polling(&Duration::from_secs(3));

        let devices = manager.list_devices();
        if devices.is_empty() {
            return Err(LedgerError(
                "Ledger visible but no remote wallet. Ensure Solana app is open and ready.".to_string()
            ));
        }

        let dev = &devices[0];
        let ledger = manager.get_ledger(&dev.host_device_path)
            .map_err(|e| LedgerError(format!("Ledger connection error: {}", e)))?;

        // Sign the message using the Ledger (exactly like main.rs)
        let path = DerivationPath::new_bip44(Some(0), Some(0));
        let signature = ledger.sign_message(&path, message)
            .map_err(|e| LedgerError(format!("Ledger sign error: {}", e)))?;

        log::info!("✅ Successfully signed transaction with Ledger");

        // Return the signature as bytes
        Ok(signature.as_ref().to_vec())
    }
}