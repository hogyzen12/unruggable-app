use crate::hardware::protocol::{
    format_esp32_command, parse_esp32_response_line, Command, Response,
};
use serialport::SerialPortInfo;
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tokio_serial::{
    ClearBuffer, SerialPort as TokioSerialPort, SerialPortBuilderExt, SerialStream,
};

const DEFAULT_RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);
const BUTTON_FLOW_RESPONSE_TIMEOUT: Duration = Duration::from_secs(45);
const SIGN_RESPONSE_TIMEOUT: Duration = Duration::from_secs(75);
const STARTUP_SETTLE_DELAY: Duration = Duration::from_secs(2);
const STARTUP_NOISE_READ_TIMEOUT: Duration = Duration::from_millis(60);
const MAX_LINE_BYTES: usize = 8192;

const SUPPORTED_USB_IDS: &[(u16, u16, &str)] = &[
    (0x10C4, 0xEA60, "Silicon Labs CP210x"),
    (0x1A86, 0x7523, "WCH CH340"),
    (0x0403, 0x6001, "FTDI FT232"),
    (0x303A, 0x1001, "Espressif USB Serial/JTAG"),
];

#[derive(Debug, Clone)]
pub struct SerialScanReport {
    pub hardware_wallet_present: bool,
    pub user_hint: Option<String>,
    pub diagnostic_text: String,
    pub fallback_port_names: Vec<String>,
}

pub struct SerialConnection {
    port: Arc<Mutex<SerialStream>>,
}

impl SerialConnection {
    fn response_timeout_for(command: &Command) -> Duration {
        match command {
            Command::SetModeNone
            | Command::SetModePin(_)
            | Command::SetModeOtpBegin
            | Command::SetModeOtpConfirm(_)
            | Command::UnlockPin(_)
            | Command::UnlockOtp(_)
            | Command::Generate
            | Command::ShowReceiveQr
            | Command::HideReceiveQr
            | Command::WipeKeys => BUTTON_FLOW_RESPONSE_TIMEOUT,
            Command::SignMessage(_) => SIGN_RESPONSE_TIMEOUT,
            _ => DEFAULT_RESPONSE_TIMEOUT,
        }
    }

    /// Find and connect to the first available hardware wallet
    pub async fn find_and_connect() -> Result<Self, Box<dyn Error>> {
        let ports = serialport::available_ports()
            .map_err(|err| format!("Could not enumerate serial ports: {err}"))?;
        let mut candidates = Vec::new();
        let mut open_failures = Vec::new();

        for port_info in ports {
            if Self::is_hardware_wallet(&port_info) {
                candidates.push(port_info.port_name.clone());
                match Self::connect(&port_info.port_name).await {
                    Ok(conn) => return Ok(conn),
                    Err(err) => {
                        open_failures.push(format!("{}: {err}", port_info.port_name));
                        continue;
                    }
                }
            }
        }

        if candidates.is_empty() {
            return Err(
                "No compatible hardware-wallet COM port was exposed by the operating system".into(),
            );
        }

        Err(format!(
            "Detected compatible port(s) {}, but could not open them: {}. Close serial monitors or other wallet tools and try again.",
            candidates.join(", "),
            open_failures.join("; ")
        )
        .into())
    }

    /// Check if a hardware wallet is present without connecting
    pub fn check_device_presence() -> bool {
        Self::scan_report().hardware_wallet_present
    }

    /// Return support-friendly discovery details without opening any serial port.
    /// Serial numbers are intentionally omitted from this report.
    pub fn scan_report() -> SerialScanReport {
        let ports = match serialport::available_ports() {
            Ok(ports) => ports,
            Err(err) => {
                return SerialScanReport {
                    hardware_wallet_present: false,
                    user_hint: Some(
                        "The app could not enumerate COM ports. Reconnect the device and check Device Manager."
                            .to_string(),
                    ),
                    diagnostic_text: format!(
                        "Hardware scan\nApp: {} {}\nOS: {} ({})\nSerial enumeration error: {err}\nExpected USB IDs: {}",
                        env!("CARGO_PKG_NAME"),
                        env!("CARGO_PKG_VERSION"),
                        std::env::consts::OS,
                        std::env::consts::ARCH,
                        Self::supported_usb_ids_label()
                    ),
                    fallback_port_names: Vec::new(),
                };
            }
        };

        let hardware_wallet_present = ports.iter().any(Self::is_hardware_wallet);
        let fallback_port_names = if cfg!(target_os = "windows") {
            ports
                .iter()
                .filter(|port| !Self::is_hardware_wallet(port))
                .filter(|port| port.port_name.to_ascii_uppercase().starts_with("COM"))
                .map(|port| port.port_name.clone())
                .collect()
        } else {
            Vec::new()
        };
        let mut lines = vec![
            "Hardware scan".to_string(),
            format!(
                "App: {} {}",
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION")
            ),
            format!("OS: {} ({})", std::env::consts::OS, std::env::consts::ARCH),
            format!("Expected USB IDs: {}", Self::supported_usb_ids_label()),
            format!("Serial ports found: {}", ports.len()),
        ];

        if ports.is_empty() {
            lines.push("  (none)".to_string());
        } else {
            lines.extend(ports.iter().map(Self::format_port_diagnostic));
        }

        let user_hint = if hardware_wallet_present {
            None
        } else if cfg!(target_os = "windows") && ports.is_empty() {
            Some(
                "Windows did not expose a COM port for the device. In Device Manager, check “Ports (COM & LPT)” and “Other devices.” Native ESP32-S3 units should appear as “USB JTAG/serial debug unit (COMx)”; older bridge-based units may require their CP210x, CH340, or FTDI VCP driver."
                    .to_string(),
            )
        } else if cfg!(target_os = "windows") {
            Some(
                "Windows exposed serial ports, but none matched the hardware wallet. Copy the connection details below so support can check the reported VID/PID."
                    .to_string(),
            )
        } else {
            None
        };

        SerialScanReport {
            hardware_wallet_present,
            user_hint,
            diagnostic_text: lines.join("\n"),
            fallback_port_names,
        }
    }

