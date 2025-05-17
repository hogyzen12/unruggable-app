// src/hardware/protocol.rs
use std::error::Error;
use base64::Engine; // Add this import

/// Command types that can be sent to the hardware wallet
#[derive(Debug, Clone)]
pub enum Command {
    GetPubkey,
    SignMessage(Vec<u8>),
}

/// Response types from the hardware wallet
#[derive(Debug, Clone)]
pub enum Response {
    Pubkey(String),
    Signature(Vec<u8>),
    Error(String),
}

/// Convert the protocol to match ESP32 expectations
pub fn format_esp32_command(cmd: &Command) -> Vec<u8> {
    match cmd {
        Command::GetPubkey => b"GET_PUBKEY\n".to_vec(),
        Command::SignMessage(data) => {
            let mut formatted = b"SIGN:".to_vec();
            // Use the standard base64 engine
            let encoded = base64::engine::general_purpose::STANDARD.encode(data);
            formatted.extend_from_slice(encoded.as_bytes());
            formatted.push(b'\n');
            formatted
        }
    }
}

/// Parse ESP32 response format
pub fn parse_esp32_response(data: &[u8]) -> Result<Response, Box<dyn Error>> {
    let response_str = String::from_utf8_lossy(data);
    let response_str = response_str.trim();
    
    if response_str.starts_with("PUBKEY:") {
        let pubkey = response_str.strip_prefix("PUBKEY:").unwrap();
        Ok(Response::Pubkey(pubkey.to_string()))
    } else if response_str.starts_with("SIGNATURE:") {
        let sig_b64 = response_str.strip_prefix("SIGNATURE:").unwrap();
        // Use the standard base64 engine
        let sig_bytes = base64::engine::general_purpose::STANDARD.decode(sig_b64)?;
        Ok(Response::Signature(sig_bytes))
    } else if response_str.starts_with("ERROR:") {
        let error = response_str.strip_prefix("ERROR:").unwrap();
        Ok(Response::Error(error.to_string()))
    } else {
        Err(format!("Unknown response format: {}", response_str).into())
    }
}