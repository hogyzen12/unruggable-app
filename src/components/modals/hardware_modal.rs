use crate::components::pin_input::PinInput;
use crate::hardware::{
    AuthMode, Esp32Capability, HardwareDeviceInfo, HardwareDeviceType, HardwareWallet,
    LedgerDerivationAddress,
};
use crate::{rpc, storage::load_rpc_from_storage};
use dioxus::prelude::*;
use std::{collections::HashMap, sync::Arc, time::Duration};

const ICON_UNRUGGABLE: &str =
    "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/icon.png";
const ICON_LEDGER: &str =
    "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/icons/ledgerLogo.webp";
const DEFAULT_RPC_URL: &str = "https://johna-k3cr1v-fast-mainnet.helius-rpc.com";
const LEDGER_SCAN_BATCH_SIZE: u32 = 50;
const LEDGER_SCAN_MAX_ACCOUNTS: u32 = 100;
const ESP32_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const LEDGER_CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PinSetupStep {
    Enter,
    Confirm,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConnectUnlockMode {
    Pin,
    Otp,
}

fn format_auth_mode(mode: AuthMode) -> &'static str {
    match mode {
        AuthMode::Unset => "UNSET",
        AuthMode::None => "NONE",
        AuthMode::Pin => "PIN",
        AuthMode::Otp => "OTP",
    }
}

fn valid_six_digits(value: &str) -> bool {
    value.len() == 6 && value.chars().all(|c| c.is_ascii_digit())
}

fn short_pubkey(pubkey: &str) -> String {
    if pubkey.len() > 12 {
        format!("{}...{}", &pubkey[..6], &pubkey[pubkey.len() - 6..])
    } else {
        pubkey.to_string()
    }
}

fn active_rpc_url_for_ledger_scan() -> String {
    load_rpc_from_storage()
        .filter(|url| !url.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_RPC_URL.to_string())
}

fn ledger_scan_changes(primary_change: u32) -> Vec<u32> {
    let mut changes = Vec::with_capacity(3);
    for change in [primary_change, 0, 1] {
        if !changes.contains(&change) {
            changes.push(change);
        }
    }
    changes
}

fn no_funded_ledger_paths_message(primary_change: u32) -> String {
    let changes = ledger_scan_changes(primary_change);
    let changes_label = changes
        .iter()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "No Ledger addresses with SOL found (scanned changes [{}], accounts 0..{}). You can still enter a custom path.",
        changes_label,
        LEDGER_SCAN_MAX_ACCOUNTS.saturating_sub(1)
    )
}

fn connect_timeout_for(device_type: &HardwareDeviceType) -> Duration {
    match device_type {
        HardwareDeviceType::ESP32 => ESP32_CONNECT_TIMEOUT,
        HardwareDeviceType::Ledger => LEDGER_CONNECT_TIMEOUT,
    }
}

fn connect_timeout_message(device_type: &HardwareDeviceType) -> String {
    match device_type {
        HardwareDeviceType::ESP32 => {
            "Connection timed out. Replug your Unruggable First Edition and try again."
                .to_string()
        }
        HardwareDeviceType::Ledger => {
            "Ledger connection timed out. Unlock the device, open the Solana app, close Ledger Live, and try again."
                .to_string()
        }
    }
}

async fn discover_funded_ledger_paths(
    wallet: Arc<HardwareWallet>,
    primary_change: u32,
    rpc_url: &str,
) -> Result<(Vec<LedgerDerivationAddress>, HashMap<String, f64>), String> {
    let mut funded = Vec::<LedgerDerivationAddress>::new();
    let mut balances = HashMap::<String, f64>::new();

    for change in ledger_scan_changes(primary_change) {
        let mut start_account = 0u32;
        while start_account < LEDGER_SCAN_MAX_ACCOUNTS {
            let remaining = LEDGER_SCAN_MAX_ACCOUNTS.saturating_sub(start_account);
            let count = remaining.min(LEDGER_SCAN_BATCH_SIZE);
            if count == 0 {
                break;
            }

            let batch = wallet
                .ledger_discover_derivation_paths(start_account, count, change)
                .await
                .map_err(|err| format!("Failed to scan Ledger paths: {err}"))?;

            if batch.is_empty() {
                break;
            }

            let pubkeys: Vec<String> = batch.iter().map(|entry| entry.pubkey.clone()).collect();
            let batch_balances = rpc::get_balances(&pubkeys, Some(rpc_url))
                .await
                .map_err(|err| format!("Failed to fetch SOL balances: {err}"))?;

            for entry in batch {
                let sol_balance = batch_balances.get(&entry.pubkey).copied().unwrap_or(0.0);
                if sol_balance > 0.0 {
                    balances.insert(entry.pubkey.clone(), sol_balance);
                    funded.push(entry);
                }
            }

            // Advance by scanned account count (not derived address count), because each
            // account can yield multiple path variants.
            start_account = start_account.saturating_add(count);
        }
    }

    funded.sort_by_key(|entry| (entry.change, entry.account));
    Ok((funded, balances))
}

fn extract_hardware_error_code(message: &str) -> Option<String> {
    const MARKER: &str = "Hardware wallet error: ";
    let idx = message.find(MARKER)?;
    let code = &message[idx + MARKER.len()..];
    let code = code
        .split(|c| c == '\n' || c == '\r')
        .next()
        .unwrap_or(code)
        .trim();
    if code.is_empty() {
        None
    } else {
        Some(code.to_string())
    }
}

fn format_connect_unlock_error(err: &str) -> String {
    if let Some(code) = extract_hardware_error_code(err) {
        return match code.as_str() {
            "AUTH_FAILED" | "OTP_BAD_CODE" => "Incorrect code. Please try again.".to_string(),
            "AUTH_LOCKED" => {
                "Too many failed attempts. Device auth is locked. Use physical factory wipe to recover."
                    .to_string()
            }
            "BAD_PIN_FORMAT" | "BAD_OTP_FORMAT" => {
                "Code must be exactly 6 digits.".to_string()
            }
            "BUTTON_TIMEOUT" => {
                "No button press detected. Submit the code again, then press the device button within 8 seconds."
                    .to_string()
            }
            "AUTH_MODE_MISMATCH" => {
                "Unlock method does not match the device mode. Open hardware connect and try again."
                    .to_string()
            }
            "TIME_NOT_SET" => {
                "Device time is not set yet. Try the unlock again.".to_string()
            }
            other => format!("Unlock failed: {other}"),
        };
    }

    format!("Unlock failed: {err}")
}

