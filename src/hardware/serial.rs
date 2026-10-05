use crate::hardware::protocol::{
    format_esp32_command, parse_esp32_response_line, Command, Response,
};
use serialport::SerialPortInfo;
use std::error::Error;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::Mutex;
use tokio_serial::{
    ClearBuffer, SerialPort as TokioSerialPort, SerialPortBuilderExt, SerialStream,
};

const DEFAULT_RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);
const BUTTON_FLOW_RESPONSE_TIMEOUT: Duration = Duration::from_secs(45);
const DEVICE_PIN_RESPONSE_TIMEOUT: Duration = Duration::from_secs(90);
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
    usable: AtomicBool,
}

impl SerialConnection {
    fn response_timeout_for(command: &Command) -> Duration {
        match command {
            Command::UnlockOnDevice => DEVICE_PIN_RESPONSE_TIMEOUT,
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

    pub async fn find_and_connect() -> Result<Self, Box<dyn Error>> {
        let ports = serialport::available_ports()
            .map_err(|err| format!("Could not enumerate serial ports: {err}"))?;
        let mut candidates = Vec::new();
        let mut open_failures = Vec::new();

        for port_info in ports {
            if Self::is_hardware_wallet(&port_info) {
                candidates.push(port_info.port_name.clone());
                match Self::connect(&port_info.port_name).await {
                    Ok(connection) => return Ok(connection),
                    Err(err) => open_failures.push(format!("{}: {err}", port_info.port_name)),
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

    pub fn check_device_presence() -> bool {
        Self::scan_report().hardware_wallet_present
    }

    /// Produce support-friendly serial diagnostics without opening a port.
    /// USB serial numbers are deliberately omitted.
    pub fn scan_report() -> SerialScanReport {
        let ports = match serialport::available_ports() {
            Ok(ports) => ports,
            Err(err) => {
                return SerialScanReport {
                    hardware_wallet_present: false,
                    user_hint: Some(
                        "The app could not enumerate serial ports. Reconnect the wallet and check Device Manager."
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
        let (platform_usb_lines, platform_wallet_present) = Self::platform_usb_diagnostics();
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
        lines.extend(platform_usb_lines);

        let user_hint = if hardware_wallet_present {
            None
        } else if cfg!(target_os = "windows") && platform_wallet_present {
            Some(
                "Windows sees the wallet over USB but has not exposed a compatible COM port. Open Device Manager and install or repair only the driver matching the Hardware ID shown in the diagnostics."
                    .to_string(),
            )
        } else if cfg!(target_os = "windows") && ports.is_empty() {
            Some(
                "Windows detected USB activity but did not expose a COM port. Check both “Ports (COM & LPT)” and “Other devices” in Device Manager."
                    .to_string(),
            )
        } else if cfg!(target_os = "windows") {
            Some(
                "Windows exposed serial ports, but none matched the wallet. Copy the diagnostics and compare the VID/PID with Device Manager."
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

    fn platform_usb_diagnostics() -> (Vec<String>, bool) {
        #[cfg(target_os = "windows")]
        {
            return windows_pnp_diagnostics();
        }

        #[cfg(not(target_os = "windows"))]
        {
            (Vec::new(), false)
        }
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

    pub async fn connect(port_name: &str) -> Result<Self, Box<dyn Error>> {
        let mut port = tokio_serial::new(port_name, 115200)
            .timeout(Duration::from_millis(250))
            .open_native_async()?;
        let _ = port.write_data_terminal_ready(false);
        let _ = port.write_request_to_send(false);
        tokio::time::sleep(STARTUP_SETTLE_DELAY).await;
        let _ = port.clear(ClearBuffer::All);
        Self::flush_startup_noise(&mut port).await;
        Ok(Self {
            port: Arc::new(Mutex::new(port)),
            usable: AtomicBool::new(true),
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

    /// Any uncertain or canceled exchange invalidates the serial session.
    pub async fn send_command(&self, command: Command) -> Result<Response, Box<dyn Error>> {
        let response_timeout = Self::response_timeout_for(&command);
        let command_bytes = zeroize::Zeroizing::new(format_esp32_command(command));
        let mut port = self.port.lock().await;
        if !self.usable.load(Ordering::Acquire) {
            return Err("USB session ended. Reconnect the hardware wallet.".into());
        }
        let _ = port.clear(ClearBuffer::Input);
        self.usable.store(false, Ordering::Release);
        let result = exchange_serial_command(&mut *port, &command_bytes, response_timeout).await;
        if result.is_ok() {
            self.usable.store(true, Ordering::Release);
        }
        result.map_err(Into::into)
    }
}

#[cfg(target_os = "windows")]
fn windows_pnp_diagnostics() -> (Vec<String>, bool) {
    use std::io::Read;
    use std::os::windows::process::CommandExt;
    use std::process::{Command as ProcessCommand, Stdio};
    use std::thread;
    use std::time::Instant;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const PNP_TIMEOUT: Duration = Duration::from_secs(3);
    const SCRIPT: &str = r#"
$ids = @('VID_303A&PID_1001','VID_10C4&PID_EA60','VID_1A86&PID_7523','VID_0403&PID_6001')
Get-CimInstance Win32_PnPEntity -ErrorAction SilentlyContinue |
  Where-Object {
    $deviceId = $_.PNPDeviceID
    @($ids | Where-Object { $deviceId -like ('*' + $_ + '*') }).Count -gt 0
  } |
  ForEach-Object {
    $hardwareId = [regex]::Match($_.PNPDeviceID, 'VID_[0-9A-F]{4}&PID_[0-9A-F]{4}', 'IgnoreCase').Value
    '{0}`t{1}`t{2}' -f $_.Status, $_.Name, $hardwareId
  }
"#;

    let mut child = match ProcessCommand::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(err) => return (vec![format!("Windows PnP scan unavailable: {err}")], false),
    };

    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut output = String::new();
                if let Some(mut stdout) = child.stdout.take() {
                    let _ = stdout.read_to_string(&mut output);
                }
                if !status.success() {
                    return (
                        vec![format!("Windows PnP scan failed with status {status}")],
                        false,
                    );
                }

                let devices = output
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .map(|line| format!("  {line}"))
                    .collect::<Vec<_>>();
                let present = !devices.is_empty();
                let mut lines = vec!["Windows PnP wallet devices:".to_string()];
                if present {
                    lines.extend(devices);
                } else {
                    lines.push("  (none matched)".to_string());
                }
                return (lines, present);
            }
            Ok(None) if started.elapsed() < PNP_TIMEOUT => {
                thread::sleep(Duration::from_millis(25));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return (vec!["Windows PnP scan timed out".to_string()], false);
            }
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                return (vec![format!("Windows PnP scan failed: {err}")], false);
            }
        }
    }
}

async fn exchange_serial_command<S: AsyncRead + AsyncWrite + Unpin>(
    port: &mut S,
    command: &[u8],
    timeout: Duration,
) -> Result<Response, String> {
    tokio::time::timeout(timeout, async {
        port.write_all(command)
            .await
            .map_err(|err| format!("Failed to write USB command: {err}"))?;
        port.flush()
            .await
            .map_err(|err| format!("Failed to flush USB command: {err}"))?;
        let mut line = Vec::with_capacity(256);
        let mut byte = [0u8; 1];
        loop {
            port.read_exact(&mut byte)
                .await
                .map_err(|err| format!("USB connection closed or failed: {err}"))?;
            match byte[0] {
                b'\r' => {}
                b'\n' => {
                    if let Ok(response) = parse_esp32_response_line(&String::from_utf8_lossy(&line))
                    {
                        return Ok(response);
                    }
                    line.clear();
                }
                ch => {
                    if line.len() == MAX_LINE_BYTES {
                        return Err(
                            "USB response exceeded the line limit. Reconnect the hardware wallet."
                                .to_string(),
                        );
                    }
                    line.push(ch);
                }
            }
        }
    })
    .await
    .map_err(|_| "USB command timed out. Reconnect the hardware wallet.".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    #[test]
    fn recognizes_supported_usb_identities() {
        assert!(SerialConnection::is_supported_usb_id(0x303A, 0x1001));
        assert!(SerialConnection::is_supported_usb_id(0x10C4, 0xEA60));
        assert!(SerialConnection::has_wallet_usb_identity(
            Some("Unruggable Engineering"),
            Some("Hardware Wallet")
        ));
        assert!(SerialConnection::has_wallet_usb_identity(
            Some("Espressif Systems"),
            Some("USB JTAG/serial debug unit")
        ));
        assert!(!SerialConnection::has_wallet_usb_identity(
            Some("Example Corp"),
            Some("USB Serial Device")
        ));
    }

    #[test]
    fn silent_or_blocked_peer_cannot_hold_exchange_open() {
        runtime().block_on(async {
            let (mut silent_host, _silent_peer) = tokio::io::duplex(64);
            assert!(exchange_serial_command(
                &mut silent_host,
                b"PING\n",
                Duration::from_millis(20)
            )
            .await
            .unwrap_err()
            .contains("timed out"));

            let (mut blocked_host, _blocked_peer) = tokio::io::duplex(1);
            assert!(exchange_serial_command(
                &mut blocked_host,
                b"PING\n",
                Duration::from_millis(20)
            )
            .await
            .unwrap_err()
            .contains("timed out"));
        });
    }

    #[test]
    fn disconnect_noise_and_overlong_replies_are_handled_safely() {
        runtime().block_on(async {
            let (mut disconnected_host, peer) = tokio::io::duplex(64);
            drop(peer);
            assert!(exchange_serial_command(
                &mut disconnected_host,
                b"PING\n",
                Duration::from_secs(1)
            )
            .await
            .is_err());

            let (mut noisy_host, mut noisy_peer) = tokio::io::duplex(256);
            noisy_peer
                .write_all(b"Booting\r\nKEY_GENERATED\nPUBKEY:example\n")
                .await
                .unwrap();
            assert!(matches!(
                exchange_serial_command(&mut noisy_host, b"GENERATE\n", Duration::from_secs(1))
                    .await
                    .unwrap(),
                Response::Pubkey(key) if key == "example"
            ));

            let (mut long_host, mut long_peer) = tokio::io::duplex(MAX_LINE_BYTES + 64);
            let mut input = vec![b'X'; MAX_LINE_BYTES + 1];
            input.extend_from_slice(b"PONG\n");
            long_peer.write_all(&input).await.unwrap();
            assert!(
                exchange_serial_command(&mut long_host, b"PING\n", Duration::from_secs(1))
                    .await
                    .unwrap_err()
                    .contains("line limit")
            );
        });
    }
}
