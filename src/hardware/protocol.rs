use base64::Engine;
use std::error::Error;

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
pub enum Esp32Capability {
    LegacyV0,
    NewV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub version: String,
    pub auth_mode: AuthMode,
    pub finalized: bool,
    pub locked: bool,
    pub retries_left: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtpSetupData {
    pub secret: String,
    pub uri: String,
    pub algo: String,
    pub digits: u8,
    pub period: u32,
}

#[derive(Debug, Clone)]
pub enum Command {
    Ping,
    GetInfo,
    SetTime(u64),
    GetPubkey,
    SetModeNone,
    SetModePin(String),
    SetModeOtpBegin,
    SetModeOtpConfirm(String),
    UnlockPin(String),
    UnlockOtp(String),
    SignMessage(Vec<u8>),
}

#[derive(Debug, Clone)]
pub enum Response {
    Pong,
    Info(DeviceInfo),
    Pubkey(String),
    Signature(Vec<u8>),
    ModeSet(AuthMode),
    OtpSetup(OtpSetupData),
    TimeSet(u64),
    UnlockedUntil(u64),
    Error(String),
}

pub fn format_esp32_command(cmd: &Command) -> Vec<u8> {
    match cmd {
        Command::Ping => b"PING\n".to_vec(),
        Command::GetInfo => b"GET_INFO\n".to_vec(),
        Command::SetTime(unix_secs) => format!("SET_TIME:{unix_secs}\n").into_bytes(),
        Command::GetPubkey => b"GET_PUBKEY\n".to_vec(),
        Command::SetModeNone => b"SET_MODE:NONE\n".to_vec(),
        Command::SetModePin(pin) => format!("SET_MODE:PIN:{pin}\n").into_bytes(),
        Command::SetModeOtpBegin => b"SET_MODE:OTP_BEGIN\n".to_vec(),
        Command::SetModeOtpConfirm(code) => format!("SET_MODE:OTP_CONFIRM:{code}\n").into_bytes(),
        Command::UnlockPin(pin) => format!("UNLOCK:PIN:{pin}\n").into_bytes(),
        Command::UnlockOtp(code) => format!("UNLOCK:OTP:{code}\n").into_bytes(),
        Command::SignMessage(data) => {
            let mut formatted = b"SIGN:".to_vec();
            let encoded = base64::engine::general_purpose::STANDARD.encode(data);
            formatted.extend_from_slice(encoded.as_bytes());
            formatted.push(b'\n');
            formatted
        }
    }
}

fn parse_bool_flag(value: &str) -> Result<bool, Box<dyn Error>> {
    match value {
        "0" | "false" | "FALSE" => Ok(false),
        "1" | "true" | "TRUE" => Ok(true),
        other => Err(format!("Invalid bool flag: {other}").into()),
    }
}

fn parse_info_response(line: &str) -> Result<DeviceInfo, Box<dyn Error>> {
    let payload = line.strip_prefix("INFO;").ok_or("Missing INFO prefix")?;

    let mut version = String::from("unknown");
    let mut auth_mode = AuthMode::Unset;
    let mut finalized = false;
    let mut locked = false;
    let mut retries_left = 0u8;

    for part in payload.split(';') {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };

        match key {
            "VER" => version = value.to_string(),
            "AUTH_MODE" => auth_mode = AuthMode::from_wire(value)?,
            "FINALIZED" => finalized = parse_bool_flag(value)?,
            "LOCKED" => locked = parse_bool_flag(value)?,
            "RETRIES_LEFT" => {
                retries_left = value
                    .parse::<u8>()
                    .map_err(|e| format!("Invalid retries_left value '{value}': {e}"))?
            }
            _ => {}
        }
    }

    Ok(DeviceInfo {
        version,
        auth_mode,
        finalized,
        locked,
        retries_left,
    })
}

fn parse_mode_set(line: &str) -> Result<AuthMode, Box<dyn Error>> {
    let mode = line
        .strip_prefix("MODE_SET:")
        .ok_or("Missing MODE_SET prefix")?;
    AuthMode::from_wire(mode)
}

fn parse_otp_setup(line: &str) -> Result<OtpSetupData, Box<dyn Error>> {
    let payload = line
        .strip_prefix("OTP_SECRET:")
        .ok_or("Missing OTP_SECRET prefix")?;

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

    let uri = uri.ok_or("Missing OTP URI")?;

    Ok(OtpSetupData {
        secret,
        uri,
        algo,
        digits,
        period,
    })
}

pub fn parse_esp32_response_line(line: &str) -> Result<Response, Box<dyn Error>> {
    let response_str = line.trim();

    if response_str.is_empty() {
        return Err("Empty response".into());
    }

    if let Some(error) = response_str.strip_prefix("ERROR:") {
        return Ok(Response::Error(error.to_string()));
    }

    if response_str == "PONG" {
        return Ok(Response::Pong);
    }

    if response_str.starts_with("INFO;") {
        return Ok(Response::Info(parse_info_response(response_str)?));
    }

    if let Some(pubkey) = response_str.strip_prefix("PUBKEY:") {
        return Ok(Response::Pubkey(pubkey.to_string()));
    }

    if let Some(sig_b64) = response_str.strip_prefix("SIGNATURE:") {
        let sig_bytes = base64::engine::general_purpose::STANDARD.decode(sig_b64)?;
        return Ok(Response::Signature(sig_bytes));
    }

    if response_str.starts_with("MODE_SET:") {
        return Ok(Response::ModeSet(parse_mode_set(response_str)?));
    }

    if response_str == "OTP_CONFIRMED" {
        return Ok(Response::ModeSet(AuthMode::Otp));
    }

    if response_str.starts_with("OTP_SECRET:") {
        return Ok(Response::OtpSetup(parse_otp_setup(response_str)?));
    }

    if let Some(unix_str) = response_str.strip_prefix("TIME_SET:") {
        let unix = unix_str
            .parse::<u64>()
            .map_err(|e| format!("Invalid TIME_SET value '{unix_str}': {e}"))?;
        return Ok(Response::TimeSet(unix));
    }

    if let Some(until_str) = response_str.strip_prefix("UNLOCKED_UNTIL:") {
        let until = until_str
            .parse::<u64>()
            .map_err(|e| format!("Invalid UNLOCKED_UNTIL value '{until_str}': {e}"))?;
        return Ok(Response::UnlockedUntil(until));
    }

    Err(format!("Unknown response format: {response_str}").into())
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
