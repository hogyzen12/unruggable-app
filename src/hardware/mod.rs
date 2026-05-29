#[cfg(target_os = "android")]
pub mod android_usb;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod ledger;
pub mod protocol;
#[cfg(not(target_os = "android"))]
pub mod serial;

pub use protocol::{AuthMode, DeviceInfo, Esp32Capability, OtpSetupData};
use protocol::{Command, Response};
use std::error::Error;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

#[derive(Debug, Clone, PartialEq)]
pub enum HardwareDeviceType {
    ESP32,
    Ledger,
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
    esp32_info: Arc<Mutex<Option<DeviceInfo>>>,
}

impl PartialEq for HardwareWallet {
    fn eq(&self, other: &Self) -> bool {
        let esp32_match = Arc::ptr_eq(&self.esp32_connection, &other.esp32_connection);
        let pubkey_match = Arc::ptr_eq(&self.public_key, &other.public_key);
        let device_type_match = Arc::ptr_eq(&self.device_type, &other.device_type);
        let capability_match = Arc::ptr_eq(&self.esp32_capability, &other.esp32_capability);
        let info_match = Arc::ptr_eq(&self.esp32_info, &other.esp32_info);

        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        let ledger_match = Arc::ptr_eq(&self.ledger_connection, &other.ledger_connection);
        #[cfg(any(target_os = "android", target_os = "ios"))]
        let ledger_match = true;

        esp32_match
            && ledger_match
            && pubkey_match
            && device_type_match
            && capability_match
            && info_match
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
            esp32_info: Arc::new(Mutex::new(None)),
        }
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

    pub async fn scan_available_devices() -> Vec<HardwareDeviceInfo> {
        let mut devices = Vec::new();

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
            if let Ok(esp32_devices) = android_usb::AndroidUsbSerial::scan_for_devices().await {
                for device in esp32_devices {
                    devices.push(HardwareDeviceInfo {
                        device_type: HardwareDeviceType::ESP32,
                        name: device.device_name,
                        connected: false,
                    });
                }
            }
        }

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

