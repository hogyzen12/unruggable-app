#![allow(dead_code)]

#[cfg(target_os = "android")]
pub mod android_usb;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod ledger;
pub mod protocol;
#[cfg(not(target_os = "android"))]
pub mod serial;

use protocol::{parse_hardware_pubkey, validate_signing_payload, Command, Response};
pub use protocol::{AuthMode, DeviceInfo, Esp32Capability, KeyState, OtpSetupData, ProtocolError};
use std::error::Error;
use std::sync::{
    atomic::{AtomicBool, AtomicU8, Ordering},
    Arc,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, Notify};

const ESP32_CONNECT_TIMEOUT: Duration = Duration::from_secs(12);
const OPERATION_IDLE: u8 = 0;
const OPERATION_WAITING_FOR_APPROVAL: u8 = 1;
const OPERATION_APPROVED: u8 = 2;
const OPERATION_CANCELED: u8 = 3;

#[derive(Debug, Clone, PartialEq)]
pub enum HardwareDeviceType {
    ESP32,
    Ledger,
}

impl std::fmt::Display for HardwareDeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HardwareDeviceType::ESP32 => write!(f, "Unruggable Hardware Wallet"),
            HardwareDeviceType::Ledger => write!(f, "Ledger"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HardwareDeviceInfo {
    pub device_type: HardwareDeviceType,
    pub name: String,
    pub connected: bool,
}

#[derive(Debug, Clone)]
pub struct HardwareDeviceScanResult {
    pub devices: Vec<HardwareDeviceInfo>,
    pub user_hint: Option<String>,
    pub diagnostic_text: Option<String>,
    pub fallback_serial_ports: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerDerivationAddress {
    pub account: u32,
    pub change: u32,
    pub path: String,
    pub pubkey: String,
}

#[derive(Clone)]
pub struct HardwareWallet {
    #[cfg(not(target_os = "android"))]
    esp32_connection: Arc<Mutex<Option<serial::SerialConnection>>>,
    #[cfg(target_os = "android")]
    esp32_connection: Arc<Mutex<Option<android_usb::AndroidUsbSerial>>>,

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    ledger_connection: Arc<Mutex<Option<ledger::LedgerConnection>>>,

    public_key: Arc<Mutex<Option<String>>>,
    device_type: Arc<Mutex<Option<HardwareDeviceType>>>,
    esp32_capability: Arc<Mutex<Option<Esp32Capability>>>,
    transaction_button_hold: Arc<AtomicBool>,
    operation_state: Arc<AtomicU8>,
    operation_cancel_notify: Arc<Notify>,
    esp32_info: Arc<Mutex<Option<DeviceInfo>>>,
    esp32_unlocked_until: Arc<Mutex<Option<u64>>>,
}

impl PartialEq for HardwareWallet {
    fn eq(&self, other: &Self) -> bool {
        let esp32_match = Arc::ptr_eq(&self.esp32_connection, &other.esp32_connection);
        let pubkey_match = Arc::ptr_eq(&self.public_key, &other.public_key);
        let device_type_match = Arc::ptr_eq(&self.device_type, &other.device_type);
        let capability_match = Arc::ptr_eq(&self.esp32_capability, &other.esp32_capability);
        let button_hold_match = Arc::ptr_eq(
            &self.transaction_button_hold,
            &other.transaction_button_hold,
        );
        let operation_state_match = Arc::ptr_eq(&self.operation_state, &other.operation_state);
        let cancel_notify_match = Arc::ptr_eq(
            &self.operation_cancel_notify,
            &other.operation_cancel_notify,
        );
        let info_match = Arc::ptr_eq(&self.esp32_info, &other.esp32_info);
        let unlock_match = Arc::ptr_eq(&self.esp32_unlocked_until, &other.esp32_unlocked_until);

        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        let ledger_match = Arc::ptr_eq(&self.ledger_connection, &other.ledger_connection);
        #[cfg(any(target_os = "android", target_os = "ios"))]
        let ledger_match = true;

        esp32_match
            && ledger_match
            && pubkey_match
            && device_type_match
            && capability_match
            && button_hold_match
            && operation_state_match
            && cancel_notify_match
            && info_match
            && unlock_match
    }
}

impl HardwareWallet {
    fn unix_time_now() -> Option<u64> {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|d| d.as_secs())
    }

    pub fn new() -> Self {
        Self {
            esp32_connection: Arc::new(Mutex::new(None)),
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            ledger_connection: Arc::new(Mutex::new(None)),
            public_key: Arc::new(Mutex::new(None)),
            device_type: Arc::new(Mutex::new(None)),
            esp32_capability: Arc::new(Mutex::new(None)),
            transaction_button_hold: Arc::new(AtomicBool::new(false)),
            operation_state: Arc::new(AtomicU8::new(OPERATION_IDLE)),
            operation_cancel_notify: Arc::new(Notify::new()),
            esp32_info: Arc::new(Mutex::new(None)),
            esp32_unlocked_until: Arc::new(Mutex::new(None)),
        }
    }

    async fn set_esp32_unlock_until(&self, until: Option<u64>) {
        *self.esp32_unlocked_until.lock().await = until;
    }

    async fn set_esp32_unlock_for_duration(&self, remaining_secs: u64) -> Option<u64> {
        let until = Self::unix_time_now()?.saturating_add(remaining_secs);
        self.set_esp32_unlock_until(Some(until)).await;
        Some(until)
    }

    async fn sync_esp32_unlock_from_info(&self, info: &DeviceInfo) {
        if let Some(remaining) = info.authoritative_unlock_remaining_secs() {
            if remaining == 0 {
                self.set_esp32_unlock_until(None).await;
            } else {
                let _ = self.set_esp32_unlock_for_duration(remaining).await;
            }
        }
    }

    async fn set_esp32_identity(
        &self,
        pubkey: Option<String>,
        capability: Esp32Capability,
        info_state: Option<DeviceInfo>,
    ) -> Result<(), Box<dyn Error>> {
        let pubkey = pubkey.as_deref().map(parse_hardware_pubkey).transpose()?;

        *self.public_key.lock().await = pubkey;
        *self.device_type.lock().await = Some(HardwareDeviceType::ESP32);
        *self.esp32_capability.lock().await = Some(capability);
        self.set_transaction_button_hold(Some(capability));
        self.reset_operation_state();
        if let Some(info) = info_state.as_ref() {
            self.sync_esp32_unlock_from_info(info).await;
        } else {
            self.set_esp32_unlock_until(None).await;
        }
        *self.esp32_info.lock().await = info_state;

        Ok(())
    }

    fn set_transaction_button_hold(&self, capability: Option<Esp32Capability>) {
        self.transaction_button_hold.store(
            capability == Some(Esp32Capability::CurrentV2),
            Ordering::Relaxed,
        );
    }

    /// CurrentV2 uses a hold-to-confirm transaction flow on its device screen.
    /// First Edition firmware uses a single press for transaction signing.
    pub fn requires_transaction_button_hold(&self) -> bool {
        self.transaction_button_hold.load(Ordering::Relaxed)
    }

    fn reset_operation_state(&self) {
        self.operation_state
            .store(OPERATION_IDLE, Ordering::Release);
    }

    /// Mark a newly launched transaction as cancelable before its signing
    /// future reaches the serial exchange.
    pub fn prepare_hardware_operation(&self) {
        if self.operation_state.load(Ordering::Acquire) != OPERATION_CANCELED {
            self.operation_state
                .store(OPERATION_WAITING_FOR_APPROVAL, Ordering::Release);
        }
    }

    fn begin_signing_operation(&self) -> Result<(), Box<dyn Error>> {
        loop {
            let state = self.operation_state.load(Ordering::Acquire);
            if state == OPERATION_CANCELED {
                return Err(
                    "Hardware operation canceled. Reconnect the hardware wallet before retrying."
                        .into(),
                );
            }
            if self
                .operation_state
                .compare_exchange(
                    state,
                    OPERATION_WAITING_FOR_APPROVAL,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                return Ok(());
            }
        }
    }

    fn mark_signing_approved(&self) -> Result<(), Box<dyn Error>> {
        match self.operation_state.compare_exchange(
            OPERATION_WAITING_FOR_APPROVAL,
            OPERATION_APPROVED,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => Ok(()),
            Err(OPERATION_CANCELED) => Err(
                "Hardware operation canceled. Reconnect the hardware wallet before retrying."
                    .into(),
            ),
            Err(_) => Err("Hardware approval state changed unexpectedly".into()),
        }
    }

    async fn wait_for_operation_cancel(&self) {
        while self.operation_state.load(Ordering::Acquire) != OPERATION_CANCELED {
            self.operation_cancel_notify.notified().await;
        }
    }

    /// Cancel a transaction that has not yet been approved on the device.
    /// Returns false once the hardware signature has already been accepted.
    pub fn cancel_current_operation(&self) -> bool {
        loop {
            let state = self.operation_state.load(Ordering::Acquire);
            if state == OPERATION_APPROVED {
                return false;
            }
            if state == OPERATION_CANCELED {
                return true;
            }
            if self
                .operation_state
                .compare_exchange(
                    state,
                    OPERATION_CANCELED,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                self.operation_cancel_notify.notify_one();
                return true;
            }
        }
    }

    pub async fn get_cached_public_key(&self) -> Option<String> {
        self.public_key.lock().await.clone()
    }

    pub async fn has_active_esp32_unlock_session(&self) -> bool {
        if self.get_device_type().await != Some(HardwareDeviceType::ESP32) {
            return false;
        }

        let board_owns_session = self
            .get_cached_esp32_info()
            .await
            .is_some_and(|info| info.session_bound);
        if board_owns_session && self.refresh_esp32_info().await.is_err() {
            self.set_esp32_unlock_until(None).await;
            return false;
        }

        let now = match Self::unix_time_now() {
            Some(now) => now,
            None => return false,
        };

        if let Some(until) = *self.esp32_unlocked_until.lock().await {
            if until >= now {
                return true;
            }
        }

        self.set_esp32_unlock_until(None).await;
        false
    }

    pub fn is_device_present() -> bool {
        Self::is_esp32_present() || Self::is_ledger_present()
    }

    pub fn is_esp32_present() -> bool {
        #[cfg(not(target_os = "android"))]
        {
            serial::SerialConnection::check_device_presence()
        }
        #[cfg(target_os = "android")]
        {
            false
        }
    }

    pub fn is_ledger_present() -> bool {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            ledger::LedgerConnection::check_device_presence()
        }
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            false
        }
    }

    pub async fn scan_available_devices() -> HardwareDeviceScanResult {
        let mut devices = Vec::new();

        #[cfg(not(target_os = "android"))]
        let (user_hint, diagnostic_text, fallback_serial_ports) = {
            let report =
                match tokio::task::spawn_blocking(serial::SerialConnection::scan_report).await {
                    Ok(report) => report,
                    Err(err) => serial::SerialScanReport {
                        hardware_wallet_present: false,
                        user_hint: Some(
                            "Hardware scanning stopped unexpectedly. Try again.".to_string(),
                        ),
                        diagnostic_text: format!("Hardware scan task failed: {err}"),
                        fallback_port_names: Vec::new(),
                    },
                };
            if report.hardware_wallet_present {
                devices.push(HardwareDeviceInfo {
                    device_type: HardwareDeviceType::ESP32,
                    name: "Unruggable Hardware Wallet".to_string(),
                    connected: false,
                });
            }
            (
                report.user_hint,
                Some(report.diagnostic_text),
                report.fallback_port_names,
            )
        };

        #[cfg(target_os = "android")]
        {
            if let Ok(esp32_devices) = android_usb::AndroidUsbSerial::scan_for_devices().await {
                for device in esp32_devices {
                    devices.push(HardwareDeviceInfo {
                        device_type: HardwareDeviceType::ESP32,
                        name: "Unruggable Hardware Wallet".to_string(),
                        connected: false,
                    });
                }
            }
        }

        #[cfg(target_os = "android")]
        let (user_hint, diagnostic_text, fallback_serial_ports) = (None, None, Vec::new());

        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            if let Ok(ledger_devices) = ledger::LedgerConnection::scan_for_devices() {
                for device in ledger_devices {
                    devices.push(HardwareDeviceInfo {
                        device_type: HardwareDeviceType::Ledger,
                        name: format!("{} {}", device.manufacturer, device.product),
                        connected: false,
                    });
                }
            }
        }

        HardwareDeviceScanResult {
            devices,
            user_hint,
            diagnostic_text,
            fallback_serial_ports,
        }
    }

    pub async fn connect(&self) -> Result<(), Box<dyn Error>> {
        if let Ok(()) = self.connect_ledger().await {
            return Ok(());
        }

        self.connect_esp32().await
    }

    #[cfg(not(target_os = "android"))]
    async fn identify_esp32_connection(
        &self,
        connection: &serial::SerialConnection,
    ) -> Result<(), Box<dyn Error>> {
        let mut capability = Esp32Capability::LegacyV0;
        let mut info_state: Option<DeviceInfo> = None;

        match connection.send_command(Command::GetInfo).await {
            Ok(Response::Info(info)) => {
                capability = info.capability();
                info_state = Some(info);
            }
            Ok(Response::Error(ProtocolError::UnknownCommand)) => {}
            Ok(_) => {}
            Err(_) => {}
        }

        let pubkey = match connection.send_command(Command::GetPubkey).await? {
            Response::Pubkey(pubkey) => Some(pubkey),
            Response::Error(err) if err.is_not_initialized() => {
                if info_state.is_none() {
                    info_state = Some(DeviceInfo::uninitialized_legacy_compatible());
                    capability = Esp32Capability::NewV1;
                }
                None
            }
            Response::Error(err) => {
                return Err(err.user_message().into());
            }
            _ => {
                return Err("Unexpected response from hardware wallet".into());
            }
        };

        self.set_esp32_identity(pubkey, capability, info_state)
            .await?;

        Ok(())
    }

    #[cfg(not(target_os = "android"))]
    async fn connect_esp32_once(&self) -> Result<serial::SerialConnection, Box<dyn Error>> {
        let connection = tokio::time::timeout(
            ESP32_CONNECT_TIMEOUT,
            serial::SerialConnection::find_and_connect(),
        )
        .await
        .map_err(|_| "Timed out while opening the hardware wallet connection")??;

        self.identify_esp32_connection(&connection).await?;

        Ok(connection)
    }

    #[cfg(not(target_os = "android"))]
    pub async fn connect_esp32_port(&self, port_name: &str) -> Result<(), Box<dyn Error>> {
        let connection = tokio::time::timeout(
            ESP32_CONNECT_TIMEOUT,
            serial::SerialConnection::connect(port_name),
        )
        .await
        .map_err(|_| format!("Timed out while opening {port_name}"))??;
        self.identify_esp32_connection(&connection).await?;
        *self.esp32_connection.lock().await = Some(connection);
        Ok(())
    }

    #[cfg(target_os = "android")]
    pub async fn connect_esp32_port(&self, _port_name: &str) -> Result<(), Box<dyn Error>> {
        Err("Explicit serial-port selection is available on desktop only".into())
    }

    pub async fn connect_esp32(&self) -> Result<(), Box<dyn Error>> {
        let mut esp32_guard = self.esp32_connection.lock().await;

        #[cfg(not(target_os = "android"))]
        {
            let mut last_err: Option<Box<dyn Error>> = None;

            for attempt in 0..2 {
                match self.connect_esp32_once().await {
                    Ok(connection) => {
                        *esp32_guard = Some(connection);
                        return Ok(());
                    }
                    Err(err) => {
                        log::warn!("ESP32 connect attempt {} failed: {}", attempt + 1, err);
                        last_err = Some(err);
                        if attempt == 0 {
                            tokio::time::sleep(Duration::from_millis(900)).await;
                        }
                    }
                }
            }

            Err(last_err.unwrap_or_else(|| "Failed to connect to hardware wallet".into()))
        }

        #[cfg(target_os = "android")]
        {
            let mut connection = android_usb::AndroidUsbSerial::new();
            tokio::time::timeout(ESP32_CONNECT_TIMEOUT, connection.find_and_connect())
                .await
                .map_err(|_| "Timed out while opening the hardware wallet connection")?
                .map_err(|e| format!("Failed to connect to hardware wallet: {e}"))?;

            let mut capability = Esp32Capability::LegacyV0;
            let mut info_state: Option<DeviceInfo> = None;

            match connection.send_command(Command::GetInfo).await {
                Ok(Response::Info(info)) => {
                    capability = info.capability();
                    info_state = Some(info);
                }
                Ok(Response::Error(ProtocolError::UnknownCommand)) => {}
                Ok(_) => {}
                Err(_) => {}
            }

            let pubkey = match connection
                .send_command(Command::GetPubkey)
                .await
                .map_err(|e| format!("Failed to get public key: {e}"))?
            {
                Response::Pubkey(pubkey) => Some(pubkey),
                Response::Error(err) if err.is_not_initialized() => {
                    if info_state.is_none() {
                        info_state = Some(DeviceInfo::uninitialized_legacy_compatible());
                        capability = Esp32Capability::NewV1;
                    }
                    None
                }
                Response::Error(err) => {
                    return Err(err.user_message().into());
                }
                _ => {
                    return Err("Unexpected response from hardware wallet".into());
                }
            };

            self.set_esp32_identity(pubkey, capability, info_state)
                .await?;

            *esp32_guard = Some(connection);
            Ok(())
        }
    }

    pub async fn connect_ledger(&self) -> Result<(), Box<dyn Error>> {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            let mut ledger_guard = self.ledger_connection.lock().await;

            let mut connection = ledger::LedgerConnection::new();
            connection
                .find_and_connect()
                .await
                .map_err(|e| format!("Failed to connect to Ledger: {e}"))?;

            let pubkey = connection
                .get_public_key()
                .map_err(|e| format!("Failed to get Ledger public key: {e}"))?;

            *self.public_key.lock().await = Some(pubkey);
            *self.device_type.lock().await = Some(HardwareDeviceType::Ledger);
            *self.esp32_capability.lock().await = None;
            self.set_transaction_button_hold(None);
            self.reset_operation_state();
            *self.esp32_info.lock().await = None;
            *self.esp32_unlocked_until.lock().await = None;
            *ledger_guard = Some(connection);

            log::info!("✅ Connected to Ledger hardware wallet");
            Ok(())
        }
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            Err("Ledger support not available on mobile platforms".into())
        }
    }

    pub async fn ledger_get_derivation_path(&self) -> Option<String> {
        if self.get_device_type().await != Some(HardwareDeviceType::Ledger) {
            return None;
        }

        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            let ledger_guard = self.ledger_connection.lock().await;
            ledger_guard
                .as_ref()
                .and_then(|connection| connection.get_derivation_path())
        }
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            None
        }
    }

    pub async fn ledger_get_derivation_indices(&self) -> Option<(u32, u32)> {
        if self.get_device_type().await != Some(HardwareDeviceType::Ledger) {
            return None;
        }

        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            let ledger_guard = self.ledger_connection.lock().await;
            ledger_guard
                .as_ref()
                .and_then(|connection| connection.get_derivation_indices())
        }
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            None
        }
    }

    pub async fn ledger_discover_derivation_paths(
        &self,
        start_account: u32,
        count: u32,
        change: u32,
    ) -> Result<Vec<LedgerDerivationAddress>, Box<dyn Error>> {
        if self.get_device_type().await != Some(HardwareDeviceType::Ledger) {
            return Err("Connected device is not Ledger".into());
        }

        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            let ledger_guard = self.ledger_connection.lock().await;
            match ledger_guard.as_ref() {
                Some(connection) => connection
                    .discover_derivation_paths(start_account, count, change)
                    .map_err(|e| e.into()),
                None => Err("Ledger not connected".into()),
            }
        }
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            Err("Ledger support not available on mobile platforms".into())
        }
    }

    pub async fn ledger_set_derivation_path(
        &self,
        account: u32,
        change: u32,
    ) -> Result<String, Box<dyn Error>> {
        if self.get_device_type().await != Some(HardwareDeviceType::Ledger) {
            return Err("Connected device is not Ledger".into());
        }

        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            let mut ledger_guard = self.ledger_connection.lock().await;
            let pubkey = match ledger_guard.as_mut() {
                Some(connection) => connection
                    .set_derivation_path(account, change)
                    .map_err(|e| format!("Failed to set Ledger derivation path: {e}"))?,
                None => return Err("Ledger not connected".into()),
            };

            *self.public_key.lock().await = Some(pubkey.clone());
            Ok(pubkey)
        }
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            Err("Ledger support not available on mobile platforms".into())
        }
    }

    pub async fn ledger_set_derivation_path_str(
        &self,
        derivation_path: &str,
    ) -> Result<String, Box<dyn Error>> {
        if self.get_device_type().await != Some(HardwareDeviceType::Ledger) {
            return Err("Connected device is not Ledger".into());
        }

        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            let mut ledger_guard = self.ledger_connection.lock().await;
            let pubkey = match ledger_guard.as_mut() {
                Some(connection) => connection
                    .set_derivation_path_str(derivation_path)
                    .map_err(|e| {
                        format!("Failed to set Ledger derivation path '{derivation_path}': {e}")
                    })?,
                None => return Err("Ledger not connected".into()),
            };

            *self.public_key.lock().await = Some(pubkey.clone());
            Ok(pubkey)
        }
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            Err("Ledger support not available on mobile platforms".into())
        }
    }

    pub async fn get_public_key(&self) -> Result<String, Box<dyn Error>> {
        match self.public_key.lock().await.as_ref() {
            Some(key) => Ok(key.clone()),
            None => Err("No hardware wallet connected".into()),
        }
    }

    pub async fn get_device_type(&self) -> Option<HardwareDeviceType> {
        self.device_type.lock().await.clone()
    }

    pub async fn get_device_name(&self) -> String {
        match self.get_device_type().await {
            Some(device_type) => device_type.to_string(),
            None => "No Device Connected".to_string(),
        }
    }

    pub async fn is_connected(&self) -> bool {
        self.public_key.lock().await.is_some()
    }

    pub async fn send_command(&self, command: Command) -> Result<Response, Box<dyn Error>> {
        let device_type = self.device_type.lock().await.clone();

        match device_type {
            Some(HardwareDeviceType::ESP32) => {
                let mut esp32_guard = self.esp32_connection.lock().await;
                let result = match esp32_guard.as_ref() {
                    Some(connection) => {
                        if self.operation_state.load(Ordering::Acquire) == OPERATION_CANCELED {
                            Err("Hardware operation canceled. Reconnect the hardware wallet before retrying."
                                .to_string())
                        } else {
                            tokio::select! {
                                biased;
                                _ = self.wait_for_operation_cancel() => {
                                    Err("Hardware operation canceled. Reconnect the hardware wallet before retrying."
                                        .to_string())
                                }
                                response = connection.send_command(command) => {
                                    response.map_err(|err| err.to_string())
                                }
                            }
                        }
                    }
                    None => Err("ESP32 not connected".to_string()),
                };
                if result.is_err() {
                    *esp32_guard = None;
                    self.set_esp32_unlock_until(None).await;
                    *self.esp32_info.lock().await = None;
                    *self.public_key.lock().await = None;
                    *self.device_type.lock().await = None;
                    *self.esp32_capability.lock().await = None;
                    self.set_transaction_button_hold(None);
                }
                result.map_err(Into::into)
            }
            Some(HardwareDeviceType::Ledger) => {
                Err("Use specific Ledger methods for Ledger operations".into())
            }
            None => Err("No hardware wallet connected".into()),
        }
    }

    async fn send_esp32_command(&self, command: Command) -> Result<Response, Box<dyn Error>> {
        match self.get_device_type().await {
            Some(HardwareDeviceType::ESP32) => self.send_command(command).await,
            Some(HardwareDeviceType::Ledger) => Err("Connected device is Ledger, not ESP32".into()),
            None => Err("No hardware wallet connected".into()),
        }
    }

    async fn sync_esp32_time_best_effort(&self) {
        let Some(unix_secs) = Self::unix_time_now() else {
            return;
        };

        match self.send_esp32_command(Command::SetTime(unix_secs)).await {
            Ok(Response::TimeSet(_)) => {}
            Ok(Response::Error(ProtocolError::UnknownCommand)) => {}
            Ok(_) => {}
            Err(_) => {}
        }
    }

    pub async fn get_esp32_capability(&self) -> Option<Esp32Capability> {
        self.esp32_capability.lock().await.clone()
    }

    pub async fn get_cached_esp32_info(&self) -> Option<DeviceInfo> {
        self.esp32_info.lock().await.clone()
    }

    pub async fn refresh_esp32_info(&self) -> Result<Option<DeviceInfo>, Box<dyn Error>> {
        if self.get_device_type().await != Some(HardwareDeviceType::ESP32) {
            return Ok(None);
        }

        let capability = self
            .get_esp32_capability()
            .await
            .unwrap_or(Esp32Capability::LegacyV0);
        if capability == Esp32Capability::LegacyV0 {
            return Ok(None);
        }

        let response = self
            .send_esp32_command(Command::GetInfo)
            .await
            .map_err(|err| err.to_string())?;
        match response {
            Response::Info(info) => {
                let capability = info.capability();
                self.sync_esp32_unlock_from_info(&info).await;
                *self.esp32_capability.lock().await = Some(capability);
                self.set_transaction_button_hold(Some(capability));
                *self.esp32_info.lock().await = Some(info.clone());
                Ok(Some(info))
            }
            Response::Error(ProtocolError::UnknownCommand) => {
                *self.esp32_capability.lock().await = Some(Esp32Capability::LegacyV0);
                self.set_transaction_button_hold(Some(Esp32Capability::LegacyV0));
                *self.esp32_info.lock().await = None;
                Ok(None)
            }
            Response::Error(err) => Err(err.user_message().into()),
            _ => Err("Unexpected response from GET_INFO".into()),
        }
    }

    pub async fn refresh_esp32_pubkey(&self) -> Result<Option<String>, Box<dyn Error>> {
        if self.get_device_type().await != Some(HardwareDeviceType::ESP32) {
            return Ok(None);
        }

        match self.send_esp32_command(Command::GetPubkey).await? {
            Response::Pubkey(pubkey) => {
                let pubkey = parse_hardware_pubkey(&pubkey)?;
                *self.public_key.lock().await = Some(pubkey.clone());
                Ok(Some(pubkey))
            }
            Response::Error(err) if err.is_not_initialized() => {
                *self.public_key.lock().await = None;
                *self.esp32_unlocked_until.lock().await = None;
                Ok(None)
            }
            Response::Error(err) => Err(err.user_message().into()),
            _ => Err("Unexpected response from GET_PUBKEY".into()),
        }
    }

    pub async fn is_esp32_setup_required(&self) -> Result<bool, Box<dyn Error>> {
        match self.refresh_esp32_info().await? {
            Some(info) => Ok(info.needs_setup()),
            None => Ok(false),
        }
    }

    pub async fn setup_mode_none(&self) -> Result<(), Box<dyn Error>> {
        if self.get_esp32_capability().await == Some(Esp32Capability::CurrentV2) {
            return Err("MLH hardware wallets require a PIN; NONE mode is not supported".into());
        }
        match self.send_esp32_command(Command::SetModeNone).await? {
            Response::ModeSet(AuthMode::None) => {
                let _ = self.refresh_esp32_info().await;
                let _ = self.refresh_esp32_pubkey().await?;
                Ok(())
            }
            Response::Error(err) => Err(err.user_message().into()),
            _ => Err("Unexpected response while setting NONE mode".into()),
        }
    }

    pub async fn setup_mode_pin(&self, pin: &str) -> Result<(), Box<dyn Error>> {
        if !is_six_digit_code(pin) {
            return Err("Device PIN must be exactly 6 digits".into());
        }

        let current_info = self.refresh_esp32_info().await?;
        if current_info.as_ref().is_some_and(|info| {
            info.is_current_v2()
                && info.auth_mode == AuthMode::Pin
                && info.finalized
                && info.key_state == KeyState::Uninitialized
        }) {
            self.unlock_pin(pin).await?;
            self.generate_and_verify_current_v2_key().await?;
            return Ok(());
        }

        match self
            .send_esp32_command(Command::SetModePin(pin.to_string()))
            .await?
        {
            Response::ModeSet(AuthMode::Pin) => {
                let info = self.refresh_esp32_info().await?;
                if info.as_ref().is_some_and(|info| {
                    info.is_current_v2() && info.key_state == KeyState::Uninitialized
                }) {
                    self.unlock_pin(pin).await?;
                    self.generate_and_verify_current_v2_key().await?;
                    Ok(())
                } else {
                    self.refresh_esp32_pubkey()
                        .await?
                        .ok_or_else(|| {
                            "Hardware wallet did not return a public key after PIN setup".into()
                        })
                        .map(|_| ())
                }
            }
            Response::Error(err) => Err(err.user_message().into()),
            _ => Err("Unexpected response while setting PIN mode".into()),
        }
    }

    async fn generate_and_verify_current_v2_key(&self) -> Result<(), Box<dyn Error>> {
        let generated = match self.send_esp32_command(Command::Generate).await? {
            Response::Pubkey(pubkey) => parse_hardware_pubkey(&pubkey)?,
            Response::Error(err) => return Err(err.user_message().into()),
            _ => return Err("Unexpected response while generating wallet key".into()),
        };
        let verified = self
            .refresh_esp32_pubkey()
            .await?
            .ok_or("Hardware wallet did not return its generated public key")?;
        if generated != verified {
            return Err("Hardware wallet public-key verification failed".into());
        }
        let refreshed = self.refresh_esp32_info().await?;
        if !refreshed.is_some_and(|info| info.key_state == KeyState::Ready) {
            return Err("Hardware wallet did not report a ready key after generation".into());
        }
        Ok(())
    }

    pub async fn setup_mode_otp_begin(&self) -> Result<OtpSetupData, Box<dyn Error>> {
        if self.get_esp32_capability().await == Some(Esp32Capability::CurrentV2) {
            return Err("MLH hardware wallets support PIN authentication only".into());
        }
        match self.send_esp32_command(Command::SetModeOtpBegin).await? {
            Response::OtpSetup(data) => Ok(data),
            Response::Error(err) => Err(err.user_message().into()),
            _ => Err("Unexpected response while starting OTP setup".into()),
        }
    }

    pub async fn setup_mode_otp_confirm(&self, code: &str) -> Result<(), Box<dyn Error>> {
        if !is_six_digit_code(code) {
            return Err("Authenticator code must be exactly 6 digits".into());
        }

        self.sync_esp32_time_best_effort().await;

        match self
            .send_esp32_command(Command::SetModeOtpConfirm(code.to_string()))
            .await?
        {
            Response::ModeSet(AuthMode::Otp) => {
                let _ = self.refresh_esp32_info().await;
                let _ = self.refresh_esp32_pubkey().await?;
                Ok(())
            }
            Response::Error(err) => Err(err.user_message().into()),
            _ => Err("Unexpected response while confirming OTP mode".into()),
        }
    }

    pub async fn unlock_pin(&self, pin: &str) -> Result<u64, Box<dyn Error>> {
        if !is_six_digit_code(pin) {
            return Err("Device PIN must be exactly 6 digits".into());
        }

        match self
            .send_esp32_command(Command::UnlockPin(pin.to_string()))
            .await?
        {
            Response::UnlockedUntil(until) => {
                if self.get_esp32_capability().await == Some(Esp32Capability::CurrentV2) {
                    let local_until = self
                        .set_esp32_unlock_for_duration(protocol::CURRENT_V2_UNLOCK_WINDOW_SECS)
                        .await
                        .ok_or("System clock is unavailable")?;
                    let _ = self.refresh_esp32_info().await;
                    Ok(local_until)
                } else {
                    self.set_esp32_unlock_until(Some(until)).await;
                    Ok(until)
                }
            }
            Response::Error(err) => {
                if matches!(err, ProtocolError::AuthFailed | ProtocolError::AuthLocked) {
                    self.set_esp32_unlock_until(None).await;
                    let _ = self.refresh_esp32_info().await;
                }
                Err(format!("Hardware wallet error: {err}").into())
            }
            _ => Err("Unexpected response while unlocking with PIN".into()),
        }
    }

    /// Keep the PIN on the MLH wallet. Only the resulting session state crosses USB.
    pub async fn unlock_on_device(&self) -> Result<u64, Box<dyn Error>> {
        let info = self
            .refresh_esp32_info()
            .await?
            .ok_or("Hardware wallet status is unavailable")?;
        if !info.supports_device_pin {
            return Err("This hardware wallet does not support on-device PIN entry".into());
        }

        self.set_esp32_unlock_until(None).await;
        match self.send_esp32_command(Command::UnlockOnDevice).await? {
            Response::UnlockedUntil(_) => {
                let local_until = self
                    .set_esp32_unlock_for_duration(protocol::CURRENT_V2_UNLOCK_WINDOW_SECS)
                    .await
                    .ok_or("System clock is unavailable")?;
                let refreshed = self.refresh_esp32_info().await?;
                if !refreshed
                    .as_ref()
                    .is_some_and(|info| info.authoritative_unlock_remaining_secs().unwrap_or(0) > 0)
                {
                    self.set_esp32_unlock_until(None).await;
                    return Err("Hardware wallet did not open an authenticated session".into());
                }
                Ok(local_until)
            }
            Response::Error(err) => Err(format!("Hardware wallet error: {err}").into()),
            _ => Err("Unexpected response while entering the PIN on the device".into()),
        }
    }

    pub async fn unlock_otp(&self, code: &str) -> Result<u64, Box<dyn Error>> {
        if !is_six_digit_code(code) {
            return Err("Authenticator code must be exactly 6 digits".into());
        }

        self.sync_esp32_time_best_effort().await;

        match self
            .send_esp32_command(Command::UnlockOtp(code.to_string()))
            .await?
        {
            Response::UnlockedUntil(until) => {
                self.set_esp32_unlock_until(Some(until)).await;
                Ok(until)
            }
            Response::Error(err) => Err(format!("Hardware wallet error: {err}").into()),
            _ => Err("Unexpected response while unlocking with OTP".into()),
        }
    }

    pub async fn show_receive_qr(&self) -> Result<(), Box<dyn Error>> {
        if self.get_esp32_capability().await != Some(Esp32Capability::CurrentV2) {
            return Err("On-device receive QR is not supported by this hardware firmware".into());
        }

        match self.send_esp32_command(Command::ShowReceiveQr).await? {
            Response::ReceiveQrShown => Ok(()),
            Response::Error(err) => Err(err.user_message().into()),
            _ => Err("Unexpected response while showing the receive QR".into()),
        }
    }

    pub async fn hide_receive_qr(&self) -> Result<(), Box<dyn Error>> {
        if self.get_esp32_capability().await != Some(Esp32Capability::CurrentV2) {
            return Ok(());
        }

        match self.send_esp32_command(Command::HideReceiveQr).await? {
            Response::HomeShown => Ok(()),
            Response::Error(err) => Err(err.user_message().into()),
            _ => Err("Unexpected response while hiding the receive QR".into()),
        }
    }

    /// Kept for protocol compatibility. The finalized MLH firmware answers
    /// `RESET_UNAVAILABLE`, which is surfaced without suggesting recovery.
    pub async fn wipe_esp32_keys(&self) -> Result<(), Box<dyn Error>> {
        if self.get_esp32_capability().await != Some(Esp32Capability::CurrentV2) {
            return Err("Key wipe is not supported by this hardware firmware".into());
        }

        match self.send_esp32_command(Command::WipeKeys).await? {
            Response::Wiped => {
                self.set_esp32_unlock_until(None).await;
                *self.public_key.lock().await = None;
                let info = self
                    .refresh_esp32_info()
                    .await?
                    .ok_or("Missing device state after key wipe")?;
                if info.key_state != KeyState::Uninitialized {
                    return Err("Hardware wallet did not report an empty key state".into());
                }
                Ok(())
            }
            Response::Error(err) => Err(err.user_message().into()),
            _ => Err("Unexpected response while wiping hardware-wallet keys".into()),
        }
    }

    pub async fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        self.begin_signing_operation()?;
        let device_type = self.device_type.lock().await.clone();

        match device_type {
            Some(HardwareDeviceType::ESP32) => {
                let capability = self
                    .get_esp32_capability()
                    .await
                    .unwrap_or(Esp32Capability::LegacyV0);
                let info = if capability == Esp32Capability::CurrentV2 {
                    self.refresh_esp32_info().await?
                } else {
                    self.get_cached_esp32_info().await
                };
                if let Some(info) = info.as_ref() {
                    if info.locked {
                        return Err(format!(
                            "Hardware wallet error: {}",
                            ProtocolError::AuthLocked
                        )
                        .into());
                    }
                    if info.key_state == KeyState::Fault {
                        return Err(format!(
                            "Hardware wallet error: {}",
                            ProtocolError::KeystoreCorrupt
                        )
                        .into());
                    }
                    if info.session_bound && !info.session_unlocked {
                        self.set_esp32_unlock_until(None).await;
                        return Err(
                            format!("Hardware wallet error: {}", ProtocolError::Locked).into()
                        );
                    }
                    if message.first() == Some(&solana_sdk_v1::message::v1::V1_PREFIX)
                        && !info.supports_transaction_v1
                    {
                        return Err(
                            "Hardware wallet error: this firmware does not support Solana v1 transactions"
                                .into(),
                        );
                    }
                    if let Err(err) = validate_signing_payload(
                        capability,
                        message.len(),
                        info.max_sign_message_bytes,
                    ) {
                        return Err(format!("Hardware wallet error: {err}").into());
                    }
                }
                let response = self
                    .send_command(Command::SignMessage(message.to_vec()))
                    .await?;
                match response {
                    Response::Signature(sig) => {
                        self.mark_signing_approved()?;
                        Ok(sig)
                    }
                    Response::Error(e) => {
                        if e == ProtocolError::Locked {
                            self.set_esp32_unlock_until(None).await;
                        }
                        Err(format!("Hardware wallet error: {e}").into())
                    }
                    _ => Err("Unexpected response from hardware wallet".into()),
                }
            }
            Some(HardwareDeviceType::Ledger) => {
                #[cfg(not(any(target_os = "android", target_os = "ios")))]
                {
                    let ledger_guard = self.ledger_connection.lock().await;
                    match ledger_guard.as_ref() {
                        Some(connection) => tokio::select! {
                            biased;
                            _ = self.wait_for_operation_cancel() => {
                                Err("Hardware operation canceled. Reconnect the hardware wallet before retrying.".into())
                            }
                            signature = connection.sign_message(message) => {
                                let signature = signature.map_err(|e| -> Box<dyn Error> { e.into() })?;
                                self.mark_signing_approved()?;
                                Ok(signature)
                            }
                        },
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

    pub async fn disconnect(&self) -> Result<(), Box<dyn Error>> {
        let supports_lock = self
            .esp32_info
            .lock()
            .await
            .as_ref()
            .is_some_and(|info| info.protocol_version >= 2);
        if supports_lock {
            let _ = tokio::time::timeout(
                Duration::from_secs(2),
                self.send_esp32_command(Command::Lock),
            )
            .await;
        }
        self.set_esp32_unlock_until(None).await;

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

        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            let mut ledger_guard = self.ledger_connection.lock().await;
            if let Some(mut connection) = ledger_guard.take() {
                connection.disconnect();
            }
        }

        *self.public_key.lock().await = None;
        *self.device_type.lock().await = None;
        *self.esp32_capability.lock().await = None;
        self.set_transaction_button_hold(None);
        self.reset_operation_state();
        *self.esp32_info.lock().await = None;
        *self.esp32_unlocked_until.lock().await = None;

        log::info!("🔌 Disconnected from all hardware wallets");
        Ok(())
    }
}

fn is_six_digit_code(value: &str) -> bool {
    value.len() == 6 && value.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn transaction_button_guidance_tracks_device_capability() {
        let wallet = HardwareWallet::new();
        assert!(!wallet.requires_transaction_button_hold());

        wallet
            .set_esp32_identity(
                Some("11111111111111111111111111111111".to_string()),
                Esp32Capability::NewV1,
                None,
            )
            .await
            .unwrap();
        assert!(!wallet.requires_transaction_button_hold());

        wallet
            .set_esp32_identity(
                Some("11111111111111111111111111111111".to_string()),
                Esp32Capability::CurrentV2,
                None,
            )
            .await
            .unwrap();
        assert!(wallet.requires_transaction_button_hold());

        wallet
            .set_esp32_identity(
                Some("11111111111111111111111111111111".to_string()),
                Esp32Capability::LegacyV0,
                None,
            )
            .await
            .unwrap();
        assert!(!wallet.requires_transaction_button_hold());

        wallet.disconnect().await.unwrap();
        assert!(!wallet.requires_transaction_button_hold());
    }

    #[tokio::test]
    async fn cancellation_wins_before_approval_and_not_after() {
        let wallet = HardwareWallet::new();
        wallet.prepare_hardware_operation();
        assert!(wallet.cancel_current_operation());
        tokio::time::timeout(
            Duration::from_millis(50),
            wallet.wait_for_operation_cancel(),
        )
        .await
        .expect("cancel notification should be observable");
        assert!(wallet.begin_signing_operation().is_err());

        wallet.reset_operation_state();
        wallet.prepare_hardware_operation();
        wallet.begin_signing_operation().unwrap();
        wallet.mark_signing_approved().unwrap();
        assert!(!wallet.cancel_current_operation());
    }
}

#[cfg(all(test, not(target_os = "android")))]
mod physical_device_tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires a connected, configured CurrentV2 hardware wallet"]
    async fn connected_current_v2_wallet_reports_a_verified_identity() {
        let wallet = HardwareWallet::new();
        wallet
            .connect_esp32()
            .await
            .expect("connect to the USB hardware wallet");

        assert_eq!(
            wallet.get_esp32_capability().await,
            Some(Esp32Capability::CurrentV2)
        );
        let info = wallet
            .refresh_esp32_info()
            .await
            .expect("read GET_INFO")
            .expect("CurrentV2 device info");
        assert!(info.is_current_v2());
        assert!(info.finalized);
        assert_eq!(info.auth_mode, AuthMode::Pin);
        assert_eq!(info.key_state, KeyState::Ready);
        assert!(info.supports_device_pin);
        assert!(info.max_sign_message_bytes > 0);
        assert!(wallet.get_public_key().await.is_ok());

        wallet.disconnect().await.expect("disconnect cleanly");
    }
}