fn format_setup_error(err: &str) -> String {
    if let Some(code) = extract_hardware_error_code(err) {
        return match code.as_str() {
            "BAD_PIN_FORMAT" => "PIN must be exactly 6 digits.".to_string(),
            "BUTTON_TIMEOUT" => {
                "No button hold detected. Submit the PIN again, then hold the device button for about 2 seconds."
                    .to_string()
            }
            "MODE_FINAL" => {
                "This device is already set up. Reconnect it to continue.".to_string()
            }
            other => format!("Setup failed: {other}"),
        };
    }

    format!("Setup failed: {err}")
}

#[component]
pub fn HardwareWalletModal(
    onclose: EventHandler<()>,
    onsuccess: EventHandler<Arc<HardwareWallet>>,
    ondisconnect: EventHandler<()>,
    existing_wallet: Option<Arc<HardwareWallet>>,
) -> Element {
    let first_edition_lock_art = crate::asset_hosting::app_asset("lock.png");
    let mut connecting = use_signal(|| false);
    let mut connecting_device = use_signal(|| None as Option<HardwareDeviceType>);
    let mut error_message = use_signal(|| None as Option<String>);
    let mut hardware_wallet = use_signal(|| existing_wallet.clone());
    let mut connected = use_signal(|| existing_wallet.is_some());
    let mut public_key = use_signal(|| None as Option<String>);
    let mut device_type = use_signal(|| None as Option<HardwareDeviceType>);
    let mut available_devices = use_signal(|| Vec::<HardwareDeviceInfo>::new());
    let mut scanning = use_signal(|| false);

    // New firmware/setup state
    let mut capability = use_signal(|| None as Option<Esp32Capability>);
    let mut fw_auth_mode = use_signal(|| None as Option<AuthMode>);
    let mut fw_finalized = use_signal(|| None as Option<bool>);
    let mut setup_required = use_signal(|| false);
    let mut setup_busy = use_signal(|| false);
    let mut pin_setup_step = use_signal(|| PinSetupStep::Enter);
    let mut pin_first_entry = use_signal(|| String::new());
    let mut pin_setup_error = use_signal(|| None as Option<String>);
    let mut connect_unlock_required = use_signal(|| false);
    let mut connect_unlock_mode = use_signal(|| None as Option<ConnectUnlockMode>);
    let mut connect_unlock_busy = use_signal(|| false);
    let mut connect_unlock_error = use_signal(|| None as Option<String>);
    let mut connect_unlock_otp_code = use_signal(|| String::new());
    let mut connect_unlock_waiting_button = use_signal(|| false);

    // Ledger derivation-path state
    let mut ledger_accounts = use_signal(|| Vec::<LedgerDerivationAddress>::new());
    let mut ledger_account_balances = use_signal(|| HashMap::<String, f64>::new());
    let mut ledger_accounts_loading = use_signal(|| false);
    let mut ledger_accounts_error = use_signal(|| None as Option<String>);
    let mut ledger_selected_path = use_signal(|| None as Option<String>);
    let mut ledger_path_account = use_signal(|| 0u32);
    let mut ledger_path_change = use_signal(|| 0u32);
    let mut ledger_path_busy = use_signal(|| false);

    let has_existing_wallet = existing_wallet.is_some();

    use_effect(move || {
        if let Some(wallet) = &existing_wallet {
            let wallet = wallet.clone();
            spawn(async move {
                if let Ok(pubkey) = wallet.get_public_key().await {
                    public_key.set(Some(pubkey));
                    connected.set(true);
                }
                if let Some(dev_type) = wallet.get_device_type().await {
                    device_type.set(Some(dev_type.clone()));
                    if dev_type == HardwareDeviceType::ESP32 {
                        capability.set(wallet.get_esp32_capability().await);
                        if let Some(info) = wallet.get_cached_esp32_info().await {
                            fw_auth_mode.set(Some(info.auth_mode));
                            fw_finalized.set(Some(info.finalized));
                            setup_required
                                .set(!info.finalized || info.auth_mode == AuthMode::Unset);
                        }
                        if let Ok(info_opt) = wallet.refresh_esp32_info().await {
                            if let Some(info) = info_opt {
                                fw_auth_mode.set(Some(info.auth_mode));
                                fw_finalized.set(Some(info.finalized));
                                setup_required
                                    .set(!info.finalized || info.auth_mode == AuthMode::Unset);
                            }
                        }
                    } else if dev_type == HardwareDeviceType::Ledger {
                        ledger_accounts_loading.set(true);
                        ledger_accounts_error.set(None);
                        ledger_path_busy.set(false);
                        let (active_account, active_change) = wallet
                            .ledger_get_derivation_indices()
                            .await
                            .unwrap_or((0, 0));
                        let active_path = wallet.ledger_get_derivation_path().await;
                        ledger_path_account.set(active_account);
                        ledger_path_change.set(active_change);

                        let rpc_url = active_rpc_url_for_ledger_scan();
                        match discover_funded_ledger_paths(wallet.clone(), active_change, &rpc_url)
                            .await
                        {
                            Ok((accounts, balances)) => {
                                if accounts.is_empty() {
                                    ledger_accounts_error
                                        .set(Some(no_funded_ledger_paths_message(active_change)));
                                    ledger_selected_path.set(None);
                                } else {
                                    ledger_accounts_error.set(None);
                                    let preferred_path = active_path
                                        .as_ref()
                                        .filter(|path| {
                                            accounts.iter().any(|entry| entry.path == **path)
                                        })
                                        .cloned()
                                        .or_else(|| {
                                            accounts.first().map(|entry| entry.path.clone())
                                        });
                                    ledger_selected_path.set(preferred_path.clone());
                                    if let Some(path) = preferred_path {
                                        if let Some(entry) =
                                            accounts.iter().find(|entry| entry.path == path)
                                        {
                                            ledger_path_account.set(entry.account);
                                            ledger_path_change.set(entry.change);
                                        }
                                    }
                                }
                                ledger_accounts.set(accounts);
                                ledger_account_balances.set(balances);
                            }
                            Err(err) => {
                                ledger_accounts.set(Vec::new());
                                ledger_account_balances.set(HashMap::new());
                                ledger_accounts_error.set(Some(err));
                            }
                        }
                        ledger_accounts_loading.set(false);
                    }
                }
            });
        }
    });

    use_effect(move || {
        if !has_existing_wallet {
            scanning.set(true);
            spawn(async move {
                let devices = HardwareWallet::scan_available_devices().await;
                available_devices.set(devices);
                scanning.set(false);
            });
        }
    });

    let mut rescan_devices = move || {
        if scanning() {
            return;
        }
        scanning.set(true);
        error_message.set(None);
        spawn(async move {
            let devices = HardwareWallet::scan_available_devices().await;
            available_devices.set(devices);
            scanning.set(false);
        });
    };

    let mut connect_device = move |dev_type: HardwareDeviceType| {
        connecting.set(true);
        connecting_device.set(Some(dev_type.clone()));
        error_message.set(None);
        hardware_wallet.set(None);
        connected.set(false);
        public_key.set(None);
        device_type.set(None);
        capability.set(None);
        fw_auth_mode.set(None);
        fw_finalized.set(None);
        setup_required.set(false);
        pin_setup_step.set(PinSetupStep::Enter);
        pin_first_entry.set(String::new());
        pin_setup_error.set(None);
        connect_unlock_required.set(false);
        connect_unlock_mode.set(None);
        connect_unlock_busy.set(false);
        connect_unlock_error.set(None);
        connect_unlock_otp_code.set(String::new());
        connect_unlock_waiting_button.set(false);
        ledger_accounts.set(Vec::new());
        ledger_account_balances.set(HashMap::new());
        ledger_accounts_loading.set(false);
        ledger_accounts_error.set(None);
        ledger_selected_path.set(None);
        ledger_path_account.set(0);
        ledger_path_change.set(0);
        ledger_path_busy.set(false);

        spawn(async move {
            let wallet = Arc::new(HardwareWallet::new());
            let result = match tokio::time::timeout(connect_timeout_for(&dev_type), async {
                match dev_type {
                    HardwareDeviceType::ESP32 => wallet.connect_esp32().await,
                    HardwareDeviceType::Ledger => wallet.connect_ledger().await,
                }
            })
            .await
            {
                Ok(result) => result,
                Err(_) => {
                    let _ = wallet.disconnect().await;
                    error_message.set(Some(connect_timeout_message(&dev_type)));
                    connecting.set(false);
                    connecting_device.set(None);
                    return;
                }
            };

            match result {
                Ok(()) => {
                    let cached_pubkey = wallet.get_cached_public_key().await;
                    let mut needs_setup = false;
                    let mut unlock_mode: Option<ConnectUnlockMode> = None;

                    if dev_type == HardwareDeviceType::ESP32 {
                        capability.set(wallet.get_esp32_capability().await);
                        if let Some(info) = wallet.get_cached_esp32_info().await {
                            fw_auth_mode.set(Some(info.auth_mode));
                            fw_finalized.set(Some(info.finalized));
                            needs_setup = !info.finalized || info.auth_mode == AuthMode::Unset;
                            unlock_mode = match info.auth_mode {
                                AuthMode::Pin => Some(ConnectUnlockMode::Pin),
                                AuthMode::Otp => Some(ConnectUnlockMode::Otp),
                                _ => None,
                            };
                        }
                        if let Ok(info_opt) = wallet.refresh_esp32_info().await {
                            if let Some(info) = info_opt {
                                fw_auth_mode.set(Some(info.auth_mode));
                                fw_finalized.set(Some(info.finalized));
                                needs_setup = !info.finalized || info.auth_mode == AuthMode::Unset;
                                unlock_mode = match info.auth_mode {
                                    AuthMode::Pin => Some(ConnectUnlockMode::Pin),
                                    AuthMode::Otp => Some(ConnectUnlockMode::Otp),
                                    _ => None,
                                };
                            }
                        }
                        setup_required.set(needs_setup);

                        if !needs_setup && cached_pubkey.is_none() {
                            let _ = wallet.disconnect().await;
                            error_message.set(Some(
                                "Device connected but did not return a wallet address. Reconnect and try again."
                                    .to_string(),
                            ));
                            connecting.set(false);
                            connecting_device.set(None);
                            return;
                        }
                    } else if cached_pubkey.is_none() {
                        let _ = wallet.disconnect().await;
                        error_message.set(Some(
                            "Connected device did not return a wallet address. Try again."
                                .to_string(),
                        ));
                        connecting.set(false);
                        connecting_device.set(None);
                        return;
                    }

                    public_key.set(cached_pubkey);
                    device_type.set(Some(dev_type.clone()));
                    hardware_wallet.set(Some(wallet.clone()));
                    connected.set(true);
                    connecting.set(false);
                    connecting_device.set(None);

                    if dev_type == HardwareDeviceType::ESP32 {
                        if needs_setup {
                            return;
                        }

                        if let Some(mode) = unlock_mode {
                            if wallet.has_active_esp32_unlock_session().await {
                                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                                onsuccess.call(wallet);
                                return;
                            }

                            connect_unlock_required.set(true);
                            connect_unlock_mode.set(Some(mode));
                            connect_unlock_busy.set(false);
                            connect_unlock_error.set(None);
                            connect_unlock_otp_code.set(String::new());
                            connect_unlock_waiting_button.set(false);
                            return;
                        }
                    }

                    if dev_type == HardwareDeviceType::Ledger {
                        ledger_accounts_loading.set(true);
                        ledger_accounts_error.set(None);
                        ledger_path_busy.set(false);
                        let (active_account, active_change) = wallet
                            .ledger_get_derivation_indices()
                            .await
                            .unwrap_or((0, 0));
                        let active_path = wallet.ledger_get_derivation_path().await;
                        ledger_path_account.set(active_account);
                        ledger_path_change.set(active_change);

                        let rpc_url = active_rpc_url_for_ledger_scan();
                        match discover_funded_ledger_paths(wallet.clone(), active_change, &rpc_url)
                            .await
                        {
                            Ok((accounts, balances)) => {
                                if accounts.is_empty() {
                                    ledger_accounts_error
                                        .set(Some(no_funded_ledger_paths_message(active_change)));
                                    ledger_selected_path.set(None);
                                } else {
                                    ledger_accounts_error.set(None);
                                    let preferred_path = active_path
                                        .as_ref()
                                        .filter(|path| {
                                            accounts.iter().any(|entry| entry.path == **path)
                                        })
                                        .cloned()
                                        .or_else(|| {
                                            accounts.first().map(|entry| entry.path.clone())
                                        });
                                    ledger_selected_path.set(preferred_path.clone());
                                    if let Some(path) = preferred_path {
                                        if let Some(entry) =
                                            accounts.iter().find(|entry| entry.path == path)
                                        {
                                            ledger_path_account.set(entry.account);
                                            ledger_path_change.set(entry.change);
                                        }
                                    }
                                }
                                ledger_accounts.set(accounts);
                                ledger_account_balances.set(balances);
                            }
                            Err(err) => {
                                ledger_accounts.set(Vec::new());
                                ledger_account_balances.set(HashMap::new());
                                ledger_accounts_error.set(Some(err));
                            }
                        }
                        ledger_accounts_loading.set(false);
                        return;
                    }

                    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                    onsuccess.call(wallet);
                }
                Err(e) => {
                    error_message.set(Some(format!("Failed to connect: {e}")));
                    connecting.set(false);
                    connecting_device.set(None);
                }
            }
        });
    };

    let disconnect_device = move |_| {
        if let Some(wallet) = hardware_wallet() {
            spawn(async move {
                let _ = wallet.disconnect().await;
            });
        }
        hardware_wallet.set(None);
        connected.set(false);
        public_key.set(None);
        device_type.set(None);
        capability.set(None);
        fw_auth_mode.set(None);
        fw_finalized.set(None);
        setup_required.set(false);
        pin_setup_step.set(PinSetupStep::Enter);
        pin_first_entry.set(String::new());
        pin_setup_error.set(None);
        connect_unlock_required.set(false);
        connect_unlock_mode.set(None);
        connect_unlock_busy.set(false);
        connect_unlock_error.set(None);
        connect_unlock_otp_code.set(String::new());
        connect_unlock_waiting_button.set(false);
        ledger_accounts.set(Vec::new());
        ledger_account_balances.set(HashMap::new());
        ledger_accounts_loading.set(false);
        ledger_accounts_error.set(None);
        ledger_selected_path.set(None);
        ledger_path_account.set(0);
        ledger_path_change.set(0);
        ledger_path_busy.set(false);
        ondisconnect.call(());
    };

    let setup_pin_component_key = if pin_setup_step() == PinSetupStep::Enter {
        "hardware-pin-setup-enter"
    } else {
        "hardware-pin-setup-confirm"
    };
    let connect_pin_component_key = "hardware-pin-unlock";
    let hardware_modal_class = if setup_required() {
        "modal-content hardware-modal hardware-modal-immersive"
    } else {
        "modal-content hardware-modal"
    };

    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| onclose.call(()),

            div {
                class: "{hardware_modal_class}",
                onclick: move |e| e.stop_propagation(),

                div {
                    class: "modal-header",
                    h2 { class: "modal-title", "Hardware Wallet" }
                    button {
                        class: "modal-close-button",
                        onclick: move |_| onclose.call(()),
                        "×"
                    }
                }

                div {
                    class: "modal-body",

                    if let Some(error) = error_message() {
                        div {
                            class: "error-message",
                            div { class: "error-icon", "⚠️" }
                            div { class: "error-text", "{error}" }
                        }
                    }

                    if !connected() {
                        div {
                            class: "connection-section hardware-connect-shell",

                            div {
                                class: "info-header hardware-auth-hero",
                                div { class: "hardware-auth-kicker", "Hardware Connect" }
                                h3 { "Connect Your Hardware Wallet" }
                                p {
                                    class: "info-subtitle",
                                    if connecting() {
                                        if connecting_device() == Some(HardwareDeviceType::ESP32) {
                                            "Handshaking with your Unruggable First Edition and establishing a secure connection. First plug-in can take a second."
                                        } else {
                                            "Opening a secure connection to Ledger. Keep the device unlocked with the Solana app open."
                                        }
                                    } else {
                                        "Plug in, tap connect, and unlock only if the device asks for it."
                                    }
                                }
                            }

                            div {
                                class: "connection-steps",
                                div { class: "connection-step-chip", "1. Plug in" }
                                div { class: "connection-step-chip", "2. Connect" }
                                div { class: "connection-step-chip", "3. Unlock" }
                            }

                            if scanning() {
                                div {
                                    class: "scanning-container",
                                    div { class: "scanning-spinner" }
                                    div { class: "scanning-text", "Scanning for devices..." }
                                }
                            } else {
                                if available_devices().is_empty() {
                                    div {
                                        class: "no-devices-container",
                                        div { class: "no-devices-icon", "🔍" }
                                        div { class: "no-devices-title", "No Hardware Wallets Detected" }
                                        div {
                                            class: "no-devices-subtitle",
                                            "Check cable and power, then rescan."
                                        }
                                        div {
                                            class: "no-devices-actions",
                                            button {
                                                class: "connect-device-button rescan-button",
                                                onclick: move |_| rescan_devices(),
                                                "Rescan Devices"
                                            }
                                        }
                                    }
                                } else {
                                    div {
                                        class: "devices-section",
                                        h4 { class: "devices-title", "Nearby Devices" }
                                        div {
                                            class: "devices-grid",
                                            for device in available_devices() {
                                                {
                                                    let is_connecting_this_device =
                                                        connecting()
                                                            && connecting_device()
                                                                == Some(device.device_type.clone());
                                                    rsx! {
                                                        div {
                                                            class: "device-card",
                                                            div {
                                                                class: "device-icon-container",
                                                                img {
                                                                    src: if device.device_type == HardwareDeviceType::ESP32 { ICON_UNRUGGABLE } else { ICON_LEDGER },
                                                                    alt: if device.device_type == HardwareDeviceType::ESP32 { "Unruggable First Edition" } else { "Ledger Hardware Wallet" },
                                                                    width: "48",
                                                                    height: "48"
                                                                }
                                                            }
                                                            div {
                                                                class: "device-info",
                                                                div { class: "device-name", "{device.name}" }
                                                                div {
                                                                    class: if device.device_type == HardwareDeviceType::ESP32 { "device-type-badge unruggable-badge" } else { "device-type-badge ledger-badge" },
                                                                    if device.device_type == HardwareDeviceType::ESP32 { "First Edition" } else { "Ledger" }
                                                                }
                                                            }
                                                            button {
                                                                class: if is_connecting_this_device {
                                                                    "connect-device-button connecting"
                                                                } else {
                                                                    "connect-device-button"
                                                                },
                                                                disabled: connecting(),
                                                                onclick: {
                                                                    let dev_type = device.device_type.clone();
                                                                    move |_| connect_device(dev_type.clone())
                                                                },
                                                                if is_connecting_this_device {
                                                                    span { class: "connect-device-button-spinner", "" }
                                                                    div {
                                                                        class: "connect-device-button-copy",
                                                                        span {
                                                                            class: "connect-device-button-label",
                                                                            "Connecting"
                                                                        }
                                                                        span {
                                                                            class: "connect-device-button-status",
                                                                            if device.device_type == HardwareDeviceType::ESP32 {
                                                                                "Handshaking with device..."
                                                                            } else {
                                                                                "Opening secure channel..."
                                                                            }
                                                                        }
                                                                    }
                                                                } else {
                                                                    "Connect"
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else if setup_required() {
                        div {
                            class: "connected-section hardware-auth-shell hardware-pin-setup-stage",
                            div {
                                class: "flow-hero-media flow-hero-media-hardware",
                                img {
                                    class: "flow-hero-image flow-hero-image-hardware",
                                    src: first_edition_lock_art,
                                    alt: "Unruggable lock artwork"
                                }
                            }

                            div {
                                class: "hardware-auth-hero hardware-auth-hero-compact",
                                div { class: "hardware-auth-kicker", "First-Time Setup" }
                                p {
                                    class: "info-subtitle",
                                    if setup_busy() {
                                        "PIN confirmed. Press and hold the hardware button for about 2 seconds to save it on the device."
                                    } else if pin_setup_step() == PinSetupStep::Enter {
                                        "Create a 6-digit PIN for your Unruggable First Edition."
                                    } else {
                                        "Confirm the same 6-digit PIN, then save it on the device."
                                    }
                                }
                            }

                            div {
                                class: "flow-note-card flow-note-card-hardware",
                                div {
                                    class: "flow-note-label",
                                    "Generated offline on device"
                                }
                                p {
                                    class: "flow-note-copy",
                                    "Your keypair is created locally on the Unruggable First Edition. The private key never leaves the device."
                                }
                            }

                            if setup_busy() {
                                div {
                                    class: "hardware-auth-card hardware-setup-busy-card",
                                    div { class: "pin-processing-spinner" }
                                    h3 { class: "hardware-setup-busy-title", "Save PIN On Device" }
                                    p {
                                        class: "hardware-setup-busy-copy",
                                        "Press and hold the hardware button for about 2 seconds to finish setup."
                                    }
                                }
                            } else if pin_setup_step() == PinSetupStep::Enter {
                                div {
                                    class: "hardware-auth-card hardware-pin-shell hardware-pin-shell-setup",
                                    key: "{setup_pin_component_key}",
                                    PinInput {
                                        title: "Create PIN".to_string(),
                                        subtitle: Some("Choose a 6-digit code for your Unruggable First Edition.".to_string()),
                                        error_message: pin_setup_error(),
                                        on_complete: EventHandler::new(move |pin_value: String| {
                                            if !valid_six_digits(&pin_value) {
                                                pin_setup_error.set(Some("PIN must be exactly 6 digits".to_string()));
                                                return;
                                            }

                                            pin_first_entry.set(pin_value);
                                            pin_setup_step.set(PinSetupStep::Confirm);
                                            pin_setup_error.set(None);
                                        }),
                                        on_cancel: None,
                                        on_input: Some(EventHandler::new(move |_| {
                                            if pin_setup_error().is_some() {
                                                pin_setup_error.set(None);
                                            }
                                        })),
                                        show_strength: Some(false),
                                        step_indicator: Some("Step 1 of 2".to_string()),
                                        clear_on_complete: Some(true),
                                        is_processing: Some(false),
                                        processing_label: None,
                                        reset_key: Some(setup_pin_component_key.to_string()),
                                    }
                                }
                            } else {
                                div {
                                    class: "hardware-auth-card hardware-pin-shell hardware-pin-shell-setup",
                                    key: "{setup_pin_component_key}",
                                    PinInput {
                                        title: "Confirm PIN".to_string(),
                                        subtitle: Some("Enter the same 6-digit PIN again.".to_string()),
                                        error_message: pin_setup_error(),
                                        on_complete: EventHandler::new(move |pin_value: String| {
                                            if !valid_six_digits(&pin_value) {
                                                pin_setup_error.set(Some("PIN must be exactly 6 digits".to_string()));
                                                return;
                                            }

                                            if pin_value != pin_first_entry() {
                                                pin_setup_error.set(Some("PINs don't match. Try again.".to_string()));
                                                pin_setup_step.set(PinSetupStep::Enter);
                                                pin_first_entry.set(String::new());
                                                return;
                                            }

                                            let Some(wallet) = hardware_wallet() else {
                                                pin_setup_error.set(Some("Hardware wallet disconnected".to_string()));
                                                return;
                                            };

                                            setup_busy.set(true);
                                            pin_setup_error.set(None);
                                            error_message.set(None);
                                            let pin_to_set = pin_value.clone();
                                            spawn(async move {
                                                match wallet.setup_mode_pin(&pin_to_set).await {
                                                    Ok(()) => {
                                                        if let Some(pubkey) = wallet.get_cached_public_key().await {
                                                            public_key.set(Some(pubkey));
                                                        }
                                                        match wallet.refresh_esp32_info().await {
                                                            Ok(Some(info)) => {
                                                                fw_auth_mode.set(Some(info.auth_mode));
                                                                fw_finalized.set(Some(info.finalized));
                                                                setup_required
                                                                    .set(!info.finalized || info.auth_mode == AuthMode::Unset);
                                                            }
                                                            _ => {
                                                                fw_auth_mode.set(Some(AuthMode::Pin));
                                                                fw_finalized.set(Some(true));
                                                                setup_required.set(false);
                                                            }
                                                        }
                                                        setup_busy.set(false);
                                                        pin_setup_step.set(PinSetupStep::Enter);
                                                        pin_first_entry.set(String::new());
                                                        pin_setup_error.set(None);
                                                        connect_unlock_required.set(false);
                                                        connect_unlock_mode.set(None);
                                                        connect_unlock_busy.set(false);
                                                        connect_unlock_error.set(None);
                                                        connect_unlock_otp_code.set(String::new());
                                                        connect_unlock_waiting_button.set(false);
                                                        error_message.set(None);
                                                        onsuccess.call(wallet);
                                                    }
                                                    Err(err) => {
                                                        setup_busy.set(false);
                                                        pin_setup_error
                                                            .set(Some(format_setup_error(&err.to_string())));
                                                    }
                                                }
                                            });
                                        }),
                                        on_cancel: None,
                                        on_input: Some(EventHandler::new(move |_| {
                                            if pin_setup_error().is_some() {
                                                pin_setup_error.set(None);
                                            }
                                        })),
                                        show_strength: Some(false),
                                        step_indicator: Some("Step 2 of 2".to_string()),
                                        clear_on_complete: Some(true),
                                        is_processing: Some(false),
                                        processing_label: None,
                                        reset_key: Some(setup_pin_component_key.to_string()),
                                    }
                                }

                                div {
                                    class: "hardware-inline-note",
                                    "After you confirm the PIN, you'll press and hold the device button for about 2 seconds."
                                }
                            }
                        }
                    } else if connect_unlock_required() {
                        match connect_unlock_mode() {
                            Some(ConnectUnlockMode::Pin) => rsx! {
                                div {
                                    class: "hardware-pin-minimal-shell",
                                    div {
                                        class: "hardware-pin-shell hardware-pin-shell-minimal",
                                        key: "{connect_pin_component_key}",
                                        PinInput {
                                            title: "PIN".to_string(),
                                            subtitle: None,
                                            error_message: connect_unlock_error(),
                                            on_complete: EventHandler::new(move |pin_value: String| {
                                                if connect_unlock_busy() {
                                                    return;
                                                }
                                                if !valid_six_digits(&pin_value) {
                                                    connect_unlock_error.set(Some("Code must be exactly 6 digits.".to_string()));
                                                    return;
                                                }

                                                let Some(wallet) = hardware_wallet() else {
                                                    connect_unlock_error.set(Some("Hardware wallet disconnected".to_string()));
                                                    return;
                                                };

                                                connect_unlock_busy.set(true);
                                                connect_unlock_error.set(None);
                                                connect_unlock_waiting_button.set(false);
                                                spawn(async move {
                                                    match wallet.unlock_pin(&pin_value).await {
                                                        Ok(_) => {
                                                            connect_unlock_busy.set(false);
                                                            connect_unlock_required.set(false);
                                                            connect_unlock_mode.set(None);
                                                            connect_unlock_error.set(None);
                                                            connect_unlock_otp_code.set(String::new());
                                                            connect_unlock_waiting_button.set(false);
                                                            onsuccess.call(wallet);
                                                        }
                                                        Err(err) => {
                                                            connect_unlock_busy.set(false);
                                                            connect_unlock_waiting_button.set(false);
                                                            connect_unlock_error.set(Some(format_connect_unlock_error(&err.to_string())));
                                                        }
                                                    }
                                                });
                                            }),
                                            on_cancel: None,
                                            on_input: Some(EventHandler::new(move |_| {
                                                if connect_unlock_error().is_some() {
                                                    connect_unlock_error.set(None);
                                                }
                                            })),
                                            show_strength: Some(false),
                                            step_indicator: None,
                                            clear_on_complete: Some(true),
                                            is_processing: Some(connect_unlock_busy()),
                                            processing_label: Some(if connect_unlock_waiting_button() {
                                                "Confirm on device...".to_string()
                                            } else {
                                                "Unlocking device...".to_string()
                                            }),
                                        }
                                    }
                                }
                            },
                            Some(ConnectUnlockMode::Otp) => rsx! {
                                div {
                                    class: "connected-section hardware-auth-shell",
                                    div {
                                        class: "hardware-auth-hero hardware-auth-hero-compact",
                                        div { class: "hardware-auth-kicker", "Authenticator" }
                                        p {
                                            class: "info-subtitle",
                                            if connect_unlock_waiting_button() {
                                                "Code accepted. Press the hardware button once now."
                                            } else {
                                                "Enter the current 6-digit code. After you submit it, press the hardware button once within 8 seconds to finish unlocking."
                                            }
                                        }
                                    }

                                    if connect_unlock_waiting_button() {
                                        div {
                                            class: "hardware-inline-note hardware-inline-note-strong",
                                            "Press the hardware button once within 8 seconds."
                                        }
                                    }

                                    if let Some(err) = connect_unlock_error() {
                                        div {
                                            class: "error-message",
                                            div { class: "error-icon", "⚠️" }
                                            div { class: "error-text", "{err}" }
                                        }
                                    }

                                    div {
                                        class: "hardware-auth-card hardware-auth-card-otp",
                                        label { "Authenticator Code" }
                                        input {
                                            class: "hardware-code-input",
                                            r#type: "password",
                                            value: "{connect_unlock_otp_code}",
                                            oninput: move |e| {
                                                connect_unlock_otp_code.set(
                                                    e.value()
                                                        .chars()
                                                        .filter(|c| c.is_ascii_digit())
                                                        .take(6)
                                                        .collect()
                                                );
                                                connect_unlock_waiting_button.set(false);
                                            },
                                            placeholder: "6-digit code",
                                            maxlength: "6",
                                            autocomplete: "off",
                                            inputmode: "numeric",
                                            pattern: "[0-9]*",
                                            disabled: connect_unlock_busy()
                                        }
                                    }
                                    div { class: "modal-buttons",
                                        button {
                                            class: "modal-button cancel",
                                            disabled: connect_unlock_busy(),
                                            onclick: move |_| {
                                                if let Some(wallet) = hardware_wallet() {
                                                    spawn(async move {
                                                        let _ = wallet.disconnect().await;
                                                    });
                                                }
                                                hardware_wallet.set(None);
                                                connected.set(false);
                                                public_key.set(None);
                                                device_type.set(None);
                                                capability.set(None);
                                                fw_auth_mode.set(None);
                                                fw_finalized.set(None);
                                                setup_required.set(false);
                                                pin_setup_step.set(PinSetupStep::Enter);
                                                pin_first_entry.set(String::new());
                                                pin_setup_error.set(None);
                                                connect_unlock_required.set(false);
                                                connect_unlock_mode.set(None);
                                                connect_unlock_busy.set(false);
                                                connect_unlock_error.set(None);
                                                connect_unlock_otp_code.set(String::new());
                                                connect_unlock_waiting_button.set(false);
                                                ondisconnect.call(());
                                            },
                                            "Cancel"
                                        }
                                        button {
                                            class: "modal-button primary",
                                            disabled: connect_unlock_busy(),
                                            onclick: move |_| {
                                                if connect_unlock_busy() {
                                                    return;
                                                }
                                                let code = connect_unlock_otp_code();
                                                if !valid_six_digits(&code) {
                                                    connect_unlock_error.set(Some("Code must be exactly 6 digits.".to_string()));
                                                    return;
                                                }

                                                let Some(wallet) = hardware_wallet() else {
                                                    connect_unlock_error.set(Some("Hardware wallet disconnected".to_string()));
                                                    return;
                                                };

                                                connect_unlock_busy.set(true);
                                                connect_unlock_error.set(None);
                                                connect_unlock_waiting_button.set(true);
                                                spawn(async move {
                                                    match wallet.unlock_otp(&code).await {
                                                        Ok(_) => {
                                                            connect_unlock_busy.set(false);
                                                            connect_unlock_required.set(false);
                                                            connect_unlock_mode.set(None);
                                                            connect_unlock_error.set(None);
                                                            connect_unlock_otp_code.set(String::new());
                                                            connect_unlock_waiting_button.set(false);
                                                            onsuccess.call(wallet);
                                                        }
                                                        Err(err) => {
                                                            connect_unlock_busy.set(false);
                                                            connect_unlock_waiting_button.set(false);
                                                            connect_unlock_error.set(Some(format_connect_unlock_error(&err.to_string())));
                                                        }
                                                    }
                                                });
                                            },
                                            if connect_unlock_busy() {
                                                if connect_unlock_waiting_button() {
                                                    "Press Device Button..."
                                                } else {
                                                    "Unlocking..."
                                                }
                                            } else {
                                                "Unlock and Continue"
                                            }
                                        }
                                    }
                                }
                            },
                            None => rsx! {
                                div {
                                    class: "wallet-field",
                                    p { class: "info-subtitle", "Unable to determine unlock method. Open hardware connect again." }
                                }
                            },
                        }
                    } else {
                        div {
                            class: "connected-section",
                            div {
                                class: "success-header",
                                div { class: "success-icon", "✅" }
                                h3 {
                                    if device_type() == Some(HardwareDeviceType::ESP32) {
                                        "Unruggable First Edition Connected"
                                    } else if device_type() == Some(HardwareDeviceType::Ledger) {
                                        "Ledger Connected"
                                    } else {
                                        "Hardware Wallet Connected"
                                    }
                                }
                            }

                            if let Some(dev_type) = device_type() {
                                div {
                                    class: "connected-device-card",
                                    div {
                                        class: "connected-device-icon",
                                        img {
                                            src: if dev_type == HardwareDeviceType::ESP32 { ICON_UNRUGGABLE } else { ICON_LEDGER },
                                            alt: if dev_type == HardwareDeviceType::ESP32 { "Unruggable First Edition" } else { "Ledger Hardware Wallet" },
                                            width: "64",
                                            height: "64"
                                        }
                                    }
                                    div {
                                        class: "connected-device-info",
                                        h4 { class: "connected-device-name", "{dev_type}" }
                                        if let Some(pubkey) = public_key() {
                                            div {
                                                class: "device-pubkey-section",
                                                div { class: "pubkey-label", "Public Key:" }
                                                div { class: "pubkey-display", span { class: "pubkey-text", "{pubkey}" } }
                                            }
                                        }

                                        if dev_type == HardwareDeviceType::Ledger {
                                            div {
                                                class: "wallet-field",
                                                label { "Active Derivation Path" }
                                                div {
                                                    class: "address-display",
                                                    if let Some(path) = ledger_selected_path() {
                                                        "{path}"
                                                    } else {
                                                        "m/44'/501'/{ledger_path_account()}'/{ledger_path_change()}'"
                                                    }
                                                }
                                                p {
                                                    class: "info-subtitle",
                                                    "Only derived addresses with non-zero SOL are listed below."
                                                }
                                                if let Some(pubkey) = public_key() {
                                                    if let Some(sol_balance) =
                                                        ledger_account_balances().get(&pubkey)
                                                    {
                                                        p {
                                                            class: "info-subtitle",
                                                            {format!("Current path balance: {:.6} SOL", sol_balance)}
                                                        }
                                                    }
                                                }
                                            }

                                            if ledger_accounts_loading() {
                                                p {
                                                    class: "info-subtitle",
                                                    "Scanning Ledger account paths and filtering funded addresses..."
                                                }
                                            } else {
                                                if let Some(scan_err) = ledger_accounts_error() {
                                                    div {
                                                        class: "error-message",
                                                        div { class: "error-icon", "⚠️" }
                                                        div { class: "error-text", "{scan_err}" }
                                                    }
                                                }

                                                if !ledger_accounts().is_empty() {
                                                    div {
                                                        class: "wallet-field",
                                                        label { "Funded Ledger Addresses" }
                                                        select {
                                                            value: "{ledger_selected_path().unwrap_or_default()}",
                                                            onchange: move |e| {
                                                                let value = e.value();
                                                                ledger_selected_path.set(Some(value.clone()));
                                                                if let Some(entry) = ledger_accounts()
                                                                    .into_iter()
                                                                    .find(|entry| entry.path == value)
                                                                {
                                                                    ledger_path_account.set(entry.account);
                                                                    ledger_path_change.set(entry.change);
                                                                }
                                                                if ledger_path_busy() || ledger_accounts_loading() {
                                                                    return;
                                                                }
                                                                let Some(wallet) = hardware_wallet() else {
                                                                    ledger_accounts_error.set(Some("Hardware wallet disconnected".to_string()));
                                                                    return;
                                                                };
                                                                ledger_path_busy.set(true);
                                                                ledger_accounts_error.set(None);
                                                                let selected_path = value.clone();
                                                                spawn(async move {
                                                                    match wallet.ledger_set_derivation_path_str(&selected_path).await {
                                                                        Ok(pubkey) => {
                                                                            public_key.set(Some(pubkey));
                                                                            ledger_accounts_error.set(None);
                                                                            ledger_selected_path.set(Some(selected_path));
                                                                        }
                                                                        Err(err) => {
                                                                            ledger_accounts_error.set(Some(format!(
                                                                                "Failed to switch Ledger path: {err}"
                                                                            )));
                                                                        }
                                                                    }
                                                                    ledger_path_busy.set(false);
                                                                });
                                                            },
                                                            for entry in ledger_accounts() {
                                                                option {
                                                                    value: "{entry.path}",
                                                                    {format!(
                                                                        "{} ({:.6} SOL, {})",
                                                                        entry.path,
                                                                        ledger_account_balances()
                                                                            .get(&entry.pubkey)
                                                                            .copied()
                                                                            .unwrap_or(0.0),
                                                                        short_pubkey(&entry.pubkey)
                                                                    )}
                                                                }
                                                            }
                                                        }
                                                    }
                                                }

                                                div {
                                                    class: "wallet-field",
                                                    label { "Custom Path Indices" }
                                                    div { class: "connection-steps",
                                                        input {
                                                            r#type: "number",
                                                            min: "0",
                                                            value: "{ledger_path_account()}",
                                                            oninput: move |e| {
                                                                if let Ok(v) = e.value().parse::<u32>() {
                                                                    ledger_selected_path.set(None);
                                                                    ledger_path_account.set(v);
                                                                }
                                                            },
                                                            placeholder: "Account index"
                                                        }
                                                        input {
                                                            r#type: "number",
                                                            min: "0",
                                                            value: "{ledger_path_change()}",
                                                            oninput: move |e| {
                                                                if let Ok(v) = e.value().parse::<u32>() {
                                                                    ledger_selected_path.set(None);
                                                                    ledger_path_change.set(v);
                                                                }
                                                            },
                                                            placeholder: "Change index"
                                                        }
                                                    }
                                                }

                                                div { class: "modal-buttons",
                                                    button {
                                                        class: "modal-button cancel",
                                                        disabled: ledger_accounts_loading() || ledger_path_busy(),
                                                        onclick: move |_| {
                                                            let Some(wallet) = hardware_wallet() else {
                                                                ledger_accounts_error.set(Some("Hardware wallet disconnected".to_string()));
                                                                return;
                                                            };
                                                            let change = ledger_path_change();
                                                            let selected_account = ledger_path_account();
                                                            let selected_path = ledger_selected_path();
                                                            ledger_accounts_loading.set(true);
                                                            ledger_accounts_error.set(None);
                                                            spawn(async move {
                                                                let rpc_url = active_rpc_url_for_ledger_scan();
                                                                match discover_funded_ledger_paths(wallet.clone(), change, &rpc_url).await {
                                                                    Ok((accounts, balances)) => {
                                                                        if accounts.is_empty() {
                                                                            ledger_accounts_error
                                                                                .set(Some(no_funded_ledger_paths_message(change)));
                                                                            ledger_selected_path.set(None);
                                                                        } else {
                                                                            ledger_accounts_error.set(None);
                                                                            let preferred_path = selected_path
                                                                                .as_ref()
                                                                                .filter(|path| accounts.iter().any(|entry| entry.path == **path))
                                                                                .cloned()
                                                                                .or_else(|| {
                                                                                    accounts
                                                                                        .iter()
                                                                                        .find(|entry| {
                                                                                            entry.account == selected_account
                                                                                                && entry.change == change
                                                                                        })
                                                                                        .map(|entry| entry.path.clone())
                                                                                })
                                                                                .or_else(|| accounts.first().map(|entry| entry.path.clone()));
                                                                            ledger_selected_path.set(preferred_path.clone());
                                                                            if let Some(path) = preferred_path {
                                                                                if let Some(entry) =
                                                                                    accounts.iter().find(|entry| entry.path == path)
                                                                                {
                                                                                    ledger_path_account.set(entry.account);
                                                                                    ledger_path_change.set(entry.change);
                                                                                }
                                                                            }
                                                                        }
                                                                        ledger_accounts.set(accounts);
                                                                        ledger_account_balances.set(balances);
                                                                    }
                                                                    Err(err) => {
                                                                        ledger_accounts.set(Vec::new());
                                                                        ledger_account_balances.set(HashMap::new());
                                                                        ledger_accounts_error.set(Some(err));
                                                                    }
                                                                }
                                                                ledger_accounts_loading.set(false);
                                                            });
                                                        },
                                                        "Rescan"
                                                    }
                                                    button {
                                                        class: "modal-button primary",
                                                        disabled: ledger_accounts_loading() || ledger_path_busy(),
                                                        onclick: move |_| {
                                                            if ledger_path_busy() {
                                                                return;
                                                            }
                                                            let Some(wallet) = hardware_wallet() else {
                                                                ledger_accounts_error.set(Some("Hardware wallet disconnected".to_string()));
                                                                return;
                                                            };
                                                            let account = ledger_path_account();
                                                            let change = ledger_path_change();
                                                            let selected_path = ledger_selected_path();
                                                            ledger_path_busy.set(true);
                                                            ledger_accounts_error.set(None);
                                                            spawn(async move {
                                                                let result = if let Some(path) = selected_path {
                                                                    wallet.ledger_set_derivation_path_str(&path).await
                                                                } else {
                                                                    wallet.ledger_set_derivation_path(account, change).await
                                                                };
                                                                match result {
                                                                    Ok(pubkey) => {
                                                                        public_key.set(Some(pubkey));
                                                                        ledger_accounts_error.set(None);
                                                                        ledger_selected_path
                                                                            .set(wallet.ledger_get_derivation_path().await);
                                                                    }
                                                                    Err(err) => {
                                                                        ledger_accounts_error.set(Some(format!(
                                                                            "Failed to set derivation path: {err}"
                                                                        )));
                                                                    }
                                                                }
                                                                ledger_path_busy.set(false);
                                                            });
                                                        },
                                                        if ledger_path_busy() { "Applying..." } else { "Apply Path" }
                                                    }
                                                }
                                            }
                                        }

                                        if dev_type == HardwareDeviceType::ESP32 {
                                            div {
                                                class: "connection-status",
                                                if let Some(cap) = capability() {
                                                    span {
                                                        if cap == Esp32Capability::NewV1 { "Firmware: NewV1" } else { "Firmware: LegacyV0" }
                                                    }
                                                }
                                            }
                                            div {
                                                class: "connection-status",
                                                if let Some(mode) = fw_auth_mode() {
                                                    span { "Auth mode: {format_auth_mode(mode)}" }
                                                }
                                                if let Some(finalized) = fw_finalized() {
                                                    span {
                                                        " | Finalized: ",
                                                        if finalized { "yes" } else { "no" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            div {
                                class: "connected-modal-actions",
                                if device_type() == Some(HardwareDeviceType::Ledger) {
                                    button {
                                        class: "connect-device-button",
                                        disabled: ledger_path_busy() || ledger_accounts_loading(),
                                        onclick: move |_| {
                                            if let Some(wallet) = hardware_wallet() {
                                                onsuccess.call(wallet);
                                            } else {
                                                ledger_accounts_error
                                                    .set(Some("Hardware wallet disconnected".to_string()));
                                            }
                                        },
                                        "Use Selected Address"
                                    }
                                }
                                button {
                                    class: "connect-device-button",
                                    onclick: disconnect_device,
                                    "Disconnect Device"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