        devices
    }

    pub async fn connect(&self) -> Result<(), Box<dyn Error>> {
        if let Ok(()) = self.connect_ledger().await {
            return Ok(());
        }

        self.connect_esp32().await
    }

    pub async fn connect_esp32(&self) -> Result<(), Box<dyn Error>> {
        let mut esp32_guard = self.esp32_connection.lock().await;

        #[cfg(not(target_os = "android"))]
        {
            let connection = serial::SerialConnection::find_and_connect().await?;

            let mut capability = Esp32Capability::LegacyV0;
            let mut info_state: Option<DeviceInfo> = None;

            match connection.send_command(Command::GetInfo).await {
                Ok(Response::Info(info)) => {
                    capability = Esp32Capability::NewV1;
                    info_state = Some(info);
                }
                Ok(Response::Error(err)) if err == "Unknown command" => {}
                Ok(_) => {}
                Err(_) => {}
            }

            let response = connection.send_command(Command::GetPubkey).await?;
            match response {
                Response::Pubkey(pubkey) => {
                    if let Err(e) = bs58::decode(&pubkey).into_vec() {
                        return Err(format!("Invalid public key format: {e}").into());
                    }
                    *self.public_key.lock().await = Some(pubkey);
                    *self.device_type.lock().await = Some(HardwareDeviceType::ESP32);
                    *self.esp32_capability.lock().await = Some(capability);
                    *self.esp32_info.lock().await = info_state;
                }
                Response::Error(e) => {
                    return Err(format!("Hardware wallet error: {e}").into());
                }
                _ => {
                    return Err("Unexpected response from hardware wallet".into());
                }
            }

            *esp32_guard = Some(connection);
        }

        #[cfg(target_os = "android")]
        {
            let mut connection = android_usb::AndroidUsbSerial::new();
            connection
                .find_and_connect()
                .await
                .map_err(|e| format!("Failed to connect to hardware wallet: {e}"))?;

            let mut capability = Esp32Capability::LegacyV0;
            let mut info_state: Option<DeviceInfo> = None;

            match connection.send_command(Command::GetInfo).await {
                Ok(Response::Info(info)) => {
                    capability = Esp32Capability::NewV1;
                    info_state = Some(info);
                }
                Ok(Response::Error(err)) if err == "Unknown command" => {}
                Ok(_) => {}
                Err(_) => {}
            }

            let response = connection
                .send_command(Command::GetPubkey)
                .await
                .map_err(|e| format!("Failed to get public key: {e}"))?;

            match response {
                Response::Pubkey(pubkey) => {
                    if let Err(e) = bs58::decode(&pubkey).into_vec() {
                        return Err(format!("Invalid public key format: {e}").into());
                    }
                    *self.public_key.lock().await = Some(pubkey);
                    *self.device_type.lock().await = Some(HardwareDeviceType::ESP32);
                    *self.esp32_capability.lock().await = Some(capability);
                    *self.esp32_info.lock().await = info_state;
                }
                Response::Error(e) => {
                    return Err(format!("Hardware wallet error: {e}").into());
                }
                _ => {
                    return Err("Unexpected response from hardware wallet".into());
                }
            }

            *esp32_guard = Some(connection);
        }

        Ok(())
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
            *self.esp32_info.lock().await = None;
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
                    .map_err(|e| format!("Failed to set Ledger derivation path '{derivation_path}': {e}"))?,
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
                let esp32_guard = self.esp32_connection.lock().await;
                match esp32_guard.as_ref() {
                    Some(connection) => {
                        connection.send_command(command).await.map_err(|e| e.into())
                    }
                    None => Err("ESP32 not connected".into()),
                }
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
            Ok(Response::Error(err)) if err == "Unknown command" => {}
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

        match self.send_esp32_command(Command::GetInfo).await? {
            Response::Info(info) => {
                *self.esp32_info.lock().await = Some(info.clone());
                Ok(Some(info))
            }
            Response::Error(err) if err == "Unknown command" => {
                *self.esp32_capability.lock().await = Some(Esp32Capability::LegacyV0);
                *self.esp32_info.lock().await = None;
                Ok(None)
            }
            Response::Error(err) => Err(format!("Hardware wallet error: {err}").into()),
            _ => Err("Unexpected response from GET_INFO".into()),
        }
    }

    pub async fn is_esp32_setup_required(&self) -> Result<bool, Box<dyn Error>> {
        match self.refresh_esp32_info().await? {
            Some(info) => Ok(!info.finalized || info.auth_mode == AuthMode::Unset),
            None => Ok(false),
        }
    }

    pub async fn setup_mode_none(&self) -> Result<(), Box<dyn Error>> {
        match self.send_esp32_command(Command::SetModeNone).await? {
            Response::ModeSet(AuthMode::None) => {
                let _ = self.refresh_esp32_info().await;
                Ok(())
            }
            Response::Error(err) => Err(format!("Hardware wallet error: {err}").into()),
            _ => Err("Unexpected response while setting NONE mode".into()),
        }
    }

    pub async fn setup_mode_pin(&self, pin: &str) -> Result<(), Box<dyn Error>> {
        if !is_six_digit_code(pin) {
            return Err("Device PIN must be exactly 6 digits".into());
        }

        match self
            .send_esp32_command(Command::SetModePin(pin.to_string()))
            .await?
        {
            Response::ModeSet(AuthMode::Pin) => {
                let _ = self.refresh_esp32_info().await;
                Ok(())
            }
            Response::Error(err) => Err(format!("Hardware wallet error: {err}").into()),
            _ => Err("Unexpected response while setting PIN mode".into()),
        }
    }

    pub async fn setup_mode_otp_begin(&self) -> Result<OtpSetupData, Box<dyn Error>> {
        match self.send_esp32_command(Command::SetModeOtpBegin).await? {
            Response::OtpSetup(data) => Ok(data),
            Response::Error(err) => Err(format!("Hardware wallet error: {err}").into()),
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
                Ok(())
            }
            Response::Error(err) => Err(format!("Hardware wallet error: {err}").into()),
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
            Response::UnlockedUntil(until) => Ok(until),
            Response::Error(err) => Err(format!("Hardware wallet error: {err}").into()),
            _ => Err("Unexpected response while unlocking with PIN".into()),
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
            Response::UnlockedUntil(until) => Ok(until),
            Response::Error(err) => Err(format!("Hardware wallet error: {err}").into()),
            _ => Err("Unexpected response while unlocking with OTP".into()),
        }
    }

    pub async fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        let device_type = self.device_type.lock().await.clone();

        match device_type {
            Some(HardwareDeviceType::ESP32) => {
                let response = self
                    .send_command(Command::SignMessage(message.to_vec()))
                    .await?;
                match response {
                    Response::Signature(sig) => Ok(sig),
                    Response::Error(e) => Err(format!("Hardware wallet error: {e}").into()),
                    _ => Err("Unexpected response from hardware wallet".into()),
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

    pub async fn disconnect(&self) -> Result<(), Box<dyn Error>> {
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
        *self.esp32_info.lock().await = None;

        log::info!("🔌 Disconnected from all hardware wallets");
        Ok(())
    }
}

fn is_six_digit_code(value: &str) -> bool {
    value.len() == 6 && value.chars().all(|c| c.is_ascii_digit())
}
