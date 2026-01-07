use dioxus::prelude::*;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use solana_sdk::signature::Keypair;
use solana_winternitz::privkey::WinternitzPrivkey;
use std::sync::Arc;
use base64::Engine;

use crate::hardware::HardwareWallet;
use crate::quantum_vault::{QuantumVaultClient, StoredVault};
use crate::rpc;
use crate::storage::{
    load_quantum_vaults_from_storage,
    mark_quantum_vault_as_used,
    save_quantum_vault_to_storage,
};
use crate::wallet::WalletInfo;
use super::send_modal::TransactionSuccessModal;

const DEFAULT_RPC_URL: &str = "https://johna-k3cr1v-fast-mainnet.helius-rpc.com";
const WINTERNITZ_PRIVKEY_LEN: usize = 896;

#[derive(Debug, Clone, PartialEq)]
enum ModalView {
    MyVaults,
    Create,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum VaultPickerMode {
    All,
    SplitOnly,
}

fn wallet_to_keypair(wallet_info: &WalletInfo) -> Result<Keypair, String> {
    let wallet = crate::wallet::Wallet::from_wallet_info(wallet_info)?;
    let keypair_b58 = wallet.get_private_key();
    Ok(Keypair::from_base58_string(&keypair_b58))
}

fn decode_vault_privkey(encoded: &str) -> Result<WinternitzPrivkey, String> {
    let cleaned = encoded.trim().trim_matches('"');
    if cleaned.is_empty() {
        return Err("Vault private key is missing. Only original vaults can be split.".to_string());
    }

    let cleaned = cleaned.strip_prefix("base64:").unwrap_or(cleaned);
    let cleaned = cleaned.replace('\n', "").replace('\r', "");

    let try_bytes = |bytes: Vec<u8>| -> Result<WinternitzPrivkey, String> {
        if bytes.len() != WINTERNITZ_PRIVKEY_LEN {
            return Err(format!(
                "Invalid vault key length (expected {}, got {})",
                WINTERNITZ_PRIVKEY_LEN,
                bytes.len()
            ));
        }
        let mut privkey_array = [0u8; WINTERNITZ_PRIVKEY_LEN];
        privkey_array.copy_from_slice(&bytes);
        Ok(WinternitzPrivkey::from(privkey_array))
    };

    let b64_variants = [
        base64::engine::general_purpose::STANDARD,
        base64::engine::general_purpose::STANDARD_NO_PAD,
        base64::engine::general_purpose::URL_SAFE,
        base64::engine::general_purpose::URL_SAFE_NO_PAD,
    ];
    for engine in b64_variants {
        if let Ok(bytes) = engine.decode(cleaned.as_bytes()) {
            if let Ok(key) = try_bytes(bytes) {
                return Ok(key);
            }
        }
    }

    if let Ok(bytes) = bs58::decode(cleaned.as_bytes()).into_vec() {
        if let Ok(key) = try_bytes(bytes) {
            return Ok(key);
        }
    }

    if let Ok(bytes) = hex::decode(cleaned.as_bytes()) {
        if let Ok(key) = try_bytes(bytes) {
            return Ok(key);
        }
    }

    if let Ok(bytes) = serde_json::from_str::<Vec<u8>>(&cleaned) {
        if let Ok(key) = try_bytes(bytes) {
            return Ok(key);
        }
    }

    Err("Invalid vault private key encoding. Use an original vault that includes its private key.".to_string())
}

#[component]
pub fn QuantumVaultModal(
    wallet: Option<WalletInfo>,
    hardware_wallet: Option<Arc<HardwareWallet>>,
    custom_rpc: Option<String>,
    onclose: EventHandler<()>,
) -> Element {
    let _ = hardware_wallet;
    let mut current_view = use_signal(|| ModalView::MyVaults);
    let mut processing = use_signal(|| false);
    let mut processing_action = use_signal(|| "".to_string());
    let mut error_message = use_signal(|| None::<String>);
    let mut status_message = use_signal(|| None::<String>);
    let mut wallet_balance = use_signal(|| None::<f64>);
    let mut wallet_balance_loading = use_signal(|| false);
    let mut show_success_modal = use_signal(|| false);
    let mut success_signature = use_signal(|| "".to_string());

    let mut my_vaults = use_signal(|| Vec::<StoredVault>::new());
    let mut vault_balances = use_signal(|| std::collections::HashMap::<String, f64>::new());
    let mut loading_balances = use_signal(|| false);
    let mut reload_balances_trigger = use_signal(|| 0u32);

    let mut selected_vault = use_signal(|| "".to_string());
    let mut deposit_amount = use_signal(|| "".to_string());
    let mut split_amount = use_signal(|| "".to_string());
    let mut show_vault_picker = use_signal(|| false);
    let mut vault_search_query = use_signal(|| "".to_string());
    let mut vault_picker_mode = use_signal(|| VaultPickerMode::All);

    let rpc_url = custom_rpc.unwrap_or_else(|| DEFAULT_RPC_URL.to_string());

    let rpc_for_balances = rpc_url.clone();
    let rpc_for_create = rpc_url.clone();
    let rpc_for_actions = rpc_url.clone();
    let wallet_for_actions = wallet.clone();
    let rpc_for_wallet_balance = rpc_url.clone();
    let wallet_for_balance = wallet.clone();

    use_effect(move || {
        let wallet_info = wallet_for_balance.clone();
        let rpc_url = rpc_for_wallet_balance.clone();
        if let Some(info) = wallet_info {
            let address = info.address.clone();
            wallet_balance_loading.set(true);
            wallet_balance.set(None);
            spawn(async move {
                match rpc::get_balance(&address, Some(rpc_url.as_str())).await {
                    Ok(balance) => wallet_balance.set(Some(balance)),
                    Err(_) => wallet_balance.set(None),
                }
                wallet_balance_loading.set(false);
            });
        } else {
            wallet_balance.set(None);
            wallet_balance_loading.set(false);
        }
    });

    if show_success_modal() && !success_signature().is_empty() {
        return rsx! {
            TransactionSuccessModal {
                signature: success_signature(),
                was_hardware_wallet: false,
                onclose: move |_| show_success_modal.set(false),
            }
        };
    }

    use_effect(move || {
        my_vaults.set(load_quantum_vaults_from_storage());
    });

    use_effect(move || {
        let _ = reload_balances_trigger();
        let vaults = my_vaults().clone();
        let rpc_clone = rpc_for_balances.clone();

        if vaults.is_empty() {
            vault_balances.set(std::collections::HashMap::new());
            return;
        }

        spawn(async move {
            loading_balances.set(true);
            let client = match QuantumVaultClient::new(Some(&rpc_clone)) {
                Ok(c) => c,
                Err(_) => {
                    loading_balances.set(false);
                    return;
                }
            };

            let mut balances = std::collections::HashMap::new();
            for vault in vaults {
                let vault_pubkey = match bs58::decode(&vault.address).into_vec() {
                    Ok(bytes) if bytes.len() == 32 => {
                        let mut arr = [0u8; 32];
                        arr.copy_from_slice(&bytes);
                        solana_sdk::pubkey::Pubkey::new_from_array(arr)
                    }
                    _ => continue,
                };

                let hash_bytes = match hex::decode(&vault.pubkey_hash) {
                    Ok(bytes) if bytes.len() == 32 => {
                        let mut arr = [0u8; 32];
                        arr.copy_from_slice(&bytes);
                        arr
                    }
                    _ => continue,
                };

                if let Ok(info) = client.get_vault_info(&vault_pubkey, hash_bytes, vault.bump) {
                    balances.insert(vault.address.clone(), info.balance_sol());
                }
            }

            vault_balances.set(balances);
            loading_balances.set(false);
        });
    });

    let handle_create_vault = move |_| {
        processing.set(true);
        processing_action.set("create".to_string());
        error_message.set(None);
        status_message.set(Some("Creating quantum vault...".to_string()));
        let rpc_url_create = rpc_for_create.clone();
        let wallet_create = wallet.clone();
        spawn(async move {
            let client = match QuantumVaultClient::new(Some(&rpc_url_create)) {
                Ok(c) => c,
                Err(e) => {
                    error_message.set(Some(format!("Failed to init client: {}", e)));
                    processing.set(false);
                    processing_action.set("".to_string());
                    return;
                }
            };

            let (privkey, vault_address, bump, pubkey_hash) = client.generate_new_vault();

            let wallet_info = match &wallet_create {
                Some(info) => info,
                None => {
                    error_message.set(Some("No wallet connected".to_string()));
                    processing.set(false);
                    processing_action.set("".to_string());
                    return;
                }
            };

            let keypair = match wallet_to_keypair(wallet_info) {
                Ok(kp) => kp,
                Err(e) => {
                    error_message.set(Some(format!("Failed to load wallet: {}", e)));
                    processing.set(false);
                    processing_action.set("".to_string());
                    return;
                }
            };

            match client.create_vault(&keypair, &pubkey_hash, bump).await {
                Ok(signature) => {
                    let privkey_bytes: [u8; 896] = unsafe {
                        std::mem::transmute::<WinternitzPrivkey, [u8; 896]>(privkey)
                    };

                    let stored_vault = StoredVault {
                        name: format!("Quantum {}", vault_address.to_string().chars().take(8).collect::<String>()),
                        address: vault_address.to_string(),
                        pubkey_hash: hex::encode(pubkey_hash),
                        private_key: base64::encode(&privkey_bytes),
                        bump,
                        created_at: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64,
                        used: false,
                    };

                    save_quantum_vault_to_storage(&stored_vault);
                    my_vaults.set(load_quantum_vaults_from_storage());
                    selected_vault.set(vault_address.to_string());
                    reload_balances_trigger.set(reload_balances_trigger() + 1);
                    status_message.set(Some(format!("Vault created: {}", signature)));
                    success_signature.set(signature);
                    show_success_modal.set(true);
                    current_view.set(ModalView::MyVaults);
                }
                Err(e) => {
                    error_message.set(Some(format!("Failed to create vault: {}", e)));
                }
            }

            processing.set(false);
            processing_action.set("".to_string());
        });
    };

    let rpc_for_actions_deposit = rpc_for_actions.clone();
    let wallet_for_actions_deposit = wallet_for_actions.clone();
    let handle_deposit = move |_| {
            processing.set(true);
            processing_action.set("deposit".to_string());
            error_message.set(None);
            status_message.set(Some("Preparing deposit...".to_string()));
        let rpc_url_deposit = rpc_for_actions_deposit.clone();
        let wallet_deposit = wallet_for_actions_deposit.clone();
        spawn(async move {
            let vault_addr = selected_vault();
            if vault_addr.is_empty() {
                error_message.set(Some("Select a vault first".to_string()));
                processing.set(false);
                processing_action.set("".to_string());
                return;
            }

            let amount_sol: f64 = match deposit_amount().parse() {
                Ok(val) if val > 0.0 => val,
                _ => {
                    error_message.set(Some("Invalid amount".to_string()));
                    processing.set(false);
                    processing_action.set("".to_string());
                    return;
                }
            };

            let amount_lamports = (amount_sol * LAMPORTS_PER_SOL as f64) as u64;

            let client = match QuantumVaultClient::new(Some(&rpc_url_deposit)) {
                Ok(c) => c,
                Err(e) => {
                    error_message.set(Some(format!("Failed to init client: {}", e)));
                    processing.set(false);
                    processing_action.set("".to_string());
                    return;
                }
            };

            let wallet_info = match &wallet_deposit {
                Some(info) => info,
                None => {
                    error_message.set(Some("No wallet connected".to_string()));
                    processing.set(false);
                    processing_action.set("".to_string());
                    return;
                }
            };

            let keypair = match wallet_to_keypair(wallet_info) {
                Ok(kp) => kp,
                Err(e) => {
                    error_message.set(Some(format!("Failed to load wallet: {}", e)));
                    processing.set(false);
                    return;
                }
            };

            let vault_pubkey = match bs58::decode(&vault_addr).into_vec() {
                Ok(bytes) if bytes.len() == 32 => {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&bytes);
                    solana_sdk::pubkey::Pubkey::new_from_array(arr)
                }
                _ => {
                    error_message.set(Some("Invalid vault address".to_string()));
                    processing.set(false);
                    return;
                }
            };

            status_message.set(Some("Submitting deposit transaction...".to_string()));
            match client.deposit_to_vault(&keypair, &vault_pubkey, amount_lamports).await {
                Ok(signature) => {
                    status_message.set(Some(format!("Deposit confirmed: {}", signature)));
                    deposit_amount.set("".to_string());
                    reload_balances_trigger.set(reload_balances_trigger() + 1);
                    success_signature.set(signature);
                    show_success_modal.set(true);
                }
                Err(e) => {
                    error_message.set(Some(format!("Deposit failed: {}", e)));
                }
            }

            processing.set(false);
            processing_action.set("".to_string());
        });
    };

