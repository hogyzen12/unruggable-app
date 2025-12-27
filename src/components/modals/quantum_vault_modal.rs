use dioxus::prelude::*;
use crate::wallet::WalletInfo;
use crate::hardware::HardwareWallet;
use crate::quantum_vault::{QuantumVaultClient, VaultInfo, StoredVault};
use crate::storage::{save_quantum_vault_to_storage, load_quantum_vaults_from_storage};
use solana_winternitz::privkey::WinternitzPrivkey;
use std::sync::Arc;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use solana_sdk::signature::Keypair;

const ICON_QUANTUM: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/icons/32x32.png";

#[derive(Debug, Clone, PartialEq)]
enum ModalView {
    MyVaults,
    Create,
}

/// Convert app's wallet to Solana Keypair
fn wallet_to_keypair(wallet_info: &WalletInfo) -> Result<Keypair, String> {
    let wallet = crate::wallet::Wallet::from_wallet_info(wallet_info)?;
    let keypair_b58 = wallet.get_private_key();
    let bytes = bs58::decode(&keypair_b58)
        .into_vec()
        .map_err(|e| format!("Failed to decode keypair: {}", e))?;
    
    if bytes.len() != 64 {
        return Err(format!("Invalid keypair length: {}", bytes.len()));
    }
    
    let mut keypair_bytes = [0u8; 64];
    keypair_bytes.copy_from_slice(&bytes);
    
    Keypair::from_bytes(&keypair_bytes)
        .map_err(|e| format!("Failed to create keypair: {}", e))
}

/// Success modal for quantum vault operations
#[component]
fn QuantumVaultSuccessModal(
    operation: String,
    signature: String,
    details: String,
    vault_address: Option<String>,
    onclose: EventHandler<()>,
) -> Element {
    let solscan_url = format!("https://solscan.io/tx/{}", signature);
    
    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| onclose.call(()),
            
            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                style: "max-width: 500px;",
                
                h2 { 
                    class: "modal-title",
                    style: "margin-bottom: 24px;",
                    "{operation}"
                }
                
                div {
                    style: "text-align: center; margin-bottom: 24px;",
                    div {
                        style: "
                            width: 80px;
                            height: 80px;
                            border-radius: 50%;
                            background: linear-gradient(135deg, rgba(139, 92, 246, 0.2) 0%, rgba(168, 85, 247, 0.2) 100%);
                            display: flex;
                            align-items: center;
                            justify-content: center;
                            margin: 0 auto 16px;
                            font-size: 40px;
                        ",
                        "✓"
                    }
                    p {
                        style: "color: rgba(255,255,255,0.8); margin: 0; font-size: 15px; line-height: 1.6;",
                        "{details}"
                    }
                }
                
                if let Some(addr) = vault_address {
                    div {
                        style: "background: rgba(139, 92, 246, 0.1); border: 1px solid rgba(139, 92, 246, 0.3); padding: 16px; border-radius: 12px; margin-bottom: 20px;",
                        
                        label {
                            style: "display: block; color: rgba(255,255,255,0.6); margin-bottom: 8px; font-size: 13px; font-weight: 500;",
                            "Quantum Vault Address"
                        }
                        div {
                            style: "background: rgba(0,0,0,0.3); padding: 12px; border-radius: 8px; font-family: monospace; font-size: 13px; word-break: break-all; color: #a78bfa;",
                            "{addr}"
                        }
                    }
                }
                
                div {
                    style: "background: rgba(59, 130, 246, 0.1); border: 1px solid rgba(59, 130, 246, 0.2); padding: 16px; border-radius: 12px; margin-bottom: 20px;",
                    
                    label {
                        style: "display: block; color: rgba(255,255,255,0.6); margin-bottom: 8px; font-size: 13px; font-weight: 500;",
                        "Transaction Signature"
                    }
                    div {
                        style: "background: rgba(0,0,0,0.3); padding: 12px; border-radius: 8px; font-family: monospace; font-size: 12px; word-break: break-all; color: #60a5fa;",
                        "{signature}"
                    }
                    
                    a {
                        class: "button-standard ghost",
                        href: "{solscan_url}",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        style: "margin-top: 12px; width: 100%;",
                        "View on Solscan"
                    }
                }
                
                div { 
                    class: "modal-buttons",
                    button {
                        class: "button-standard primary",
                        style: "width: 100%;",
                        onclick: move |_| onclose.call(()),
                        "Done"
                    }
                }
            }
        }
    }
}

