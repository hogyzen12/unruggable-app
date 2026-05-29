use crate::hardware::protocol::{
    format_esp32_command, parse_esp32_response_line, Command, Response,
};
use serialport::SerialPortInfo;
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tokio_serial::{SerialPortBuilderExt, SerialStream};

const DEFAULT_RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);
const BUTTON_FLOW_RESPONSE_TIMEOUT: Duration = Duration::from_secs(45);
const MAX_LINE_BYTES: usize = 8192;

pub struct SerialConnection {
    port: Arc<Mutex<SerialStream>>,
}

impl SerialConnection {
    fn response_timeout_for(command: &Command) -> Duration {
        match command {
            Command::SetModeNone
            | Command::SetModePin(_)
            | Command::SetModeOtpBegin
            | Command::SignMessage(_) => BUTTON_FLOW_RESPONSE_TIMEOUT,
            _ => DEFAULT_RESPONSE_TIMEOUT,
        }
    }

    /// Find and connect to the first available hardware wallet
    pub async fn find_and_connect() -> Result<Self, Box<dyn Error>> {
        let ports = serialport::available_ports()?;

        for port_info in ports {
            if Self::is_hardware_wallet(&port_info) {
                match Self::connect(&port_info.port_name).await {
                    Ok(conn) => return Ok(conn),
                    Err(_) => continue, // Try next port
                }
            }
        }

        Err("No hardware wallet found".into())
    }

    /// Check if a hardware wallet is present without connecting
    pub fn check_device_presence() -> bool {
        if let Ok(ports) = serialport::available_ports() {
            for port_info in ports {
                if Self::is_hardware_wallet(&port_info) {
                    return true;
                }
            }
        }
        false
    }

    /// Check if a port looks like our hardware wallet
    fn is_hardware_wallet(port_info: &SerialPortInfo) -> bool {
        // Check for ESP32 USB identifiers
        match &port_info.port_type {
            serialport::SerialPortType::UsbPort(usb_info) => {
                // Common ESP32 USB VID/PID combinations
                (usb_info.vid == 0x10C4 && usb_info.pid == 0xEA60) || // CP2102
                (usb_info.vid == 0x1A86 && usb_info.pid == 0x7523) || // CH340
                (usb_info.vid == 0x0403 && usb_info.pid == 0x6001) || // FTDI
                (usb_info.vid == 0x303A && usb_info.pid == 0x1001) // ESP32-S3
            }
            _ => false,
        }
    }

    /// Connect to a specific port
    pub async fn connect(port_name: &str) -> Result<Self, Box<dyn Error>> {
        let port = tokio_serial::new(port_name, 115200)
            .timeout(Duration::from_millis(5000))
            .open_native_async()?;

        // Ensure the port is readable and writable
        tokio::time::sleep(Duration::from_millis(100)).await;

        Ok(Self {
            port: Arc::new(Mutex::new(port)),
        })
    }

    /// Send a command and read the first parseable protocol response line.
    pub async fn send_command(&self, command: Command) -> Result<Response, Box<dyn Error>> {
        let response_timeout = Self::response_timeout_for(&command);
        let cmd_bytes = format_esp32_command(&command);

        let mut port = self.port.lock().await;

        port.write_all(&cmd_bytes).await?;
        port.flush().await?;

        let deadline = Instant::now() + response_timeout;
        let mut line_buf = Vec::with_capacity(256);
        let mut byte = [0u8; 1];
        let mut last_non_protocol_line: Option<String> = None;

        while Instant::now() < deadline {
            match port.read(&mut byte).await {
                Ok(1) => {
                    let ch = byte[0];
                    if ch == b'\r' {
                        continue;
                    }

                    if ch == b'\n' {
                        if line_buf.is_empty() {
                            continue;
                        }

                        let line = String::from_utf8_lossy(&line_buf).trim().to_string();
                        line_buf.clear();

                        match parse_esp32_response_line(&line) {
                            Ok(response) => return Ok(response),
                            Err(_) => {
                                // Ignore boot logs/noise and keep waiting for the real response.
                                last_non_protocol_line = Some(line);
                            }
                        }
                        continue;
                    }

                    if line_buf.len() < MAX_LINE_BYTES {
                        line_buf.push(ch);
                    } else {
                        // Drop oversized noise lines safely.
                        line_buf.clear();
                    }
                }
                Ok(0) => {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                Ok(n) => {
                    return Err(format!("Unexpected read size: {n}").into());
                }
                Err(_) => {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            }
        }

        match last_non_protocol_line {
            Some(line) => Err(format!(
                "Timeout waiting for protocol response (last non-protocol line: {line})"
            )
            .into()),
            None => Err("Timeout waiting for protocol response".into()),
        }
    }
}
