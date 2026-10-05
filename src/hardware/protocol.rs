#![allow(dead_code)]

use base64::Engine as _;
use solana_sdk::pubkey::Pubkey;
use std::{error::Error, fmt, str::FromStr};
use zeroize::Zeroize;

pub const HARDWARE_PIN_DIGITS: usize = 6;
pub const LEGACY_MAX_SIGN_BYTES: usize = 2_048;
pub const CURRENT_V2_MAX_SIGN_BYTES: usize = 4_096;
pub const CURRENT_V2_UNLOCK_WINDOW_SECS: u64 = 420;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    Unset,
    None,
    Pin,
    Otp,
}

impl AuthMode {
    pub fn from_wire(value: &str) -> Result<Self, Box<dyn Error>> {
        match value {
            "UNSET" => Ok(Self::Unset),
            "NONE" => Ok(Self::None),
            "PIN" => Ok(Self::Pin),
            "OTP" => Ok(Self::Otp),
            other => Err(format!("Unknown auth mode: {other}").into()),
        }
    }

    pub fn as_wire(self) -> &'static str {
        match self {
            Self::Unset => "UNSET",
            Self::None => "NONE",
            Self::Pin => "PIN",
            Self::Otp => "OTP",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    Uninitialized,
    Ready,
    Fault,
    Unknown,
}

impl KeyState {
    fn from_wire(value: &str) -> Self {
        match value {
            "UNINITIALIZED" => Self::Uninitialized,
            "READY" => Self::Ready,
            "FAULT" => Self::Fault,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Esp32Capability {
    LegacyV0,
    NewV1,
    CurrentV2,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub version: String,
    pub protocol_version: u16,
    pub build_id: Option<String>,
    pub board: Option<String>,
    pub device_mac: Option<String>,
    pub ble_security: Option<String>,
    pub session_bound: bool,
    pub supports_device_pin: bool,
    pub session_unlocked: bool,
    pub unlock_remaining_secs: Option<u64>,
    pub auth_mode: AuthMode,
    pub finalized: bool,
    pub locked: bool,
    pub retries_left: u8,
    pub pin_attempt_limit: Option<u8>,
    pub key_state: KeyState,
    pub pqc_state: KeyState,
    pub pin_backend: Option<String>,
    pub solana_backend: Option<String>,
    pub active_transport: Option<String>,
    pub supports_transaction_v1: bool,
    pub max_sign_message_bytes: usize,
    pub reset_supported: bool,
    pub reset_policy: Option<String>,
    pub sign_review: Option<String>,
    pub review_timeout_secs: Option<u64>,
    has_current_v2_fields: bool,
}

impl DeviceInfo {
    pub fn capability(&self) -> Esp32Capability {
        if self.has_current_v2_fields {
            Esp32Capability::CurrentV2
        } else {
            Esp32Capability::NewV1
        }
    }

    pub fn is_current_v2(&self) -> bool {
        self.has_current_v2_fields
    }

    pub fn needs_setup(&self) -> bool {
        !self.finalized
            || self.auth_mode == AuthMode::Unset
            || (self.is_current_v2() && self.key_state == KeyState::Uninitialized)
    }

    pub fn authoritative_unlock_remaining_secs(&self) -> Option<u64> {
        if self.session_bound && self.session_unlocked {
            Some(
                self.unlock_remaining_secs
                    .unwrap_or_default()
                    .min(CURRENT_V2_UNLOCK_WINDOW_SECS),
            )
        } else if self.session_bound {
            Some(0)
        } else {
            None
        }
    }

    pub fn uninitialized_legacy_compatible() -> Self {
        Self {
            version: "unknown".to_string(),
            protocol_version: 1,
            build_id: None,
            board: None,
            device_mac: None,
            ble_security: None,
            session_bound: false,
            supports_device_pin: false,
            session_unlocked: false,
            unlock_remaining_secs: None,
            auth_mode: AuthMode::Unset,
            finalized: false,
            locked: false,
            retries_left: 0,
            pin_attempt_limit: None,
            key_state: KeyState::Unknown,
            pqc_state: KeyState::Unknown,
            pin_backend: None,
            solana_backend: None,
            active_transport: None,
            supports_transaction_v1: false,
            max_sign_message_bytes: LEGACY_MAX_SIGN_BYTES,
            reset_supported: true,
            reset_policy: None,
            sign_review: None,
            review_timeout_secs: None,
            has_current_v2_fields: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtpSetupData {
    pub secret: String,
    pub uri: String,
    pub algo: String,
    pub digits: u8,
    pub period: u32,
}

#[derive(Debug)]
pub enum Command {
    Ping,
    Lock,
    GetInfo,
    SetTime(u64),
    GetPubkey,
    Generate,
    ShowReceiveQr,
    HideReceiveQr,
    SetModeNone,
    SetModePin(String),
    SetModeOtpBegin,
    SetModeOtpConfirm(String),
    UnlockPin(String),
    UnlockOtp(String),
    UnlockOnDevice,
    SignMessage(Vec<u8>),
    WipeKeys,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    WalletNotInitialized,
    PinRequired,
    ModeFinal,
    BadPinFormat,
    BadOtpFormat,
    AuthFailed,
    OtpBadCode,
    AuthLocked,
    Locked,
    AuthModeMismatch,
    ModeUnset,
    ConfirmTimeout,
    ButtonTimeout,
    UserRejected,
    WalletAlreadyInitialized,
    KeystoreCorrupt,
    PayloadTooLarge,
    ResetUnavailable,
    TimeNotSet,
    Unsupported,
    UnknownCommand,
    Busy,
    Other(String),
}

impl ProtocolError {
    pub fn from_wire(value: &str) -> Self {
        match value.trim() {
            "WALLET_NOT_INITIALIZED" => Self::WalletNotInitialized,
            "PIN_REQUIRED" => Self::PinRequired,
            "MODE_FINAL" => Self::ModeFinal,
            "BAD_PIN_FORMAT" => Self::BadPinFormat,
            "BAD_OTP_FORMAT" => Self::BadOtpFormat,
            "AUTH_FAILED" => Self::AuthFailed,
            "OTP_BAD_CODE" => Self::OtpBadCode,
            "AUTH_LOCKED" => Self::AuthLocked,
            "LOCKED" => Self::Locked,
            "AUTH_MODE_MISMATCH" => Self::AuthModeMismatch,
            "MODE_UNSET" => Self::ModeUnset,
            "CONFIRM_TIMEOUT" => Self::ConfirmTimeout,
            "BUTTON_TIMEOUT" => Self::ButtonTimeout,
            "USER_REJECTED" => Self::UserRejected,
            "WALLET_ALREADY_INITIALIZED" => Self::WalletAlreadyInitialized,
            "KEYSTORE_CORRUPT" => Self::KeystoreCorrupt,
            "PAYLOAD_TOO_LARGE" => Self::PayloadTooLarge,
            "RESET_UNAVAILABLE" => Self::ResetUnavailable,
            "TIME_NOT_SET" => Self::TimeNotSet,
            "UNSUPPORTED" => Self::Unsupported,
            "Unknown command" | "UNKNOWN_COMMAND" => Self::UnknownCommand,
            "BUSY" => Self::Busy,
            other => Self::Other(other.to_string()),
        }
    }

    pub fn wire_code(&self) -> &str {
        match self {
            Self::WalletNotInitialized => "WALLET_NOT_INITIALIZED",
            Self::PinRequired => "PIN_REQUIRED",
            Self::ModeFinal => "MODE_FINAL",
            Self::BadPinFormat => "BAD_PIN_FORMAT",
            Self::BadOtpFormat => "BAD_OTP_FORMAT",
            Self::AuthFailed => "AUTH_FAILED",
            Self::OtpBadCode => "OTP_BAD_CODE",
            Self::AuthLocked => "AUTH_LOCKED",
            Self::Locked => "LOCKED",
            Self::AuthModeMismatch => "AUTH_MODE_MISMATCH",
            Self::ModeUnset => "MODE_UNSET",
            Self::ConfirmTimeout => "CONFIRM_TIMEOUT",
            Self::ButtonTimeout => "BUTTON_TIMEOUT",
            Self::UserRejected => "USER_REJECTED",
            Self::WalletAlreadyInitialized => "WALLET_ALREADY_INITIALIZED",
            Self::KeystoreCorrupt => "KEYSTORE_CORRUPT",
            Self::PayloadTooLarge => "PAYLOAD_TOO_LARGE",
            Self::ResetUnavailable => "RESET_UNAVAILABLE",
            Self::TimeNotSet => "TIME_NOT_SET",
            Self::Unsupported => "UNSUPPORTED",
            Self::UnknownCommand => "Unknown command",
            Self::Busy => "BUSY",
            Self::Other(message) => message,
        }
    }

    pub fn user_message(&self) -> String {
        match self {
            Self::ModeUnset | Self::PinRequired => {
                "Create a 6-digit PIN on this hardware wallet before using it.".to_string()
            }
            Self::WalletNotInitialized => {
                "Create a wallet key on this hardware wallet before using it.".to_string()
            }
            Self::ModeFinal => "This hardware wallet already has a PIN.".to_string(),
            Self::BadPinFormat | Self::BadOtpFormat => {
                "The code must contain exactly 6 digits.".to_string()
            }
            Self::AuthFailed | Self::OtpBadCode => {
                "Incorrect code. Check the attempts remaining and try again carefully.".to_string()
            }
            Self::AuthLocked => {
                "Too many incorrect PIN attempts permanently locked this hardware wallet. There is no reset or recovery procedure.".to_string()
            }
            Self::Locked => "Unlock the hardware wallet before continuing.".to_string(),
            Self::AuthModeMismatch => {
                "The hardware wallet authentication mode changed. Refresh its status and retry."
                    .to_string()
            }
            Self::ConfirmTimeout | Self::ButtonTimeout => {
                "Confirmation timed out on the hardware wallet. You can retry safely.".to_string()
            }
            Self::UserRejected => {
                "Request rejected on the hardware wallet. Nothing was signed.".to_string()
            }
            Self::WalletAlreadyInitialized => {
                "This hardware wallet already has a wallet key.".to_string()
            }
            Self::KeystoreCorrupt => {
                "The hardware wallet secure elements reported a fault. Disconnect it and contact support with diagnostics.".to_string()
            }
            Self::PayloadTooLarge => {
                "This signing request is too large for the hardware wallet.".to_string()
            }
            Self::ResetUnavailable => {
                "This hardware wallet does not support reset, key wipe, or PIN recovery."
                    .to_string()
            }
            Self::Busy => {
                "Finish or cancel the action currently shown on the hardware wallet, then retry."
                    .to_string()
            }
            Self::TimeNotSet => "The hardware wallet time is not set yet.".to_string(),
            Self::Unsupported | Self::UnknownCommand => {
                "This firmware does not support the required operation.".to_string()
            }
            Self::Other(message) => format!("Hardware wallet error: {message}"),
        }
    }

    pub fn is_not_initialized(&self) -> bool {
        matches!(self, Self::WalletNotInitialized | Self::ModeUnset)
    }
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire_code())
    }
}

impl Error for ProtocolError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Response {
    Pong,
    Locked,
    Info(DeviceInfo),
    Pubkey(String),
    Signature(Vec<u8>),
    ReceiveQrShown,
    HomeShown,
    ModeSet(AuthMode),
    OtpSetup(OtpSetupData),
    TimeSet(u64),
    UnlockedUntil(u64),
    Wiped,
    Error(ProtocolError),
}

pub(crate) fn clear_sensitive_string(value: &mut String) {
    value.zeroize();
}

pub fn format_esp32_command(cmd: Command) -> Vec<u8> {
    match cmd {
        Command::Ping => b"PING\n".to_vec(),
        Command::Lock => b"LOCK\n".to_vec(),
        Command::GetInfo => b"GET_INFO\n".to_vec(),
        Command::SetTime(unix_secs) => format!("SET_TIME:{unix_secs}\n").into_bytes(),
        Command::GetPubkey => b"GET_PUBKEY\n".to_vec(),
        Command::Generate => b"GENERATE\n".to_vec(),
        Command::ShowReceiveQr => b"SHOW_RECEIVE_QR\n".to_vec(),
        Command::HideReceiveQr => b"HIDE_RECEIVE_QR\n".to_vec(),
        Command::SetModeNone => b"SET_MODE:NONE\n".to_vec(),
        Command::SetModePin(mut pin) => {
            let bytes = format!("SET_MODE:PIN:{pin}\n").into_bytes();
            pin.zeroize();
            bytes
        }
        Command::SetModeOtpBegin => b"SET_MODE:OTP_BEGIN\n".to_vec(),
        Command::SetModeOtpConfirm(mut code) => {
            let bytes = format!("SET_MODE:OTP_CONFIRM:{code}\n").into_bytes();
            code.zeroize();
            bytes
        }
        Command::UnlockPin(mut pin) => {
            let bytes = format!("UNLOCK:PIN:{pin}\n").into_bytes();
            pin.zeroize();
            bytes
        }
        Command::UnlockOtp(mut code) => {
            let bytes = format!("UNLOCK:OTP:{code}\n").into_bytes();
            code.zeroize();
            bytes
        }
        Command::UnlockOnDevice => b"UNLOCK:DEVICE\n".to_vec(),
        Command::SignMessage(mut data) => {
            let mut encoded = base64::engine::general_purpose::STANDARD.encode(&data);
            data.zeroize();
            let bytes = format!("SIGN:{encoded}\n").into_bytes();
            encoded.zeroize();
            bytes
        }
        Command::WipeKeys => b"WIPE_KEYS\n".to_vec(),
    }
}

fn parse_bool_flag(value: &str) -> Result<bool, Box<dyn Error>> {
    match value {
        "0" | "false" | "FALSE" => Ok(false),
        "1" | "true" | "TRUE" => Ok(true),
        other => Err(format!("Invalid bool flag: {other}").into()),
    }
}

fn strip_prefix_ascii_case<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    let prefix_len = prefix.len();
    let head = value.as_bytes().get(..prefix_len)?;
    if head.eq_ignore_ascii_case(prefix.as_bytes()) {
        Some(&value[prefix_len..])
    } else {
        None
    }
}

fn parse_info_response(line: &str) -> Result<DeviceInfo, Box<dyn Error>> {
    let payload = strip_prefix_ascii_case(line, "INFO;").ok_or("Missing INFO prefix")?;
    let mut info = DeviceInfo::uninitialized_legacy_compatible();
    let mut saw_board = false;
    let mut saw_key_state = false;
    let mut saw_pqc_state = false;

    for part in payload.split(';') {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        match key {
            "VER" => info.version = value.to_string(),
            "PROTOCOL" => info.protocol_version = value.parse()?,
            "BUILD" => info.build_id = Some(value.to_string()),
            "BOARD" => {
                saw_board = true;
                info.board = Some(value.to_string());
            }
            "DEVICE_MAC" => info.device_mac = Some(value.to_string()),
            "BLE_SECURITY" => info.ble_security = Some(value.to_string()),
            "SESSION_BOUND" => info.session_bound = parse_bool_flag(value)?,
            "LOCAL_PIN" => info.supports_device_pin = parse_bool_flag(value)?,
            "SESSION_UNLOCKED" => info.session_unlocked = parse_bool_flag(value)?,
            "UNLOCK_REMAINING_SECS" => info.unlock_remaining_secs = Some(value.parse()?),
            "AUTH_MODE" => info.auth_mode = AuthMode::from_wire(value)?,
            "FINALIZED" => info.finalized = parse_bool_flag(value)?,
            "LOCKED" => info.locked = parse_bool_flag(value)?,
            "RETRIES_LEFT" => info.retries_left = value.parse()?,
            "PIN_ATTEMPT_LIMIT" => info.pin_attempt_limit = Some(value.parse()?),
            "KEY_STATE" => {
                saw_key_state = true;
                info.key_state = KeyState::from_wire(value);
            }
            "PQC_STATE" => {
                saw_pqc_state = true;
                info.pqc_state = KeyState::from_wire(value);
            }
            "PIN_BACKEND" => info.pin_backend = Some(value.to_string()),
            "SOLANA_BACKEND" => info.solana_backend = Some(value.to_string()),
            "ACTIVE_TRANSPORT" => info.active_transport = Some(value.to_string()),
            "TX_V1" => info.supports_transaction_v1 = parse_bool_flag(value)?,
            "MAX_SIGN_BYTES" => {
                info.max_sign_message_bytes =
                    value.parse::<usize>()?.clamp(1, CURRENT_V2_MAX_SIGN_BYTES)
            }
            "RESET_SUPPORTED" => info.reset_supported = parse_bool_flag(value)?,
            "RESET_POLICY" => info.reset_policy = Some(value.to_string()),
            "SIGN_REVIEW" => info.sign_review = Some(value.to_string()),
            "REVIEW_TIMEOUT_SECS" => info.review_timeout_secs = Some(value.parse()?),
            _ => {}
        }
    }

    info.has_current_v2_fields = saw_board && saw_key_state && saw_pqc_state;
    if info.session_bound && !info.session_unlocked {
        info.unlock_remaining_secs = Some(0);
    }
    Ok(info)
}

fn parse_mode_set(line: &str) -> Result<AuthMode, Box<dyn Error>> {
    let mode = strip_prefix_ascii_case(line, "MODE_SET:").ok_or("Missing MODE_SET prefix")?;
    AuthMode::from_wire(mode)
}

fn parse_otp_setup(line: &str) -> Result<OtpSetupData, Box<dyn Error>> {
    let payload =
        strip_prefix_ascii_case(line, "OTP_SECRET:").ok_or("Missing OTP_SECRET prefix")?;
    let mut parts = payload.split(';');
    let secret = parts.next().ok_or("Missing OTP secret")?.trim().to_string();
    let mut uri = None;
    let mut algo = "SHA1".to_string();
    let mut digits = 6;
    let mut period = 30;

    for part in parts {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        match key {
            "URI" => uri = Some(value.to_string()),
            "ALGO" => algo = value.to_string(),
            "DIGITS" => digits = value.parse()?,
            "PERIOD" => period = value.parse()?,
            _ => {}
        }
    }

    Ok(OtpSetupData {
        secret,
        uri: uri.ok_or("Missing OTP URI")?,
        algo,
        digits,
        period,
    })
}

pub fn parse_esp32_response_line(line: &str) -> Result<Response, Box<dyn Error>> {
    let response = line.trim_matches(|ch: char| ch.is_whitespace() || ch == '\0');
    if response.is_empty() {
        return Err("Empty response".into());
    }
    if let Some(error) = strip_prefix_ascii_case(response, "ERROR:") {
        return Ok(Response::Error(ProtocolError::from_wire(error)));
    }
    if strip_prefix_ascii_case(response, "INFO;").is_some() {
        return Ok(Response::Info(parse_info_response(response)?));
    }
    if response.eq_ignore_ascii_case("PONG") {
        return Ok(Response::Pong);
    }
    if response.eq_ignore_ascii_case("LOCKED") {
        return Ok(Response::Locked);
    }
    if let Some(pubkey) = strip_prefix_ascii_case(response, "PUBKEY:") {
        return Ok(Response::Pubkey(pubkey.to_string()));
    }
    if let Some(sig_b64) = strip_prefix_ascii_case(response, "SIGNATURE:") {
        return Ok(Response::Signature(
            base64::engine::general_purpose::STANDARD.decode(sig_b64)?,
        ));
    }
    if response.eq_ignore_ascii_case("RECEIVE_QR_SHOWN") {
        return Ok(Response::ReceiveQrShown);
    }
    if response.eq_ignore_ascii_case("HOME_SHOWN") {
        return Ok(Response::HomeShown);
    }
    if strip_prefix_ascii_case(response, "MODE_SET:").is_some() {
        return Ok(Response::ModeSet(parse_mode_set(response)?));
    }
    if response.eq_ignore_ascii_case("OTP_CONFIRMED") {
        return Ok(Response::ModeSet(AuthMode::Otp));
    }
    if strip_prefix_ascii_case(response, "OTP_SECRET:").is_some() {
        return Ok(Response::OtpSetup(parse_otp_setup(response)?));
    }
    if let Some(unix_str) = strip_prefix_ascii_case(response, "TIME_SET:") {
        return Ok(Response::TimeSet(unix_str.parse()?));
    }
    if let Some(until_str) = strip_prefix_ascii_case(response, "UNLOCKED_UNTIL:")
        .or_else(|| strip_prefix_ascii_case(response, "NLOCKED_UNTIL:"))
    {
        return Ok(Response::UnlockedUntil(until_str.parse()?));
    }
    if response.eq_ignore_ascii_case("WIPED") {
        return Ok(Response::Wiped);
    }
    Err(format!("Unknown response format: {response}").into())
}

pub fn parse_esp32_response(data: &[u8]) -> Result<Response, Box<dyn Error>> {
    let response = String::from_utf8_lossy(data);
    let mut saw_non_empty = false;
    for line in response.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        saw_non_empty = true;
        if let Ok(parsed) = parse_esp32_response_line(line) {
            return Ok(parsed);
        }
    }
    if !saw_non_empty {
        return Err("Empty response".into());
    }
    Err(format!("Unknown response format: {}", response.trim()).into())
}

pub fn parse_hardware_pubkey(value: &str) -> Result<String, Box<dyn Error>> {
    Pubkey::from_str(value.trim())
        .map(|pubkey| pubkey.to_string())
        .map_err(|err| format!("Invalid Solana public key from hardware wallet: {err}").into())
}

pub fn validate_signing_payload(
    capability: Esp32Capability,
    message_len: usize,
    negotiated_limit: usize,
) -> Result<(), ProtocolError> {
    if capability == Esp32Capability::CurrentV2 && message_len > negotiated_limit {
        Err(ProtocolError::PayloadTooLarge)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MLH_INFO: &str = "INFO;PROTOCOL=2;BLE_SECURITY=SC_MITM_BOND;SESSION_BOUND=1;VER=0.3.3;BOARD=dual-element-se050;DEVICE_MAC=E0:72:A1:E4:E8:F0;AUTH_MODE=PIN;FINALIZED=1;LOCKED=0;RETRIES_LEFT=10;KEY_STATE=READY;PQC_STATE=UNINITIALIZED;TX_V1=1;MAX_SIGN_BYTES=4096;PIN_BACKEND=SE050;SOLANA_BACKEND=TROPIC;PQC_BACKEND=DISABLED;PQC_SUPPORTED=0;TRANSPORTS=USB,BLE;ACTIVE_TRANSPORT=USB;BUILD=v2-ff212385a4d13c18;RESET_SUPPORTED=0;RESET_POLICY=PERMANENT_LOCKOUT;PIN_ATTEMPT_LIMIT=10;USB_HOST_LOCK=1;LOCAL_PIN=1;SESSION_UNLOCKED=0;UNLOCK_REMAINING_SECS=0;SIGN_REVIEW=PAGED;REVIEW_TIMEOUT_SECS=60";

    #[test]
    fn parses_exact_mlh_v033_capabilities() {
        let Response::Info(info) = parse_esp32_response_line(MLH_INFO).unwrap() else {
            panic!("expected info")
        };
        assert_eq!(info.capability(), Esp32Capability::CurrentV2);
        assert_eq!(info.version, "0.3.3");
        assert_eq!(info.build_id.as_deref(), Some("v2-ff212385a4d13c18"));
        assert!(info.supports_device_pin);
        assert!(info.session_bound);
        assert!(!info.session_unlocked);
        assert_eq!(info.authoritative_unlock_remaining_secs(), Some(0));
        assert_eq!(info.pin_backend.as_deref(), Some("SE050"));
        assert_eq!(info.solana_backend.as_deref(), Some("TROPIC"));
        assert_eq!(info.max_sign_message_bytes, 4096);
        assert!(!info.reset_supported);
        assert_eq!(info.reset_policy.as_deref(), Some("PERMANENT_LOCKOUT"));
    }

    #[test]
    fn preserves_legacy_info_defaults() {
        let Response::Info(info) = parse_esp32_response_line(
            "INFO;VER=1.0;AUTH_MODE=OTP;FINALIZED=1;LOCKED=0;RETRIES_LEFT=3",
        )
        .unwrap() else {
            panic!("expected info")
        };
        assert_eq!(info.capability(), Esp32Capability::NewV1);
        assert_eq!(info.protocol_version, 1);
        assert!(info.reset_supported);
        assert!(!info.supports_device_pin);
    }

    #[test]
    fn board_session_state_is_authoritative_and_bounded() {
        let Response::Info(info) = parse_esp32_response_line(
            "INFO;PROTOCOL=2;BOARD=v2;KEY_STATE=READY;PQC_STATE=UNINITIALIZED;SESSION_BOUND=1;SESSION_UNLOCKED=1;UNLOCK_REMAINING_SECS=9999",
        )
        .unwrap()
        else {
            panic!("expected info")
        };
        assert_eq!(
            info.authoritative_unlock_remaining_secs(),
            Some(CURRENT_V2_UNLOCK_WINDOW_SECS)
        );
    }

    #[test]
    fn formats_v2_commands_and_parses_typed_results() {
        assert_eq!(format_esp32_command(Command::Lock), b"LOCK\n");
        assert_eq!(
            format_esp32_command(Command::UnlockOnDevice),
            b"UNLOCK:DEVICE\n"
        );
        assert_eq!(format_esp32_command(Command::Generate), b"GENERATE\n");
        assert_eq!(
            parse_esp32_response_line("ERROR:USER_REJECTED").unwrap(),
            Response::Error(ProtocolError::UserRejected)
        );
        assert_eq!(
            parse_esp32_response_line("ERROR:RESET_UNAVAILABLE").unwrap(),
            Response::Error(ProtocolError::ResetUnavailable)
        );
        assert_eq!(
            parse_esp32_response_line("ERROR:BUSY").unwrap(),
            Response::Error(ProtocolError::Busy)
        );
    }

    #[test]
    fn validates_real_solana_keys_and_negotiated_sign_limit() {
        assert!(parse_hardware_pubkey("So11111111111111111111111111111111111111112").is_ok());
        assert!(parse_hardware_pubkey("111111111111111111111111111111111").is_err());
        assert_eq!(
            validate_signing_payload(Esp32Capability::CurrentV2, 4097, 4096),
            Err(ProtocolError::PayloadTooLarge)
        );
        assert!(validate_signing_payload(Esp32Capability::LegacyV0, 4097, 2048).is_ok());
    }

    #[test]
    fn ignores_boot_logs_and_accepts_clipped_legacy_unlock() {
        let response = parse_esp32_response(
            b"KEY_GENERATED\nPUBKEY:So11111111111111111111111111111111111111112\n",
        )
        .unwrap();
        assert!(matches!(response, Response::Pubkey(_)));
        assert_eq!(
            parse_esp32_response_line("NLOCKED_until:123456").unwrap(),
            Response::UnlockedUntil(123456)
        );
    }
}
