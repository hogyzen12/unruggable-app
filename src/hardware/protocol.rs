#![allow(dead_code)]

use base64::Engine as _;
use solana_sdk::pubkey::Pubkey;
use std::{error::Error, fmt, str::FromStr};

pub const HARDWARE_PIN_DIGITS: usize = 6;
pub const CURRENT_V2_MAX_SIGN_BYTES: usize = 2_048;

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
    pub board: Option<String>,
    pub auth_mode: AuthMode,
    pub finalized: bool,
    pub locked: bool,
    pub retries_left: u8,
    pub key_state: KeyState,
    pub pqc_state: KeyState,
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

    pub fn uninitialized_legacy_compatible() -> Self {
        Self {
            version: "unknown".to_string(),
            board: None,
            auth_mode: AuthMode::Unset,
            finalized: false,
            locked: false,
            retries_left: 0,
            key_state: KeyState::Unknown,
            pqc_state: KeyState::Unknown,
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
    WalletAlreadyInitialized,
    KeystoreCorrupt,
    PayloadTooLarge,
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
            "WALLET_ALREADY_INITIALIZED" => Self::WalletAlreadyInitialized,
            "KEYSTORE_CORRUPT" => Self::KeystoreCorrupt,
            "PAYLOAD_TOO_LARGE" => Self::PayloadTooLarge,
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
            Self::WalletAlreadyInitialized => "WALLET_ALREADY_INITIALIZED",
            Self::KeystoreCorrupt => "KEYSTORE_CORRUPT",
            Self::PayloadTooLarge => "PAYLOAD_TOO_LARGE",
            Self::TimeNotSet => "TIME_NOT_SET",
            Self::Unsupported => "UNSUPPORTED",
            Self::UnknownCommand => "Unknown command",
            Self::Busy => "BUSY",
            Self::Other(message) => message,
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

#[derive(Debug)]
pub enum Response {
    Pong,
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
    // String is valid UTF-8 before and after filling its existing allocation with zeroes.
    unsafe {
        value.as_mut_vec().fill(0);
    }
    value.clear();
}

pub fn format_esp32_command(cmd: Command) -> Vec<u8> {
    match cmd {
        Command::Ping => b"PING\n".to_vec(),
        Command::GetInfo => b"GET_INFO\n".to_vec(),
        Command::SetTime(unix_secs) => format!("SET_TIME:{unix_secs}\n").into_bytes(),
        Command::GetPubkey => b"GET_PUBKEY\n".to_vec(),
        Command::Generate => b"GENERATE\n".to_vec(),
        Command::ShowReceiveQr => b"SHOW_RECEIVE_QR\n".to_vec(),
        Command::HideReceiveQr => b"HIDE_RECEIVE_QR\n".to_vec(),
        Command::SetModeNone => b"SET_MODE:NONE\n".to_vec(),
        Command::SetModePin(mut pin) => {
            let bytes = format!("SET_MODE:PIN:{pin}\n").into_bytes();
            clear_sensitive_string(&mut pin);
            bytes
        }
        Command::SetModeOtpBegin => b"SET_MODE:OTP_BEGIN\n".to_vec(),
        Command::SetModeOtpConfirm(mut code) => {
            let bytes = format!("SET_MODE:OTP_CONFIRM:{code}\n").into_bytes();
            clear_sensitive_string(&mut code);
            bytes
        }
        Command::UnlockPin(mut pin) => {
            let bytes = format!("UNLOCK:PIN:{pin}\n").into_bytes();
            clear_sensitive_string(&mut pin);
            bytes
        }
        Command::UnlockOtp(mut code) => {
            let bytes = format!("UNLOCK:OTP:{code}\n").into_bytes();
            clear_sensitive_string(&mut code);
            bytes
        }
        Command::SignMessage(mut data) => {
            let mut encoded = base64::engine::general_purpose::STANDARD.encode(&data);
            data.fill(0);
            let bytes = format!("SIGN:{encoded}\n").into_bytes();
            clear_sensitive_string(&mut encoded);
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

    let mut version = String::from("unknown");
    let mut board = None;
    let mut auth_mode = AuthMode::Unset;
    let mut finalized = false;
    let mut locked = false;
    let mut retries_left = 0u8;
    let mut key_state = KeyState::Unknown;
    let mut pqc_state = KeyState::Unknown;
    let mut saw_board = false;
    let mut saw_key_state = false;
    let mut saw_pqc_state = false;

    for part in payload.split(';') {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };

        match key {
            "VER" => version = value.to_string(),
            "BOARD" => {
                saw_board = true;
                board = Some(value.to_string());
            }
            "AUTH_MODE" => auth_mode = AuthMode::from_wire(value)?,
            "FINALIZED" => finalized = parse_bool_flag(value)?,
            "LOCKED" => locked = parse_bool_flag(value)?,
            "RETRIES_LEFT" => {
                retries_left = value
                    .parse::<u8>()
                    .map_err(|e| format!("Invalid retries_left value '{value}': {e}"))?
            }
            "KEY_STATE" => {
                saw_key_state = true;
                key_state = KeyState::from_wire(value);
            }
            "PQC_STATE" => {
                saw_pqc_state = true;
                pqc_state = KeyState::from_wire(value);
            }
            _ => {}
        }
    }

    Ok(DeviceInfo {
        version,
        board,
        auth_mode,
        finalized,
        locked,
        retries_left,
        key_state,
        pqc_state,
        has_current_v2_fields: saw_board && saw_key_state && saw_pqc_state,
    })
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
    let mut uri: Option<String> = None;
    let mut algo = String::from("SHA1");
    let mut digits = 6u8;
    let mut period = 30u32;

    for part in parts {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        match key {
            "URI" => uri = Some(value.to_string()),
            "ALGO" => algo = value.to_string(),
            "DIGITS" => {
                digits = value
                    .parse::<u8>()
                    .map_err(|e| format!("Invalid DIGITS value '{value}': {e}"))?
            }
            "PERIOD" => {
                period = value
                    .parse::<u32>()
                    .map_err(|e| format!("Invalid PERIOD value '{value}': {e}"))?
            }
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
    if let Some(pubkey) = strip_prefix_ascii_case(response, "PUBKEY:") {
        return Ok(Response::Pubkey(pubkey.to_string()));
    }
    if let Some(sig_b64) = strip_prefix_ascii_case(response, "SIGNATURE:") {
        let sig_bytes = base64::engine::general_purpose::STANDARD.decode(sig_b64)?;
        return Ok(Response::Signature(sig_bytes));
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
        let unix = unix_str
            .parse::<u64>()
            .map_err(|e| format!("Invalid TIME_SET value '{unix_str}': {e}"))?;
        return Ok(Response::TimeSet(unix));
    }
    if let Some(until_str) = strip_prefix_ascii_case(response, "UNLOCKED_UNTIL:")
        .or_else(|| strip_prefix_ascii_case(response, "NLOCKED_UNTIL:"))
    {
        let until = until_str
            .parse::<u64>()
            .map_err(|e| format!("Invalid UNLOCKED_UNTIL value '{until_str}': {e}"))?;
        return Ok(Response::UnlockedUntil(until));
    }
    if response.eq_ignore_ascii_case("WIPED") {
        return Ok(Response::Wiped);
    }

    Err(format!("Unknown response format: {response}").into())
}

pub fn parse_esp32_response(data: &[u8]) -> Result<Response, Box<dyn Error>> {
    let response_str = String::from_utf8_lossy(data);
    let mut saw_non_empty = false;

    for line in response_str.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        saw_non_empty = true;
        if let Ok(parsed) = parse_esp32_response_line(trimmed) {
            return Ok(parsed);
        }
    }

    if !saw_non_empty {
        return Err("Empty response".into());
    }

    Err(format!("Unknown response format: {}", response_str.trim()).into())
}

pub fn parse_hardware_pubkey(value: &str) -> Result<String, Box<dyn Error>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("Hardware wallet returned an empty public key".into());
    }

    Pubkey::from_str(trimmed)
        .map(|pubkey| pubkey.to_string())
        .map_err(|err| format!("Invalid Solana public key from hardware wallet: {err}").into())
}

pub fn validate_signing_payload(
    capability: Esp32Capability,
    message_len: usize,
) -> Result<(), ProtocolError> {
    if capability == Esp32Capability::CurrentV2 && message_len > CURRENT_V2_MAX_SIGN_BYTES {
        return Err(ProtocolError::PayloadTooLarge);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_current_v2_from_required_info_fields() {
        let response = parse_esp32_response_line(
            "INFO;VER=1.2.3;BOARD=dual-element-se050;AUTH_MODE=PIN;FINALIZED=1;LOCKED=0;RETRIES_LEFT=5;KEY_STATE=READY;PQC_STATE=UNINITIALIZED",
        )
        .expect("valid info");

        let Response::Info(info) = response else {
            panic!("expected info");
        };
        assert_eq!(info.capability(), Esp32Capability::CurrentV2);
        assert_eq!(info.board.as_deref(), Some("dual-element-se050"));
        assert_eq!(info.key_state, KeyState::Ready);
        assert_eq!(info.pqc_state, KeyState::Uninitialized);
    }

    #[test]
    fn preserves_new_v1_info_compatibility() {
        let response = parse_esp32_response_line(
            "INFO;VER=1.0;AUTH_MODE=OTP;FINALIZED=1;LOCKED=0;RETRIES_LEFT=3",
        )
        .expect("valid info");
        let Response::Info(info) = response else {
            panic!("expected info");
        };
        assert_eq!(info.capability(), Esp32Capability::NewV1);
        assert_eq!(info.auth_mode, AuthMode::Otp);
    }

    #[test]
    fn parses_typed_errors() {
        for (wire, expected) in [
            ("PIN_REQUIRED", ProtocolError::PinRequired),
            ("AUTH_FAILED", ProtocolError::AuthFailed),
            ("AUTH_LOCKED", ProtocolError::AuthLocked),
            ("KEYSTORE_CORRUPT", ProtocolError::KeystoreCorrupt),
            ("CONFIRM_TIMEOUT", ProtocolError::ConfirmTimeout),
            ("PAYLOAD_TOO_LARGE", ProtocolError::PayloadTooLarge),
        ] {
            let parsed =
                parse_esp32_response_line(&format!("ERROR:{wire}")).expect("valid error response");
            assert!(matches!(parsed, Response::Error(error) if error == expected));
        }
    }

    #[test]
    fn formats_current_v2_commands() {
        assert_eq!(format_esp32_command(Command::Generate), b"GENERATE\n");
        assert_eq!(
            format_esp32_command(Command::ShowReceiveQr),
            b"SHOW_RECEIVE_QR\n"
        );
        assert_eq!(
            format_esp32_command(Command::HideReceiveQr),
            b"HIDE_RECEIVE_QR\n"
        );
        assert_eq!(format_esp32_command(Command::WipeKeys), b"WIPE_KEYS\n");
    }

    #[test]
    fn accepts_only_real_solana_pubkeys() {
        assert!(parse_hardware_pubkey("So11111111111111111111111111111111111111112").is_ok());
        assert!(parse_hardware_pubkey("111111111111111111111111111111111").is_err());
    }

    #[test]
    fn ignores_logs_and_parses_later_protocol_line() {
        let response = parse_esp32_response(
            b"KEY_GENERATED\nPUBKEY:So11111111111111111111111111111111111111112\n",
        )
        .expect("later pubkey should parse");
        assert!(matches!(response, Response::Pubkey(_)));
    }

    #[test]
    fn parses_clipped_unlock_response() {
        let response = parse_esp32_response_line("NLOCKED_until:123456")
            .expect("clipped response should parse");
        assert!(matches!(response, Response::UnlockedUntil(123456)));
    }

    #[test]
    fn enforces_current_v2_signing_limit_without_changing_legacy_limits() {
        assert!(validate_signing_payload(
            Esp32Capability::CurrentV2,
            CURRENT_V2_MAX_SIGN_BYTES
        )
        .is_ok());
        assert_eq!(
            validate_signing_payload(
                Esp32Capability::CurrentV2,
                CURRENT_V2_MAX_SIGN_BYTES + 1
            ),
            Err(ProtocolError::PayloadTooLarge)
        );
        assert!(validate_signing_payload(
            Esp32Capability::LegacyV0,
            CURRENT_V2_MAX_SIGN_BYTES + 1
        )
        .is_ok());
    }
}