/// Single vault card component
#[component]
fn VaultCard(
    vault: StoredVault,
    balance: Option<f64>,
    loading: bool,
    ondeposit: EventHandler<String>,
    onsplit: EventHandler<String>,
) -> Element {
    let mut show_deposit = use_signal(|| false);
    let mut show_split = use_signal(|| false);
    let mut deposit_amount = use_signal(|| "".to_string());
    let mut split_amount = use_signal(|| "".to_string());
    rsx! {
        div {
            style: "
                background: linear-gradient(135deg, rgba(139, 92, 246, 0.05) 0%, rgba(168, 85, 247, 0.05) 100%);
                border: 1px solid rgba(139, 92, 246, 0.2);
                border-radius: 16px;
                padding: 20px;
                margin-bottom: 12px;
            ",
            
            div {
                style: "display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 16px;",
                
                div {
                    style: "flex: 1;",
                    div {
                        style: "display: flex; align-items: center; gap: 8px; margin-bottom: 8px;",
                        div {
                            style: format!("
                                width: 8px;
                                height: 8px;
                                border-radius: 50%;
                                background: {};
                            ", if vault.used { "#ef4444" } else { "#22c55e" })
                        }
                        span {
                            style: "color: rgba(255,255,255,0.6); font-size: 13px;",
                            if vault.used { "Used" } else { "Active" }
                        }
                    }
                    
                    div {
                        style: "font-family: monospace; font-size: 14px; color: #a78bfa; word-break: break-all;",
                        "{vault.address}"
                    }
                }
                
                if loading {
                    div {
                        style: "
                            width: 20px;
                            height: 20px;
                            border: 2px solid rgba(139, 92, 246, 0.3);
                            border-top-color: #8b5cf6;
                            border-radius: 50%;
                            animation: spin 1s linear infinite;
                        "
                    }
                } else if let Some(bal) = balance {
                    div {
                        style: "text-align: right;",
                        div {
                            style: "font-size: 24px; font-weight: 700; color: #f8fafc; margin-bottom: 4px;",
                            "{bal:.4}"
                        }
                        div {
                            style: "font-size: 13px; color: rgba(255,255,255,0.6);",
                            "SOL"
                        }
                    }
                }
            }
            
            // Deposit form (appears when deposit button clicked)
            if show_deposit() && !vault.used {
                div {
                    style: "margin-top: 12px; padding: 12px; background: rgba(0,0,0,0.2); border-radius: 8px;",
                    
                    input {
                        r#type: "text",
                        class: "input-standard",
                        placeholder: "Amount (SOL)",
                        value: "{deposit_amount()}",
                        oninput: move |e| deposit_amount.set(e.value()),
                        style: "width: 100%; margin-bottom: 8px; font-size: 14px;"
                    }
                    
                    div {
                        style: "display: flex; gap: 8px;",
                        
                        button {
                            class: "button-standard primary",
                            style: "flex: 1; font-size: 13px; padding: 8px;",
                            disabled: deposit_amount().is_empty(),
                            onclick: {
                                let addr = vault.address.clone();
                                move |_| {
                                    ondeposit.call(addr.clone());
                                    show_deposit.set(false);
                                    deposit_amount.set("".to_string());
                                }
                            },
                            "Confirm Deposit"
                        }
                        
                        button {
                            class: "button-standard ghost",
                            style: "flex: 1; font-size: 13px; padding: 8px;",
                            onclick: move |_| {
                                show_deposit.set(false);
                                deposit_amount.set("".to_string());
                            },
                            "Cancel"
                        }
                    }
                }
            }
            
            // Split form (appears when split button clicked)
            if show_split() && !vault.used && balance.is_some() && balance.unwrap() > 0.0 {
                div {
                    style: "margin-top: 12px; padding: 12px; background: rgba(0,0,0,0.2); border-radius: 8px;",
                    
                    div {
                        style: "margin-bottom: 8px;",
                        label {
                            style: "display: block; color: rgba(255,255,255,0.6); font-size: 13px; margin-bottom: 4px;",
                            "Amount to withdraw (SOL)"
                        }
                        input {
                            r#type: "text",
                            class: "input-standard",
                            placeholder: "Amount (SOL)",
                            value: "{split_amount()}",
                            oninput: move |e| split_amount.set(e.value()),
                            style: "width: 100%; font-size: 14px;"
                        }
                    }
                    
                    div {
                        style: "display: flex; gap: 8px;",
                        
                        button {
                            class: "button-standard primary",
                            style: "flex: 1; font-size: 13px; padding: 8px;",
                            disabled: split_amount().is_empty(),
                            onclick: {
                                let addr = vault.address.clone();
                                move |_| {
                                    onsplit.call(addr.clone());
                                    show_split.set(false);
                                    split_amount.set("".to_string());
                                }
                            },
                            "Confirm Split"
                        }
                        
                        button {
                            class: "button-standard ghost",
                            style: "flex: 1; font-size: 13px; padding: 8px;",
                            onclick: move |_| {
                                show_split.set(false);
                                split_amount.set("".to_string());
                            },
                            "Cancel"
                        }
                    }
                }
            }
            
            // Action buttons
            if !show_deposit() && !show_split() {
                div {
                    style: "display: flex; gap: 8px;",
                    
                    button {
                        class: "button-standard ghost",
                        style: "flex: 1; font-size: 14px; padding: 10px;",
                        disabled: vault.used,
                        onclick: move |_| show_deposit.set(true),
                        "Deposit"
                    }
                    
                    button {
                        class: "button-standard primary",
                        style: "flex: 1; font-size: 14px; padding: 10px;",
                        disabled: vault.used || balance.is_none() || balance.unwrap() == 0.0,
                        onclick: move |_| show_split.set(true),
                        "Split/Withdraw"
                    }
                }
            }
        }
    }
}

#[component]
pub fn QuantumVaultModal(
    wallet: Option<WalletInfo>,
    hardware_wallet: Option<Arc<HardwareWallet>>,
    custom_rpc: Option<String>,
    onclose: EventHandler<()>,
) -> Element {
    let mut current_view = use_signal(|| ModalView::MyVaults);
    let mut processing = use_signal(|| false);
    let mut error_message = use_signal(|| None as Option<String>);
    let mut status_message = use_signal(|| None as Option<String>);
    
    // My Vaults state
    let mut my_vaults = use_signal(|| Vec::<StoredVault>::new());
    let mut vault_balances = use_signal(|| std::collections::HashMap::<String, f64>::new());
    let mut loading_balances = use_signal(|| false);
    let mut reload_balances_trigger = use_signal(|| 0);
    
    // Active operation state
    let mut active_vault_address = use_signal(|| "".to_string());
    let mut deposit_amount = use_signal(|| "".to_string());
    let mut split_amount = use_signal(|| "".to_string());
    
    // Success modal state
    let mut show_success = use_signal(|| false);
    let mut success_operation = use_signal(|| "".to_string());
    let mut success_signature = use_signal(|| "".to_string());
    let mut success_details = use_signal(|| "".to_string());
    let mut success_vault_address = use_signal(|| None as Option<String>);
    
    let rpc_url = custom_rpc.clone().unwrap_or_else(|| "https://johna-k3cr1v-fast-mainnet.helius-rpc.com".to_string());
    
    // Clone for handlers
    let rpc_for_balances = rpc_url.clone();
    let rpc_for_create = rpc_url.clone();
    let rpc_for_deposit = rpc_url.clone();
    let wallet_for_create = wallet.clone();
    let wallet_for_deposit = wallet.clone();
    
    // Load vaults on mount
    use_effect(move || {
        let vaults = load_quantum_vaults_from_storage();
        my_vaults.set(vaults);
    });
    
    // Load balances for all vaults
    use_effect(move || {
        let _ = reload_balances_trigger();
        let vaults = my_vaults().clone();
        let rpc_clone = rpc_for_balances.clone();
        
        if vaults.is_empty() {
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
    
    // Create vault handler
    let handle_create_vault = move |_| {
        let rpc_url_create = rpc_for_create.clone();
        let wallet_create = wallet_for_create.clone();
        spawn(async move {
            log::info!("QUANTUM VAULT: Starting vault creation flow");
            processing.set(true);
            error_message.set(None);
            status_message.set(Some("Generating quantum-resistant keys...".to_string()));
            
            match QuantumVaultClient::new(Some(&rpc_url_create)) {
                Ok(client) => {
                    log::info!("QUANTUM VAULT: Client initialized");
                    status_message.set(Some("Creating vault on-chain...".to_string()));
                    let (privkey, vault_address, bump, pubkey_hash) = client.generate_new_vault();
                    log::info!("QUANTUM VAULT: Generated vault address: {}", vault_address);
                    
                    if let Some(wallet_info) = &wallet_create {
                        match wallet_to_keypair(wallet_info) {
                            Ok(keypair) => {
                                log::info!("QUANTUM VAULT: Sending transaction...");
                                match client.create_vault(&keypair, &pubkey_hash, bump).await {
                                    Ok(signature) => {
                                        log::info!("QUANTUM VAULT: Transaction confirmed!");
                                        log::info!("QUANTUM VAULT: Signature: {}", signature);
                                        status_message.set(Some("Saving vault to storage...".to_string()));
                                        // Serialize WinternitzPrivkey to bytes (896 bytes)
                                        let privkey_bytes: [u8; 896] = unsafe {
                                            std::mem::transmute::<WinternitzPrivkey, [u8; 896]>(privkey)
                                        };
                                        
                                        // Automatically save vault to storage
                                        let stored_vault = StoredVault {
                                            name: format!("Quantum Vault {}", vault_address.to_string().chars().take(8).collect::<String>()),
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
                                        log::info!("QUANTUM VAULT: Vault saved to storage");
                                        
                                        // Reload vaults
                                        my_vaults.set(load_quantum_vaults_from_storage());
                                        log::info!("QUANTUM VAULT: Vault creation complete!");
                                        
                                        success_operation.set("Vault Created".to_string());
                                        success_signature.set(signature);
                                        success_details.set("Your quantum-secure vault has been created and saved automatically. You can now deposit SOL to secure it against quantum attacks.".to_string());
                                        success_vault_address.set(Some(vault_address.to_string()));
                                        show_success.set(true);
                                        
                                        // Trigger balance reload
                                        reload_balances_trigger.set(reload_balances_trigger() + 1);
                                    }
                                    Err(e) => error_message.set(Some(format!("Failed to create vault: {}", e))),
                                }
                            }
                            Err(e) => error_message.set(Some(format!("Failed to load wallet: {}", e))),
                        }
                    } else {
                        error_message.set(Some("No wallet connected".to_string()));
                    }
                }
                Err(e) => error_message.set(Some(format!("Failed to initialize client: {}", e))),
            }
            
            processing.set(false);
        });
    };
    
    // Deposit handler - called from vault cards
    let handle_deposit = move |_vault_addr: String| {
        let rpc_url_deposit = rpc_for_deposit.clone();
        let wallet_deposit = wallet_for_deposit.clone();
        spawn(async move {
            log::info!("QUANTUM VAULT: Starting deposit flow");
            processing.set(true);
            error_message.set(None);
            status_message.set(Some("Preparing deposit transaction...".to_string()));
            
            let amount_str = deposit_amount();
            let vault_addr = active_vault_address();
            
            if vault_addr.is_empty() {
                error_message.set(Some("Please select a vault".to_string()));
                processing.set(false);
                return;
            }
            
            let amount_sol: f64 = match amount_str.parse() {
                Ok(val) if val > 0.0 => val,
                _ => {
                    error_message.set(Some("Invalid amount".to_string()));
                    processing.set(false);
                    return;
                }
            };
            
            let amount_lamports = (amount_sol * LAMPORTS_PER_SOL as f64) as u64;
            log::info!("QUANTUM VAULT: Depositing {} SOL ({} lamports) to {}", amount_sol, amount_lamports, vault_addr);
            
            status_message.set(Some(format!("Depositing {} SOL...", amount_sol)));
            match QuantumVaultClient::new(Some(&rpc_url_deposit)) {
                Ok(client) => {
                    if let Some(wallet_info) = &wallet_deposit {
                        match wallet_to_keypair(wallet_info) {
                            Ok(keypair) => {
                                let vault_pubkey = match bs58::decode(&vault_addr).into_vec() {
                                    Ok(bytes) if bytes.len() == 32 => {
                                        let mut arr = [0u8; 32];
                                        arr.copy_from_slice(&bytes);
                                        solana_sdk::pubkey::Pubkey::new_from_array(arr)
                                    }
                                    _ => {
                                        error_message.set(Some("Invalid vault address format".to_string()));
                                        processing.set(false);
                                        return;
                                    }
                                };
                                
                                match client.deposit_to_vault(&keypair, &vault_pubkey, amount_lamports).await {
                                    Ok(signature) => {
                                        log::info!("QUANTUM VAULT: Deposit confirmed!");
                                        log::info!("QUANTUM VAULT: Signature: {}", signature);
                                        success_operation.set("Deposit Complete".to_string());
                                        success_signature.set(signature);
                                        success_details.set(format!(
                                            "Deposited {} SOL to quantum vault. Your funds are now secured with post-quantum cryptography.",
                                            amount_sol
                                        ));
                                        success_vault_address.set(None);
                                        show_success.set(true);
                                        deposit_amount.set("".to_string());
                                        
                                        // Trigger balance reload
                                        reload_balances_trigger.set(reload_balances_trigger() + 1);
                                    }
                                    Err(e) => error_message.set(Some(format!("Deposit failed: {}", e))),
                                }
                            }
                            Err(e) => error_message.set(Some(format!("Failed to load wallet: {}", e))),
                        }
                    }
                }
                Err(e) => error_message.set(Some(format!("Failed to initialize client: {}", e))),
            }
            
            processing.set(false);
        });
    };
    
    // Split handler - called from vault cards
    let handle_split = move |_vault_addr: String| {
        let rpc_url_split = rpc_url.clone();
        let wallet_split = wallet.clone();
        spawn(async move {
            log::info!("QUANTUM VAULT: Starting split/withdraw flow");
            processing.set(true);
            error_message.set(None);
            status_message.set(Some("Loading vault private key...".to_string()));
            
            let amount_str = split_amount();
            let vault_addr = active_vault_address();
            
            if vault_addr.is_empty() {
                error_message.set(Some("Please select a vault".to_string()));
                processing.set(false);
                return;
            }
            
            let amount_sol: f64 = match amount_str.parse() {
                Ok(val) if val > 0.0 => val,
                _ => {
                    error_message.set(Some("Invalid amount".to_string()));
                    processing.set(false);
                    return;
                }
            };
            
            let amount_lamports = (amount_sol * LAMPORTS_PER_SOL as f64) as u64;
            log::info!("QUANTUM VAULT: Splitting {} SOL ({} lamports) from vault {}", amount_sol, amount_lamports, vault_addr);
            
            // Find the vault in storage to get private key and metadata
            let vaults = load_quantum_vaults_from_storage();
            let vault_data = match vaults.iter().find(|v| v.address == vault_addr) {
                Some(v) => v.clone(),
                None => {
                    error_message.set(Some("Vault not found in storage".to_string()));
                    processing.set(false);
                    return;
                }
            };
            
            // Decode private key bytes (896 bytes)
            let privkey_bytes = match base64::decode(&vault_data.private_key) {
                Ok(bytes) if bytes.len() == 896 => bytes,
                Ok(bytes) => {
                    error_message.set(Some(format!("Invalid private key length: {} bytes (expected 896)", bytes.len())));
                    processing.set(false);
                    return;
                }
                Err(e) => {
                    error_message.set(Some(format!("Failed to decode private key: {}", e)));
                    processing.set(false);
                    return;
                }
            };
            
            // Convert bytes to [u8; 896] array
            let mut privkey_array = [0u8; 896];
            privkey_array.copy_from_slice(&privkey_bytes);
            
            // Deserialize to WinternitzPrivkey using transmute (reverse of serialization)
            let vault_privkey: WinternitzPrivkey = unsafe {
                std::mem::transmute::<[u8; 896], WinternitzPrivkey>(privkey_array)
            };
            log::info!("QUANTUM VAULT: Private key loaded successfully");
            
            // Parse vault address
            let vault_pubkey = match bs58::decode(&vault_addr).into_vec() {
                Ok(bytes) if bytes.len() == 32 => {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&bytes);
                    solana_sdk::pubkey::Pubkey::new_from_array(arr)
                }
                _ => {
                    error_message.set(Some("Invalid vault address format".to_string()));
                    processing.set(false);
                    return;
                }
            };
            
            // Initialize client
            let client = match QuantumVaultClient::new(Some(&rpc_url_split)) {
                Ok(c) => c,
                Err(e) => {
                    error_message.set(Some(format!("Failed to initialize client: {}", e)));
                    processing.set(false);
                    return;
                }
            };
            
            // Generate new vaults for split and refund
            status_message.set(Some("Generating new vaults for split...".to_string()));
            let (_, split_vault_address, split_bump, split_pubkey_hash) = client.generate_new_vault();
            let (_, refund_vault_address, refund_bump, refund_pubkey_hash) = client.generate_new_vault();
            log::info!("QUANTUM VAULT: Split vault: {}", split_vault_address);
            log::info!("QUANTUM VAULT: Refund vault: {}", refund_vault_address);
            
            // Get wallet keypair
            let keypair = match &wallet_split {
                Some(wallet_info) => match wallet_to_keypair(wallet_info) {
                    Ok(kp) => kp,
                    Err(e) => {
                        error_message.set(Some(format!("Failed to load wallet: {}", e)));
                        processing.set(false);
                        return;
                    }
                },
                None => {
                    error_message.set(Some("No wallet connected".to_string()));
                    processing.set(false);
                    return;
                }
            };
            
            // Create the split and refund vaults first
            status_message.set(Some("Creating split vault on-chain...".to_string()));
            log::info!("QUANTUM VAULT: Creating split vault...");
            match client.create_vault(&keypair, &split_pubkey_hash, split_bump).await {
                Ok(sig) => {
                    log::info!("QUANTUM VAULT: Split vault created: {}", sig);
                },
                Err(e) => {
                    error_message.set(Some(format!("Failed to create split vault: {}", e)));
                    processing.set(false);
                    return;
                }
            }
            
            status_message.set(Some("Creating refund vault on-chain...".to_string()));
            log::info!("QUANTUM VAULT: Creating refund vault...");
            match client.create_vault(&keypair, &refund_pubkey_hash, refund_bump).await {
                Ok(sig) => {
                    log::info!("QUANTUM VAULT: Refund vault created: {}", sig);
                },
                Err(e) => {
                    error_message.set(Some(format!("Failed to create refund vault: {}", e)));
                    processing.set(false);
                    return;
                }
            }
            
            // Perform the split
            status_message.set(Some("Executing quantum-resistant split transaction...".to_string()));
            log::info!("QUANTUM VAULT: Executing split with Winternitz signature...");
            match client.split_vault(
                &keypair,
                &vault_privkey,
                &vault_pubkey,
                &split_vault_address,
                &refund_vault_address,
                amount_lamports,
                vault_data.bump,
            ).await {
                Ok(result) => {
                    log::info!("QUANTUM VAULT: Split successful!");
                    log::info!("QUANTUM VAULT: Transaction: {}", result.transaction_signature);
                    log::info!("QUANTUM VAULT: Split amount: {} SOL", result.split_amount as f64 / LAMPORTS_PER_SOL as f64);
                    log::info!("QUANTUM VAULT: Refund amount: {} SOL", result.refund_amount as f64 / LAMPORTS_PER_SOL as f64);
                    
                    status_message.set(Some("Saving new vaults...".to_string()));
                    // Mark original vault as used
                    crate::storage::mark_quantum_vault_as_used(&vault_addr);
                    log::info!("QUANTUM VAULT: Original vault marked as used");
                    
                    // Save new split vault (this is the withdrawal amount)
                    let split_vault = StoredVault {
                        name: format!("Split {}", split_vault_address.to_string().chars().take(8).collect::<String>()),
                        address: split_vault_address.to_string(),
                        pubkey_hash: hex::encode(split_pubkey_hash),
                        private_key: "".to_string(), // Don't store key for split vault
                        bump: split_bump,
                        created_at: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64,
                        used: true, // Already used in split
                    };
                    crate::storage::save_quantum_vault_to_storage(&split_vault);
                    log::info!("QUANTUM VAULT: Split vault saved");
                    
                    // Save refund vault (remaining balance)
                    let refund_vault = StoredVault {
                        name: format!("Refund {}", refund_vault_address.to_string().chars().take(8).collect::<String>()),
                        address: refund_vault_address.to_string(),
                        pubkey_hash: hex::encode(refund_pubkey_hash),
                        private_key: "".to_string(), // Don't store key for refund vault
                        bump: refund_bump,
                        created_at: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64,
                        used: true, // Already used in split
                    };
                    crate::storage::save_quantum_vault_to_storage(&refund_vault);
                    log::info!("QUANTUM VAULT: Refund vault saved");
                    
                    // Reload vaults
                    my_vaults.set(load_quantum_vaults_from_storage());
                    log::info!("QUANTUM VAULT: Split operation complete!");
                    
                    success_operation.set("Vault Split Complete".to_string());
                    success_signature.set(result.transaction_signature);
                    success_details.set(format!(
                        "Successfully split {} SOL to new vault. Remaining balance sent to refund vault. Original vault is now closed (one-time signature used).",
                        result.split_amount as f64 / LAMPORTS_PER_SOL as f64
                    ));
                    success_vault_address.set(Some(split_vault_address.to_string()));
                    show_success.set(true);
                    split_amount.set("".to_string());
                    
                    // Trigger balance reload
                    reload_balances_trigger.set(reload_balances_trigger() + 1);
                }
                Err(e) => error_message.set(Some(format!("Split failed: {}", e))),
            }
            
            processing.set(false);
        });
    };
    
    rsx! {
        style {
            "
            @keyframes spin {{
                to {{ transform: rotate(360deg); }}
            }}
            
            .tab-active {{
                background: linear-gradient(135deg, rgba(139, 92, 246, 0.2) 0%, rgba(168, 85, 247, 0.2) 100%);
                color: #a78bfa;
            }}
            
            .tab-inactive {{
                background: transparent;
                color: rgba(255,255,255,0.6);
            }}
            "
        }
        
        if show_success() {
            QuantumVaultSuccessModal {
                operation: success_operation(),
                signature: success_signature(),
                details: success_details(),
                vault_address: success_vault_address(),
                onclose: move |_| {
                    show_success.set(false);
                    current_view.set(ModalView::MyVaults);
                }
            }
        }
        
        div {
            class: "modal-backdrop",
            onclick: move |_| onclose.call(()),
            
            div {
                class: "modal-content",
                style: "max-width: 700px;",
                onclick: move |e| e.stop_propagation(),
                
                // Header
                div {
                    style: "
                        display: flex;
                        justify-content: space-between;
                        align-items: center;
                        padding: 24px;
                        border-bottom: 1px solid rgba(255,255,255,0.1);
                    ",
                    
                    h2 {
                        style: "color: #f8fafc; font-size: 22px; font-weight: 700; margin: 0;",
                        "Quantum Vault"
                    }
                    
                    button {
                        style: "
                            background: none;
                            border: none;
                            color: white;
                            font-size: 28px;
                            cursor: pointer;
                            padding: 0;
                        ",
                        onclick: move |_| onclose.call(()),
                        "×"
                    }
                }
                
                // Status and Error messages
                if let Some(status) = status_message() {
                    div {
                        style: "
                            background: rgba(59, 130, 246, 0.1);
                            border: 1px solid rgba(59, 130, 246, 0.3);
                            color: #3b82f6;
                            padding: 12px 24px;
                            margin: 16px 24px 0;
                            border-radius: 8px;
                            font-size: 14px;
                            display: flex;
                            align-items: center;
                            gap: 12px;
                        ",
                        div {
                            style: "
                                width: 16px;
                                height: 16px;
                                border: 2px solid rgba(59, 130, 246, 0.3);
                                border-top-color: #3b82f6;
                                border-radius: 50%;
                                animation: spin 1s linear infinite;
                            "
                        }
                        span { "{status}" }
                    }
                }
                
                if let Some(error) = error_message() {
                    div {
                        style: "
                            background: rgba(239, 68, 68, 0.1);
                            border: 1px solid rgba(239, 68, 68, 0.3);
                            color: #ef4444;
                            padding: 12px 24px;
                            margin: 16px 24px 0;
                            border-radius: 8px;
                            font-size: 14px;
                        ",
                        "{error}"
                    }
                }
                
                // View selector
                div {
                    style: "
                        display: flex;
                        gap: 4px;
                        padding: 16px 24px 0;
                        border-bottom: 1px solid rgba(255,255,255,0.1);
                    ",
                    
                    button {
                        class: if current_view() == ModalView::MyVaults { "tab-active" } else { "tab-inactive" },
                        style: "
                            flex: 1;
                            padding: 12px;
                            border: none;
                            cursor: pointer;
                            border-radius: 8px 8px 0 0;
                            font-size: 14px;
                            font-weight: 600;
                            transition: all 0.2s;
                        ",
                        onclick: move |_| current_view.set(ModalView::MyVaults),
                        "My Vaults"
                    }
                    
                    button {
                        class: if current_view() == ModalView::Create { "tab-active" } else { "tab-inactive" },
                        style: "
                            flex: 1;
                            padding: 12px;
                            border: none;
                            cursor: pointer;
                            border-radius: 8px 8px 0 0;
                            font-size: 14px;
                            font-weight: 600;
                            transition: all 0.2s;
                        ",
                        onclick: move |_| current_view.set(ModalView::Create),
                        "Create"
                    }
                    

                }
                
                // Content area
                div {
                    style: "padding: 24px; min-height: 400px; max-height: 500px; overflow-y: auto;",
                    
                    match current_view() {
                        ModalView::MyVaults => rsx! {
                            div {
                                if my_vaults().is_empty() {
                                    div {
                                        style: "text-align: center; padding: 60px 20px;",
                                        
                                        div {
                                            style: "font-size: 48px; margin-bottom: 16px; opacity: 0.5;",
                                            "🔐"
                                        }
                                        
                                        h3 {
                                            style: "color: rgba(255,255,255,0.8); margin-bottom: 8px; font-size: 18px;",
                                            "No Quantum Vaults Yet"
                                        }
                                        
                                        p {
                                            style: "color: rgba(255,255,255,0.6); margin-bottom: 24px; font-size: 14px; line-height: 1.6;",
                                            "Create your first quantum-secure vault to protect your SOL against future quantum computers."
                                        }
                                        
                                        button {
                                            class: "button-standard primary",
                                            onclick: move |_| current_view.set(ModalView::Create),
                                            "Create Your First Vault"
                                        }
                                    }
                                } else {
                                    div {
                                        div {
                                            style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 20px;",
                                            
                                            h3 {
                                                style: "color: rgba(255,255,255,0.9); margin: 0; font-size: 16px; font-weight: 600;",
                                                "Your Quantum Vaults ({my_vaults().len()})"
                                            }
                                            
                                            button {
                                                class: "button-standard ghost",
                                                style: "padding: 8px 16px; font-size: 13px;",
                                                onclick: move |_| reload_balances_trigger.set(reload_balances_trigger() + 1),
                                                disabled: loading_balances(),
                                                if loading_balances() { "Refreshing..." } else { "Refresh" }
                                            }
                                        }
                                        
                                        for vault in my_vaults() {
                                            VaultCard {
                                                key: "{vault.address}",
                                                vault: vault.clone(),
                                                balance: vault_balances().get(&vault.address).copied(),
                                                loading: loading_balances(),
                                                ondeposit: move |addr| {
                                                    active_vault_address.set(addr);
                                                },
                                                onsplit: move |addr| {
                                                    active_vault_address.set(addr);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        
                        ModalView::Create => rsx! {
                            div {
                                div {
                                    style: "
                                        background: linear-gradient(135deg, rgba(139, 92, 246, 0.1) 0%, rgba(168, 85, 247, 0.1) 100%);
                                        border: 1px solid rgba(139, 92, 246, 0.3);
                                        padding: 20px;
                                        border-radius: 12px;
                                        margin-bottom: 24px;
                                    ",
                                    
                                    h3 {
                                        style: "color: #a78bfa; margin: 0 0 12px 0; font-size: 16px; font-weight: 600;",
                                        "Post-Quantum Security"
                                    }
                                    
                                    p {
                                        style: "color: rgba(255,255,255,0.8); margin: 0 0 12px 0; font-size: 14px; line-height: 1.6;",
                                        "Quantum vaults use Winternitz One-Time Signatures (WOTS) to protect your SOL against quantum computer attacks."
                                    }
                                    
                                    ul {
                                        style: "color: rgba(255,255,255,0.7); margin: 0; padding-left: 20px; font-size: 13px; line-height: 1.8;",
                                        li { "Based on hash functions (SHA256), not elliptic curves" }
                                        li { "Secure against Shor's algorithm" }
                                        li { "Keys saved automatically to device" }
                                        li { "One-time signatures (vault closes after split)" }
                                    }
                                }
                                
                                button {
                                    class: "button-standard primary",
                                    style: "width: 100%; font-size: 16px; padding: 16px;",
                                    disabled: processing(),
                                    onclick: handle_create_vault,
                                    if processing() {
                                        "Creating Vault..."
                                    } else {
                                        "Create Quantum Vault"
                                    }
                                }
                            }
                        },
                        

                        
                        _ => rsx! { div {} }
                    }
                }
            }
        }
    }
}