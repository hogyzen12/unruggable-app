// src/hardware/ledger.rs
// Only compile this module on desktop platforms (not mobile)
#![cfg(not(any(target_os = "android", target_os = "ios")))]
#![allow(dead_code, deprecated)]

use super::LedgerDerivationAddress;
use hidapi::HidApi;
use parking_lot::Mutex;
use solana_derivation_path::DerivationPath;
use solana_remote_wallet::ledger::LedgerWallet;
use solana_remote_wallet::remote_wallet::{RemoteWallet, RemoteWalletInfo, RemoteWalletManager};
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
    account: u32,
    change: u32,
    selected_derivation_path: Option<DerivationPath>,
    selected_derivation_path_str: Option<String>,
    host_device_path: Option<String>,
}

impl LedgerConnection {
    const DEFAULT_ACCOUNT: u32 = 0;
    const DEFAULT_CHANGE: u32 = 0;

    pub fn new() -> Self {
        Self {
            pubkey: None,
            account: Self::DEFAULT_ACCOUNT,
            change: Self::DEFAULT_CHANGE,
            selected_derivation_path: None,
            selected_derivation_path_str: None,
            host_device_path: None,
        }
    }

    fn bip44_path(account: u32, change: u32) -> Result<DerivationPath, LedgerError> {
        let path_str = Self::path_string(account, change);
        DerivationPath::from_absolute_path_str(&path_str)
            .map_err(|e| LedgerError(format!("Invalid derivation path {path_str}: {e}")))
    }

    fn bip44_root_path() -> Result<DerivationPath, LedgerError> {
        let path_str = Self::root_path_string();
        DerivationPath::from_absolute_path_str(path_str)
            .map_err(|e| LedgerError(format!("Invalid derivation path {path_str}: {e}")))
    }

    fn bip44_account_only_path(account: u32) -> Result<DerivationPath, LedgerError> {
        let path_str = Self::account_only_path_string(account);
        DerivationPath::from_absolute_path_str(&path_str)
            .map_err(|e| LedgerError(format!("Invalid derivation path {path_str}: {e}")))
    }