    /// Check if a port looks like our hardware wallet
    fn is_hardware_wallet(port_info: &SerialPortInfo) -> bool {
        match &port_info.port_type {
            serialport::SerialPortType::UsbPort(usb_info) => {
                Self::is_supported_usb_id(usb_info.vid, usb_info.pid)
                    || Self::has_wallet_usb_identity(
                        usb_info.manufacturer.as_deref(),
                        usb_info.product.as_deref(),
                    )
            }
            _ => false,
        }
    }

    fn is_supported_usb_id(vid: u16, pid: u16) -> bool {
        SUPPORTED_USB_IDS
            .iter()
            .any(|(expected_vid, expected_pid, _)| vid == *expected_vid && pid == *expected_pid)
    }

    fn has_wallet_usb_identity(manufacturer: Option<&str>, product: Option<&str>) -> bool {
        let identity = format!(
            "{} {}",
            manufacturer.unwrap_or_default(),
            product.unwrap_or_default()
        )
        .to_ascii_lowercase();

        identity.contains("unruggable")
            || (identity.contains("espressif")
                && (identity.contains("serial") || identity.contains("jtag")))
    }

    fn supported_usb_ids_label() -> String {
        SUPPORTED_USB_IDS
            .iter()
            .map(|(vid, pid, name)| format!("{vid:04X}:{pid:04X} ({name})"))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn format_port_diagnostic(port_info: &SerialPortInfo) -> String {
        let classification = if Self::is_hardware_wallet(port_info) {
            "compatible"
        } else {
            "not matched"
        };

        match &port_info.port_type {
            serialport::SerialPortType::UsbPort(usb_info) => format!(
                "  {}: USB VID={:04X} PID={:04X} manufacturer={} product={} [{}]",
                port_info.port_name,
                usb_info.vid,
                usb_info.pid,
                usb_info.manufacturer.as_deref().unwrap_or("unknown"),
                usb_info.product.as_deref().unwrap_or("unknown"),
                classification
            ),
            serialport::SerialPortType::BluetoothPort => {
                format!("  {}: Bluetooth [{}]", port_info.port_name, classification)
            }
            serialport::SerialPortType::PciPort => {
                format!("  {}: PCI [{}]", port_info.port_name, classification)
            }
            serialport::SerialPortType::Unknown => {
                format!(
                    "  {}: unknown type [{}]",
                    port_info.port_name, classification
                )
            }
        }
    }

    /// Connect to a specific port
    pub async fn connect(port_name: &str) -> Result<Self, Box<dyn Error>> {
        let mut port = tokio_serial::new(port_name, 115200)
            .timeout(Duration::from_millis(250))
            .open_native_async()?;

        // Avoid resetting the device again right after open, then give firmware time to boot.
        let _ = port.write_data_terminal_ready(false);
        let _ = port.write_request_to_send(false);

        tokio::time::sleep(STARTUP_SETTLE_DELAY).await;
        let _ = port.clear(ClearBuffer::All);
        Self::flush_startup_noise(&mut port).await;

        Ok(Self {
            port: Arc::new(Mutex::new(port)),
        })
    }

    async fn flush_startup_noise(port: &mut SerialStream) {
        let _ = port.clear(ClearBuffer::Input);

        let mut scratch = [0u8; 256];
        loop {
            match tokio::time::timeout(STARTUP_NOISE_READ_TIMEOUT, port.read(&mut scratch)).await {
                Ok(Ok(read)) if read > 0 => continue,
                Ok(Ok(_)) | Ok(Err(_)) | Err(_) => break,
            }
        }

        let _ = port.clear(ClearBuffer::Input);
    }

    /// Send a command and read the first parseable protocol response line.
    pub async fn send_command(&self, command: Command) -> Result<Response, Box<dyn Error>> {
        let response_timeout = Self::response_timeout_for(&command);
        let mut cmd_bytes = format_esp32_command(command);

        let mut port = self.port.lock().await;

        // Each request starts on a fresh protocol boundary; stale boot/status lines
        // should never be interpreted as this command's response.
        let _ = port.clear(ClearBuffer::Input);
        let write_result = port.write_all(&cmd_bytes).await;
        let flush_result = if write_result.is_ok() {
            Some(port.flush().await)
        } else {
            None
        };
        cmd_bytes.fill(0);

        write_result?;
        if let Some(result) = flush_result {
            result?;
        }

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

#[cfg(test)]
mod tests {
    use super::SerialConnection;

    #[test]
    fn recognizes_native_esp32_s3_usb_serial_jtag_id() {
        assert!(SerialConnection::is_supported_usb_id(0x303A, 0x1001));
    }

    #[test]
    fn recognizes_unruggable_or_espressif_identity_with_custom_pid() {
        assert!(SerialConnection::has_wallet_usb_identity(
            Some("Unruggable Engineering"),
            Some("First Edition")
        ));
        assert!(SerialConnection::has_wallet_usb_identity(
            Some("Espressif Systems"),
            Some("USB JTAG/serial debug unit")
        ));
    }

    #[test]
    fn does_not_treat_arbitrary_usb_serial_text_as_wallet_identity() {
        assert!(!SerialConnection::has_wallet_usb_identity(
            Some("Example Corp"),
            Some("USB Serial Device")
        ));
    }
}
