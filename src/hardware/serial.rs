// src/hardware/serial.rs
use serialport::SerialPortInfo;
use std::error::Error;
use std::time::Duration;
use tokio_serial::{SerialPortBuilderExt, SerialStream};
use crate::hardware::protocol::{Command, Response, format_esp32_command, parse_esp32_response};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct SerialConnection {
    port: Arc<Mutex<SerialStream>>,
}

impl SerialConnection {
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
                (usb_info.vid == 0x303A && usb_info.pid == 0x1001)    // ESP32-S3
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
            port: Arc::new(Mutex::new(port))
        })
    }
    
    /// Send a command and receive a response
    pub async fn send_command(&self, command: Command) -> Result<Response, Box<dyn Error>> {
        let cmd_bytes = format_esp32_command(&command);
        
        // Send command and read response using a single port lock
        let response_bytes = {
            let mut port = self.port.lock().await;
            
            // Send command
            port.write_all(&cmd_bytes).await?;
            port.flush().await?;
            
            // Read response line by line
            let mut response_buf = Vec::new();
            let mut byte = [0u8; 1];
            let mut timeout_count = 0;
            
            // Wait for response, timing out after 10 seconds
            loop {
                match port.read(&mut byte).await {
                    Ok(1) => {
                        response_buf.push(byte[0]);
                        if byte[0] == b'\n' {
                            break;
                        }
                        // Prevent buffer overflow
                        if response_buf.len() > 1024 {
                            return Err("Response too long".into());
                        }
                    }
                    Ok(0) => {
                        timeout_count += 1;
                        if timeout_count > 100 { // 10 seconds timeout
                            return Err("Timeout waiting for response".into());
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    Err(e) => {
                        // Check if it's a timeout error, if so, keep trying
                        timeout_count += 1;
                        if timeout_count > 100 { // 10 seconds timeout
                            return Err(format!("Read error after timeout: {}", e).into());
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    Ok(n) => return Err(format!("Unexpected read size: {}", n).into()),
                }
            }
            
            response_buf
        };
        
        parse_esp32_response(&response_bytes)
    }
}