    let rpc_for_actions_split = rpc_for_actions.clone();
    let wallet_for_actions_split = wallet_for_actions.clone();
    let handle_split = move |_| {
            processing.set(true);
            processing_action.set("split".to_string());
            error_message.set(None);
            status_message.set(Some("Preparing split...".to_string()));
        let rpc_url_split = rpc_for_actions_split.clone();
        let wallet_split = wallet_for_actions_split.clone();
        spawn(async move {
            let vault_addr = selected_vault();
            if vault_addr.is_empty() {
                error_message.set(Some("Select a vault first".to_string()));
                processing.set(false);
                processing_action.set("".to_string());
                return;
            }

            let vault_data = match my_vaults().iter().find(|v| v.address == vault_addr) {
                Some(v) => v.clone(),
                None => {
                    error_message.set(Some("Vault not found".to_string()));
                    processing.set(false);
                    processing_action.set("".to_string());
                    return;
                }
            };

            if vault_data.used {
                error_message.set(Some("Vault already used".to_string()));
                processing.set(false);
                processing_action.set("".to_string());
                return;
            }
            if vault_data.private_key.trim().is_empty() {
                error_message.set(Some("This vault has no private key. Use the original vault created before splitting.".to_string()));
                processing.set(false);
                processing_action.set("".to_string());
                return;
            }

            let amount_sol: f64 = match split_amount().parse() {
                Ok(val) if val > 0.0 => val,
                _ => {
                    error_message.set(Some("Invalid amount".to_string()));
                    processing.set(false);
                    processing_action.set("".to_string());
                    return;
                }
            };

            let amount_lamports = (amount_sol * LAMPORTS_PER_SOL as f64) as u64;

            let vault_privkey = match decode_vault_privkey(&vault_data.private_key) {
                Ok(key) => key,
                Err(_) => {
                    error_message.set(Some("Vault private key is invalid. Use the original vault created before splitting.".to_string()));
                    processing.set(false);
                    processing_action.set("".to_string());
                    return;
                }
            };

            let client = match QuantumVaultClient::new(Some(&rpc_url_split)) {
                Ok(c) => c,
                Err(e) => {
                    error_message.set(Some(format!("Failed to init client: {}", e)));
                    processing.set(false);
                    processing_action.set("".to_string());
                    return;
                }
            };

            let wallet_info = match &wallet_split {
                Some(info) => info,
                None => {
                    error_message.set(Some("No wallet connected".to_string()));
                    processing.set(false);
                    return;
                }
            };

            let keypair = match wallet_to_keypair(wallet_info) {
                Ok(kp) => kp,
                Err(e) => {
                    error_message.set(Some(format!("Failed to load wallet: {}", e)));
                    processing.set(false);
                    return;
                }
            };

            let vault_pubkey = match bs58::decode(&vault_addr).into_vec() {
                Ok(bytes) if bytes.len() == 32 => {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&bytes);
                    solana_sdk::pubkey::Pubkey::new_from_array(arr)
                }
                _ => {
                    error_message.set(Some("Invalid vault address".to_string()));
                    processing.set(false);
                    return;
                }
            };

            let (_, split_vault_address, split_bump, split_pubkey_hash) = client.generate_new_vault();
            let (_, refund_vault_address, refund_bump, refund_pubkey_hash) = client.generate_new_vault();

            status_message.set(Some("Creating split vault...".to_string()));
            if let Err(e) = client.create_vault(&keypair, &split_pubkey_hash, split_bump).await {
                error_message.set(Some(format!("Failed to create split vault: {}", e)));
                processing.set(false);
                processing_action.set("".to_string());
                return;
            }

            status_message.set(Some("Creating refund vault...".to_string()));
            if let Err(e) = client.create_vault(&keypair, &refund_pubkey_hash, refund_bump).await {
                error_message.set(Some(format!("Failed to create refund vault: {}", e)));
                processing.set(false);
                processing_action.set("".to_string());
                return;
            }

            status_message.set(Some("Submitting split transaction...".to_string()));
            match client
                .split_vault(
                    &keypair,
                    &vault_privkey,
                    &vault_pubkey,
                    &split_vault_address,
                    &refund_vault_address,
                    amount_lamports,
                    vault_data.bump,
                )
                .await
            {
                Ok(result) => {
                    mark_quantum_vault_as_used(&vault_addr);

                    let split_vault = StoredVault {
                        name: format!("Split {}", split_vault_address.to_string().chars().take(8).collect::<String>()),
                        address: split_vault_address.to_string(),
                        pubkey_hash: hex::encode(split_pubkey_hash),
                        private_key: "".to_string(),
                        bump: split_bump,
                        created_at: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64,
                        used: true,
                    };
                    save_quantum_vault_to_storage(&split_vault);

                    let refund_vault = StoredVault {
                        name: format!("Refund {}", refund_vault_address.to_string().chars().take(8).collect::<String>()),
                        address: refund_vault_address.to_string(),
                        pubkey_hash: hex::encode(refund_pubkey_hash),
                        private_key: "".to_string(),
                        bump: refund_bump,
                        created_at: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64,
                        used: true,
                    };
                    save_quantum_vault_to_storage(&refund_vault);

                    my_vaults.set(load_quantum_vaults_from_storage());
                    reload_balances_trigger.set(reload_balances_trigger() + 1);
                    split_amount.set("".to_string());
                    status_message.set(Some(format!("Split confirmed: {}", result.transaction_signature)));
                    success_signature.set(result.transaction_signature);
                    show_success_modal.set(true);
                }
                Err(e) => {
                    error_message.set(Some(format!("Split failed: {}", e)));
                }
            }

            processing.set(false);
            processing_action.set("".to_string());
        });
    };

    let vault_balances_snapshot = vault_balances();
    let get_vault_balance = |address: &str| vault_balances_snapshot.get(address).copied();
    let refresh_label = if loading_balances() { "Refreshing..." } else { "Refresh" };
    let vault_balance_text = |address: &str| {
        match get_vault_balance(address) {
            Some(bal) => format!("{bal:.4} SOL"),
            None => "...".to_string(),
        }
    };
    let selected_balance_text = if selected_vault().is_empty() {
        "--".to_string()
    } else {
        vault_balance_text(&selected_vault())
    };
    let selected_balance_value = get_vault_balance(&selected_vault());
    let short_address = |address: &str| {
        if address.len() > 10 {
            format!("{}...{}", &address[..4], &address[address.len() - 4..])
        } else {
            address.to_string()
        }
    };
    let vault_is_empty = |vault: &StoredVault| {
        get_vault_balance(&vault.address)
            .map(|balance| balance <= 0.0)
            .unwrap_or(false)
    };
    let vault_status_text = |vault: &StoredVault| {
        if vault.used {
            "Used"
        } else if vault_is_empty(vault) {
            "Empty"
        } else {
            "Active"
        }
    };
    let vault_split_label = |vault: &StoredVault| {
        if vault.used {
            "Split used"
        } else if vault.private_key.trim().is_empty() {
            "No key"
        } else if decode_vault_privkey(&vault.private_key).is_ok() {
            "Splittable"
        } else {
            "Key invalid"
        }
    };
    let selected_address = selected_vault();
    let display_vaults: Vec<StoredVault> = my_vaults()
        .into_iter()
        .filter(|vault| {
            if vault.address == selected_address {
                return true;
            }
            if vault.used {
                return true;
            }
            match get_vault_balance(&vault.address) {
                Some(balance) => balance > 0.0,
                None => true,
            }
        })
        .collect();
    let selected_vault_used = my_vaults()
        .iter()
        .find(|vault| vault.address == selected_vault())
        .map(|vault| vault.used)
        .unwrap_or(false);
    let selected_vault_has_privkey = my_vaults()
        .iter()
        .find(|vault| vault.address == selected_vault())
        .map(|vault| !vault.private_key.trim().is_empty())
        .unwrap_or(false);
    let selected_vault_can_split = my_vaults()
        .iter()
        .find(|vault| vault.address == selected_vault())
        .map(|vault| decode_vault_privkey(&vault.private_key).is_ok())
        .unwrap_or(false);
    let can_show_vault = |vault: &StoredVault| {
        if vault_picker_mode() == VaultPickerMode::SplitOnly {
            return !vault.used && decode_vault_privkey(&vault.private_key).is_ok();
        }
        true
    };
    let selected_vault_zero = selected_balance_value
        .map(|balance| balance <= 0.0)
        .unwrap_or(false);
    let wallet_balance_text = if wallet_balance_loading() {
        "Wallet SOL: …".to_string()
    } else if let Some(balance) = wallet_balance() {
        format!("Wallet SOL: {:.4}", balance)
    } else {
        "Wallet SOL: --".to_string()
    };

    use_effect(move || {
        if !selected_vault().is_empty() && selected_vault_used {
            selected_vault.set("".to_string());
        }
    });

    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| onclose.call(()),

                div {
                    class: "modal-content",
                    style: "max-width: 720px; position: relative;",
                    onclick: move |e| e.stop_propagation(),

                div {
                    style: "display: flex; justify-content: space-between; align-items: center; padding: 20px 24px; border-bottom: 1px solid rgba(255,255,255,0.1);",
                    h2 { style: "color: #f8fafc; font-size: 20px; margin: 0;", "Quantum Vault" }
                    button {
                        style: "background: none; border: none; color: white; font-size: 28px; cursor: pointer;",
                        onclick: move |_| onclose.call(()),
                        "×"
                    }
                }

                if let Some(error) = error_message() {
                    div {
                        style: "background: rgba(239,68,68,0.1); border: 1px solid rgba(239,68,68,0.2); color: #f87171; padding: 10px 24px; margin: 16px 24px 0; border-radius: 8px; font-size: 13px;",
                        "{error}"
                    }
                }

                div {
                    style: "display: flex; gap: 8px; padding: 16px 24px 0;",
                    button {
                        class: if current_view() == ModalView::MyVaults { "button-standard primary" } else { "button-standard ghost" },
                        style: "flex: 1;",
                        onclick: move |_| current_view.set(ModalView::MyVaults),
                        "My Vaults"
                    }
                    button {
                        class: if current_view() == ModalView::Create { "button-standard primary" } else { "button-standard ghost" },
                        style: "flex: 1;",
                        onclick: move |_| current_view.set(ModalView::Create),
                        "Create"
                    }
                }

                div {
                    style: "padding: 20px 24px 24px; max-height: 520px; overflow-y: auto;",

                    match current_view() {
                        ModalView::MyVaults => rsx! {
                            if display_vaults.is_empty() {
                                div {
                                    style: "text-align: center; padding: 40px 0;",
                                    div { style: "font-size: 36px; margin-bottom: 12px;", "🔐" }
                                    div { style: "color: rgba(255,255,255,0.7);", "No quantum vaults yet." }
                                    button {
                                        class: "button-standard primary",
                                        style: "margin-top: 16px;",
                                        onclick: move |_| current_view.set(ModalView::Create),
                                        "Create Vault"
                                    }
                                }
                            } else {
                                div {
                                    style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px;",
                                    div { style: "color: rgba(255,255,255,0.8);", "Vaults ({display_vaults.len()})" }
                                    button {
                                        class: "button-standard ghost",
                                        style: "padding: 6px 12px;",
                                        disabled: loading_balances(),
                                        onclick: move |_| reload_balances_trigger.set(reload_balances_trigger() + 1),
                                        "{refresh_label}"
                                    }
                                }

                                div {
                                    style: "
                                        display: flex;
                                        justify-content: space-between;
                                        align-items: center;
                                        background: #1a1a1a;
                                        border: 1.5px solid #4a4a4a;
                                        border-radius: 12px;
                                        padding: 16px;
                                        margin-bottom: 16px;
                                        gap: 12px;
                                    ",
                                    div {
                                        style: "display: flex; flex-direction: column; gap: 6px; min-width: 0;",
                                        span { style: "color: #94a3b8; font-size: 13px;", "Selected vault" }
                                        if selected_vault().is_empty() {
                                            span { style: "color: rgba(255,255,255,0.7); font-size: 13px;", "No vault selected" }
                                        } else {
                                            span { style: "color: #f8fafc; font-weight: 600; font-size: 14px;", "{selected_balance_text}" }
                                            span { style: "color: rgba(255,255,255,0.55); font-size: 12px;", "{short_address(&selected_vault())}" }
                                            if selected_vault_used {
                                                span { style: "color: #f59e0b; font-size: 11px;", "Used vault • split already completed" }
                                            } else if selected_vault_zero {
                                                span { style: "color: rgba(255,255,255,0.5); font-size: 11px;", "Empty vault • deposit to fund" }
                                            }
                                        }
                                    }
                                    button {
                                        class: "button-standard ghost",
                                        style: "padding: 8px 12px; white-space: nowrap;",
                                        onclick: move |_| {
                                            vault_picker_mode.set(VaultPickerMode::All);
                                            vault_search_query.set("".to_string());
                                            show_vault_picker.set(true);
                                        },
                                        if selected_vault().is_empty() { "Select Vault ▾" } else { "Change ▾" }
                                    }
                                }

                                if show_vault_picker() {
                                    div {
                                        style: "border: 1px solid rgba(255,255,255,0.08); border-radius: 12px; padding: 12px; margin-bottom: 16px; background: rgba(0,0,0,0.25);",
                                        if vault_picker_mode() == VaultPickerMode::SplitOnly {
                                            div { style: "font-size: 12px; color: rgba(255,255,255,0.6); margin-bottom: 8px;",
                                                "Choose a splittable vault"
                                            }
                                        }
                                        input {
                                            r#type: "text",
                                            class: "input-standard",
                                            placeholder: "Search vault address",
                                            value: "{vault_search_query()}",
                                            oninput: move |e| vault_search_query.set(e.value()),
                                        }
                                        div { style: "margin-top: 10px; display: grid; gap: 8px; max-height: 220px; overflow-y: auto;" }
                                        for vault in display_vaults.clone() {
                                            if (vault.address.to_lowercase().contains(&vault_search_query().to_lowercase()) || vault.name.to_lowercase().contains(&vault_search_query().to_lowercase()))
                                                && can_show_vault(&vault)
                                            {
                                                button {
                                                    class: "button-standard ghost",
                                                    style: "text-align: left; padding: 10px 12px;",
                                                    disabled: vault.used,
                                                    onclick: {
                                                        let addr = vault.address.clone();
                                                        move |_| {
                                                            if vault.used {
                                                                return;
                                                            }
                                                            selected_vault.set(addr.clone());
                                                            show_vault_picker.set(false);
                                                        }
                                                    },
                                                    div { style: "display: flex; justify-content: space-between; align-items: center; gap: 12px;" }
                                                    div {
                                                        div { style: "color: rgba(255,255,255,0.7); font-size: 12px;", "{vault_status_text(&vault)}" }
                                                        div { style: "font-family: monospace; font-size: 12px; color: #a78bfa;", "{short_address(&vault.address)}" }
                                                        div { style: "color: rgba(255,255,255,0.45); font-size: 11px; margin-top: 2px;", "{vault_split_label(&vault)}" }
                                                    }
                                                    div { style: "color: #f8fafc; font-weight: 600; font-size: 12px; white-space: nowrap;", "{vault_balance_text(&vault.address)}" }
                                                }
                                            }
                                        }
                                    }
                                }

                                if !selected_vault().is_empty() {
                                    div {
                                        style: "margin-top: 12px; padding: 16px; border-radius: 14px; background: rgba(0,0,0,0.28); border: 1px solid rgba(255,255,255,0.06);",
                                        div {
                                            style: "display: flex; justify-content: space-between; align-items: center; gap: 12px; margin-bottom: 10px;",
                                            div {
                                                div { style: "font-size: 12px; color: rgba(255,255,255,0.6); margin-bottom: 4px;", "Vault address" }
                                                div { style: "font-family: monospace; font-size: 12px; color: #a78bfa; word-break: break-all;", "{selected_vault()}" }
                                            }
                                            div {
                                                style: "text-align: right;",
                                                div { style: "font-size: 11px; color: rgba(255,255,255,0.6);", "Balance" }
                                                div { style: "font-size: 14px; font-weight: 600; color: #f8fafc;", "{selected_balance_text}" }
                                                div { style: "font-size: 11px; color: rgba(255,255,255,0.5); margin-top: 4px;", "{wallet_balance_text}" }
                                            }
                                        }

                                        if selected_vault_used {
                                            div {
                                                style: "background: rgba(245,158,11,0.1); border: 1px solid rgba(245,158,11,0.2); color: #fbbf24; padding: 8px 10px; border-radius: 8px; font-size: 11px; margin-bottom: 10px;",
                                                "This vault was already split and can no longer be used."
                                            }
                                        } else if !selected_vault_has_privkey {
                                            div {
                                                style: "background: rgba(248,113,113,0.12); border: 1px solid rgba(248,113,113,0.25); color: #fca5a5; padding: 8px 10px; border-radius: 8px; font-size: 11px; margin-bottom: 10px;",
                                                "This vault has no private key and cannot be split."
                                            }
                                        } else if !selected_vault_can_split {
                                            div {
                                                style: "background: rgba(248,113,113,0.12); border: 1px solid rgba(248,113,113,0.25); color: #fca5a5; padding: 8px 10px; border-radius: 8px; font-size: 11px; margin-bottom: 10px;",
                                                "This vault's private key is invalid. Use the original vault created before splitting."
                                            }
                                            button {
                                                class: "button-standard ghost",
                                                style: "margin-bottom: 10px; width: 100%;",
                                                onclick: move |_| {
                                                    vault_picker_mode.set(VaultPickerMode::SplitOnly);
                                                    vault_search_query.set("".to_string());
                                                    show_vault_picker.set(true);
                                                },
                                                "Choose a splittable vault"
                                            }
                                        } else if selected_vault_zero {
                                            div {
                                                style: "background: rgba(148,163,184,0.12); border: 1px solid rgba(148,163,184,0.2); color: rgba(255,255,255,0.7); padding: 8px 10px; border-radius: 8px; font-size: 11px; margin-bottom: 10px;",
                                                "Vault balance is zero. Deposit before splitting."
                                            }
                                        }

                                        if processing() {
                                            div {
                                                class: "loading-stakes-modern",
                                                div { class: "loading-spinner", style: "width: 20px; height: 20px; margin-bottom: 8px;" }
                                                div {
                                                    style: "color: rgba(255,255,255,0.7); font-size: 12px;",
                                                    if let Some(status) = status_message() {
                                                        "{status}"
                                                    } else if processing_action() == "deposit" {
                                                        "Processing deposit…"
                                                    } else if processing_action() == "split" {
                                                        "Processing split…"
                                                    } else if processing_action() == "create" {
                                                        "Processing vault creation…"
                                                    } else {
                                                        "Processing…"
                                                    }
                                                }
                                            }
                                        }
                                        if let Some(status) = status_message() {
                                            div {
                                                style: "margin-bottom: 12px; color: rgba(96,165,250,0.9); font-size: 12px;",
                                                "{status}"
                                            }
                                        }

                                        div { style: "display: grid; gap: 12px;" }
                                        div {
                                            style: "display: grid; gap: 8px; padding: 12px; border-radius: 12px; background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.05);",
                                            div { style: "color: rgba(255,255,255,0.7); font-size: 12px;", "Deposit" }
                                            input {
                                                r#type: "text",
                                                class: "input-standard",
                                                placeholder: "Amount in SOL",
                                                value: "{deposit_amount()}",
                                                oninput: move |e| deposit_amount.set(e.value()),
                                                disabled: processing(),
                                            }
                                            button {
                                                class: "button-standard primary",
                                                style: "display: inline-flex; align-items: center; justify-content: center; gap: 8px;",
                                                disabled: processing() || deposit_amount().is_empty() || selected_vault_used,
                                                onclick: handle_deposit,
                                                if processing() && processing_action() == "deposit" {
                                                    div { class: "loading-spinner", style: "width: 14px; height: 14px; margin: 0;" }
                                                    span { "Depositing…" }
                                                } else {
                                                    span { "Deposit" }
                                                }
                                            }
                                        }
                                        div {
                                            style: "display: grid; gap: 8px; padding: 12px; border-radius: 12px; background: rgba(255,255,255,0.02); border: 1px solid rgba(255,255,255,0.05);",
                                            div { style: "color: rgba(255,255,255,0.7); font-size: 12px;", "Split / Withdraw" }
                                            input {
                                                r#type: "text",
                                                class: "input-standard",
                                                placeholder: "Amount in SOL",
                                                value: "{split_amount()}",
                                                oninput: move |e| split_amount.set(e.value()),
                                                disabled: processing(),
                                            }
                                            button {
                                                class: "button-standard ghost",
                                                style: "display: inline-flex; align-items: center; justify-content: center; gap: 8px;",
                                                disabled: processing() || split_amount().is_empty() || selected_vault_used || selected_vault_zero || !selected_vault_has_privkey || !selected_vault_can_split,
                                                onclick: handle_split,
                                                if processing() && processing_action() == "split" {
                                                    div { class: "loading-spinner", style: "width: 14px; height: 14px; margin: 0;" }
                                                    span { "Splitting…" }
                                                } else {
                                                    span { "Split / Withdraw" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        },

                        ModalView::Create => rsx! {
                            div {
                                style: "border: 1px solid rgba(255,255,255,0.08); border-radius: 12px; padding: 16px;",
                                p { style: "color: rgba(255,255,255,0.7); font-size: 14px; line-height: 1.6;",
                                    "Quantum vaults use Winternitz One-Time Signatures for post-quantum security. Each vault can be split once."
                                }
                                button {
                                    class: "button-standard primary",
                                    style: "margin-top: 16px; width: 100%;",
                                    disabled: processing(),
                                    onclick: handle_create_vault,
                                    if processing() && processing_action() == "create" { "Creating..." } else { "Create Vault" }
                                }
                            }
                        },
                    }
                }

                if processing() {
                    div {
                        style: "position: absolute; inset: 0; background: rgba(0,0,0,0.55); display: flex; align-items: center; justify-content: center; border-radius: 16px; z-index: 5;",
                        div {
                            class: "loading-stakes-modern",
                            div { class: "loading-spinner" }
                            div {
                                style: "color: rgba(255,255,255,0.8); font-size: 13px;",
                                if let Some(status) = status_message() {
                                    "{status}"
                                } else if processing_action() == "deposit" {
                                    "Sending deposit…"
                                } else if processing_action() == "split" {
                                    "Sending split…"
                                } else if processing_action() == "create" {
                                    "Creating vault…"
                                } else {
                                    "Processing…"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