    fn root_path_string() -> &'static str {
        "m/44'/501'"
    }

    fn path_string(account: u32, change: u32) -> String {
        format!("m/44'/501'/{}'/{}'", account, change)
    }

    fn account_only_path_string(account: u32) -> String {
        format!("m/44'/501'/{}'", account)
    }

    fn legacy_path_string(account: u32) -> String {
        format!("m/501'/{}'/0/0", account)
    }

    fn legacy_path(account: u32) -> Result<DerivationPath, LedgerError> {
        DerivationPath::from_absolute_path_str(&Self::legacy_path_string(account))
            .map_err(|e| LedgerError(format!("Invalid legacy derivation path: {e}")))
    }

    fn parse_account_change_from_path(path: &str) -> Option<(u32, u32)> {
        let mut parts = path.trim().split('/');
        let m = parts.next()?;
        if m != "m" {
            return None;
        }
        let purpose = parts.next()?;
        let coin = parts.next()?;
        if purpose != "44'" || coin != "501'" {
            return None;
        }
        let account_part = parts.next()?;
        let account = account_part.trim_end_matches('\'').parse::<u32>().ok()?;
        let change = parts
            .next()
            .and_then(|p| p.trim_end_matches('\'').parse::<u32>().ok())
            .unwrap_or(0);
        Some((account, change))
    }

    fn push_unique_address(
        accounts: &mut Vec<LedgerDerivationAddress>,
        entry: LedgerDerivationAddress,
    ) {
        if !accounts
            .iter()
            .any(|existing| existing.pubkey == entry.pubkey || existing.path == entry.path)
        {
            accounts.push(entry);
        }
    }

    fn pick_device<'a>(&self, devices: &'a [RemoteWalletInfo]) -> &'a RemoteWalletInfo {
        if let Some(path) = &self.host_device_path {
            if let Some(found) = devices.iter().find(|d| d.host_device_path == *path) {
                return found;
            }
        }
        &devices[0]
    }

    fn open_ledger_wallet(&self) -> Result<(Rc<LedgerWallet>, String), LedgerError> {
        // Fresh HID context mirrors the CLI behavior and makes hotplug reliable.
        let mut hidapi =
            HidApi::new().map_err(|e| LedgerError(format!("HID init error: {}", e)))?;

        hidapi
            .refresh_devices()
            .map_err(|e| LedgerError(format!("HID refresh failed: {}", e)))?;

        if !hidapi.device_list().any(|d| d.vendor_id() == 0x2c97) {
            return Err(LedgerError(
                "No Ledger at HID layer. Unlock device, open Solana app, quit Ledger Live."
                    .to_string(),
            ));
        }

        let usb = Arc::new(Mutex::new(hidapi));
        let manager: Rc<RemoteWalletManager> = RemoteWalletManager::new(usb);
        let _ = manager.try_connect_polling(&Duration::from_secs(3));

        let devices = manager.list_devices();
        if devices.is_empty() {
            return Err(LedgerError(
                "Ledger visible via HID but no remote wallet found.\nEnsure Solana app shows 'Application is ready' and Ledger Live is closed."
                    .to_string(),
            ));
        }

        let device = self.pick_device(&devices);
        let host_device_path = device.host_device_path.clone();
        let ledger = manager
            .get_ledger(&host_device_path)
            .map_err(|e| LedgerError(format!("Ledger connection error: {}", e)))?;

        Ok((ledger, host_device_path))
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
        let mut hidapi =
            HidApi::new().map_err(|e| LedgerError(format!("HID init error: {}", e)))?;

        // Refresh the USB device list (important on macOS hotplug)
        hidapi
            .refresh_devices()
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
        log::info!("🔄 Attempting to connect to Ledger device...");
        let (ledger, host_device_path) = self.open_ledger_wallet()?;
        let path = Self::bip44_path(Self::DEFAULT_ACCOUNT, Self::DEFAULT_CHANGE)?;
        let pubkey = ledger
            .get_pubkey(&path, false)
            .map_err(|e| LedgerError(format!("Pubkey error: {}", e)))?;

        self.pubkey = Some(pubkey);
        self.account = Self::DEFAULT_ACCOUNT;
        self.change = Self::DEFAULT_CHANGE;
        self.selected_derivation_path = Some(path);
        self.selected_derivation_path_str = Some(Self::path_string(
            Self::DEFAULT_ACCOUNT,
            Self::DEFAULT_CHANGE,
        ));
        self.host_device_path = Some(host_device_path);

        log::info!("✅ Successfully connected to Ledger device");
        log::info!("📋 Public key: {}", pubkey);

        Ok(())
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

    pub fn get_derivation_path(&self) -> Option<String> {
        self.pubkey
            .as_ref()
            .and_then(|_| self.selected_derivation_path_str.clone())
    }

    pub fn get_derivation_indices(&self) -> Option<(u32, u32)> {
        self.pubkey.as_ref().map(|_| (self.account, self.change))
    }

    pub fn set_derivation_path(
        &mut self,
        account: u32,
        change: u32,
    ) -> Result<String, LedgerError> {
        if self.pubkey.is_none() {
            return Err(LedgerError("Not connected to Ledger device".to_string()));
        }

        let (ledger, host_device_path) = self.open_ledger_wallet()?;
        let path = Self::bip44_path(account, change)?;
        let pubkey = ledger
            .get_pubkey(&path, false)
            .map_err(|e| LedgerError(format!("Pubkey error: {}", e)))?;

        self.pubkey = Some(pubkey);
        self.account = account;
        self.change = change;
        self.selected_derivation_path = Some(path);
        self.selected_derivation_path_str = Some(Self::path_string(account, change));
        self.host_device_path = Some(host_device_path);

        Ok(pubkey.to_string())
    }

    pub fn set_derivation_path_str(
        &mut self,
        derivation_path: &str,
    ) -> Result<String, LedgerError> {
        if self.pubkey.is_none() {
            return Err(LedgerError("Not connected to Ledger device".to_string()));
        }

        let path = DerivationPath::from_absolute_path_str(derivation_path).map_err(|e| {
            LedgerError(format!("Invalid derivation path '{derivation_path}': {e}"))
        })?;

        let (ledger, host_device_path) = self.open_ledger_wallet()?;
        let pubkey = ledger
            .get_pubkey(&path, false)
            .map_err(|e| LedgerError(format!("Pubkey error: {}", e)))?;

        self.pubkey = Some(pubkey);
        self.selected_derivation_path = Some(path);
        self.selected_derivation_path_str = Some(derivation_path.to_string());
        if let Some((account, change)) = Self::parse_account_change_from_path(derivation_path) {
            self.account = account;
            self.change = change;
        }
        self.host_device_path = Some(host_device_path);

        Ok(pubkey.to_string())
    }

    pub fn discover_derivation_paths(
        &self,
        start_account: u32,
        count: u32,
        change: u32,
    ) -> Result<Vec<LedgerDerivationAddress>, LedgerError> {
        if self.pubkey.is_none() {
            return Err(LedgerError("Not connected to Ledger device".to_string()));
        }

        let scan_count = count.min(50);
        if scan_count == 0 {
            return Ok(Vec::new());
        }

        if scan_count != count {
            log::warn!(
                "Requested scan count {} exceeds cap; scanning {} accounts instead",
                count,
                scan_count
            );
        }

        let (ledger, _) = self.open_ledger_wallet()?;
        let mut accounts = Vec::with_capacity(scan_count as usize);

        if start_account == 0 && change == 0 {
            let root_path = Self::bip44_root_path()?;
            let root_pubkey = ledger.get_pubkey(&root_path, false).map_err(|e| {
                LedgerError(format!(
                    "Failed to derive {}: {}",
                    Self::root_path_string(),
                    e
                ))
            })?;

            Self::push_unique_address(
                &mut accounts,
                LedgerDerivationAddress {
                    account: 0,
                    change: 0,
                    path: Self::root_path_string().to_string(),
                    pubkey: root_pubkey.to_string(),
                },
            );
        }

        for offset in 0..scan_count {
            let account = start_account.saturating_add(offset);
            let path = Self::bip44_path(account, change)?;
            let pubkey = ledger.get_pubkey(&path, false).map_err(|e| {
                LedgerError(format!(
                    "Failed to derive {}: {}",
                    Self::path_string(account, change),
                    e
                ))
            })?;

            Self::push_unique_address(
                &mut accounts,
                LedgerDerivationAddress {
                    account,
                    change,
                    path: Self::path_string(account, change),
                    pubkey: pubkey.to_string(),
                },
            );

            // Also probe account-only path m/44'/501'/{account}' for compatibility with
            // wallets that expose Ledger addresses from that branch.
            if change == 0 {
                let account_only_path = Self::bip44_account_only_path(account)?;
                let account_only_pubkey =
                    ledger.get_pubkey(&account_only_path, false).map_err(|e| {
                        LedgerError(format!(
                            "Failed to derive {}: {}",
                            Self::account_only_path_string(account),
                            e
                        ))
                    })?;
                let account_only_pubkey_str = account_only_pubkey.to_string();
                Self::push_unique_address(
                    &mut accounts,
                    LedgerDerivationAddress {
                        account,
                        change: 0,
                        path: Self::account_only_path_string(account),
                        pubkey: account_only_pubkey_str,
                    },
                );

                // Some wallets historically used this legacy Solana path family.
                if let Ok(legacy_path) = Self::legacy_path(account) {
                    match ledger.get_pubkey(&legacy_path, false) {
                        Ok(legacy_pubkey) => {
                            Self::push_unique_address(
                                &mut accounts,
                                LedgerDerivationAddress {
                                    account,
                                    change: 0,
                                    path: Self::legacy_path_string(account),
                                    pubkey: legacy_pubkey.to_string(),
                                },
                            );
                        }
                        Err(e) => {
                            log::debug!(
                                "Skipping unsupported legacy path {}: {}",
                                Self::legacy_path_string(account),
                                e
                            );
                        }
                    }
                }
            }
        }

        Ok(accounts)
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
        self.account = Self::DEFAULT_ACCOUNT;
        self.change = Self::DEFAULT_CHANGE;
        self.selected_derivation_path = None;
        self.selected_derivation_path_str = None;
        self.host_device_path = None;
        log::info!("🔌 Disconnected from Ledger device");
    }

    /// Sign a message with the Ledger - implementing the real signing from main.rs
    pub async fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, LedgerError> {
        if self.pubkey.is_none() {
            return Err(LedgerError("Not connected to Ledger device".to_string()));
        }

        log::info!("🔄 Attempting to sign transaction with Ledger...");
        let (ledger, _) = self.open_ledger_wallet()?;
        let path = self
            .selected_derivation_path
            .as_ref()
            .ok_or_else(|| LedgerError("Missing active Ledger derivation path".to_string()))?;
        let signature = ledger
            .sign_message(path, message)
            .map_err(|e| LedgerError(format!("Ledger sign error: {}", e)))?;

        log::info!("✅ Successfully signed transaction with Ledger");

        // Return the signature as bytes
        Ok(signature.as_ref().to_vec())
    }
}
