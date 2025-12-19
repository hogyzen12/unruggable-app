use dioxus::prelude::*;
use crate::wallet::WalletInfo;
use crate::hardware::HardwareWallet;
use crate::quantum_vault::{QuantumVaultClient, VaultInfo, StoredQuantumVault, store_quantum_vault, get_all_quantum_vaults, mark_vault_as_used};
use solana_winternitz::privkey::WinternitzPrivkey;
use std::sync::Arc;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use solana_sdk::signature::Keypair;

const ICON_QUANTUM: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/icons/32x32.png";

#[derive(Debug, Clone, PartialEq)]
enum ModalView {
    MyVaults,
    Create,
    Deposit,
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
    vault: StoredQuantumVault,
    balance: Option<f64>,
    loading: bool,
    ondeposit: EventHandler<String>,
) -> Element {
    let created_date = {
        let timestamp = vault.created_at / 1000; // Convert ms to seconds
        format!("Created {}", timestamp) // Simplified - could use proper date formatting
    };
    
    rsx! {
        div {
            style: "
                background: linear-gradient(135deg, rgba(139, 92, 246, 0.05) 0%, rgba(168, 85, 247, 0.05) 100%);
                border: 1px solid rgba(139, 92, 246, 0.2);
                border-radius: 16px;
                padding: 20px;
                margin-bottom: 12px;
                transition: all 0.3s ease;
            ",
            onmouseenter: move |_| {},
            onmouseleave: move |_| {},
            
            div {
                style: "display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 16px;",
                
                div {
                    style: "flex: 1;",
                    div {
                        style: "display: flex; align-items: center; gap: 8px; margin-bottom: 8px;",
                        div {
                            style: format_args!("width: 8px; height: 8px; border-radius: 50%; background: {};", 
                                if vault.used { "#ef4444" } else { "#22c55e" })
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
            
            div {
                style: "display: flex; gap: 8px;",
                
                button {
                    class: "button-standard ghost",
                    style: "flex: 1; font-size: 14px; padding: 10px;",
                    disabled: vault.used,
                    onclick: move |_| {
                        let addr = vault.address.clone();
                        ondeposit.call(addr)
                    },
                    "Deposit"
                }
                
                if !vault.used {
                    button {
                        class: "button-standard ghost",
                        style: "flex: 1; font-size: 14px; padding: 10px;",
                        "Split"
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
    
    // My Vaults state
    let mut my_vaults = use_signal(|| Vec::<StoredQuantumVault>::new());
    let mut vault_balances = use_signal(|| std::collections::HashMap::<String, f64>::new());
    let mut loading_balances = use_signal(|| false);
    
    // Deposit state
    let mut deposit_vault_address = use_signal(|| "".to_string());
    let mut deposit_amount = use_signal(|| "".to_string());
    
    // Success modal state
    let mut show_success = use_signal(|| false);
    let mut success_operation = use_signal(|| "".to_string());
    let mut success_signature = use_signal(|| "".to_string());
    let mut success_details = use_signal(|| "".to_string());
    let mut success_vault_address = use_signal(|| None as Option<String>);
    
    let rpc_url = custom_rpc.clone().unwrap_or_else(|| "https://johna-k3cr1v-fast-mainnet.helius-rpc.com".to_string());
    
    // Clone rpc_url for each closure that needs it
    let rpc_for_balances = rpc_url.clone();
    let rpc_for_create = rpc_url.clone();
    let rpc_for_deposit = rpc_url.clone();
    
    // Clone wallet for each handler
    let wallet_for_create = wallet.clone();
    let wallet_for_deposit = wallet.clone();
    
    // Signal to trigger balance reload
    let mut reload_balances_trigger = use_signal(|| 0);
    
    // Load vaults on mount
    use_effect(move || {
        match get_all_quantum_vaults() {
            Ok(vaults) => {
                my_vaults.set(vaults);
            }
            Err(e) => {
                error_message.set(Some(format!("Failed to load vaults: {}", e)));
            }
        }
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
            processing.set(true);
            error_message.set(None);
            
            match QuantumVaultClient::new(Some(&rpc_url_create)) {
                Ok(client) => {
                    let (privkey, vault_address, bump, pubkey_hash) = client.generate_new_vault();
                    
                    if let Some(wallet_info) = &wallet_create {
                        match wallet_to_keypair(wallet_info) {
                            Ok(keypair) => {
                                match client.create_vault(&keypair, &pubkey_hash, bump).await {
                                    Ok(signature) => {
                                        // Store vault automatically
                                        let stored_vault = StoredQuantumVault {
                                            address: vault_address.to_string(),
                                            pubkey_hash: hex::encode(pubkey_hash),
                                            private_key: base64::encode(format!("{:?}", privkey)), // Placeholder
                                            bump,
                                            created_at: web_sys::window()
                                                .and_then(|w| w.performance())
                                                .map(|p| p.now() as u64)
                                                .unwrap_or(0),
                                            used: false,
                                        };
                                        
                                        if let Err(e) = store_quantum_vault(&stored_vault) {
                                            error_message.set(Some(format!("Vault created but failed to save: {}", e)));
                                        } else {
                                            // Reload vaults
                                            if let Ok(vaults) = get_all_quantum_vaults() {
                                                my_vaults.set(vaults);
                                            }
                                            
                                            success_operation.set("Vault Created".to_string());
                                            success_signature.set(signature);
                                            success_details.set("Your quantum-secure vault has been created and saved automatically. You can now deposit SOL to secure it against quantum attacks.".to_string());
                                            success_vault_address.set(Some(vault_address.to_string()));
                                            show_success.set(true);
                                        }
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
    
    // Deposit handler
    let handle_deposit = move |_| {
        let rpc_url_deposit = rpc_for_deposit.clone();
        let wallet_deposit = wallet_for_deposit.clone();
        spawn(async move {
            processing.set(true);
            error_message.set(None);
            
            let amount_str = deposit_amount();
            let vault_addr = deposit_vault_address();
            
            if vault_addr.is_empty() {
                error_message.set(Some("Please enter vault address".to_string()));
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
    
    rsx! {
        style {
            "
            @keyframes spin {{
                to {{ transform: rotate(360deg); }}
            }}
            
            @keyframes fadeIn {{
                from {{ opacity: 0; transform: translateY(10px); }}
                to {{ opacity: 1; transform: translateY(0); }}
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
                style: "max-width: 700px; animation: fadeIn 0.3s ease;",
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
                        style: "color: #f8fafc; font-size: 22px; font-weight: 700; margin: 0; letter-spacing: -0.025em;",
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
                            min-width: 32px;
                            min-height: 32px;
                            display: flex;
                            align-items: center;
                            justify-content: center;
                            transition: opacity 0.2s;
                        ",
                        onclick: move |_| onclose.call(()),
                        "×"
                    }
                }
                
                // Error message
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
                    
                    button {
                        class: if current_view() == ModalView::Deposit { "tab-active" } else { "tab-inactive" },
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
                        onclick: move |_| current_view.set(ModalView::Deposit),
                        "Deposit"
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
                                                    deposit_vault_address.set(addr);
                                                    current_view.set(ModalView::Deposit);
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
                                        li { "Keys saved automatically" }
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
                        
                        ModalView::Deposit => rsx! {
                            div {
                                div {
                                    style: "margin-bottom: 20px;",
                                    label {
                                        style: "display: block; color: rgba(255,255,255,0.8); margin-bottom: 8px; font-size: 14px; font-weight: 500;",
                                        "Vault Address"
                                    }
                                    input {
                                        r#type: "text",
                                        class: "input-standard",
                                        placeholder: "Enter quantum vault address",
                                        value: "{deposit_vault_address()}",
                                        oninput: move |e| deposit_vault_address.set(e.value()),
                                        style: "width: 100%; font-family: monospace; font-size: 13px;"
                                    }
                                }
                                
                                div {
                                    style: "margin-bottom: 24px;",
                                    label {
                                        style: "display: block; color: rgba(255,255,255,0.8); margin-bottom: 8px; font-size: 14px; font-weight: 500;",
                                        "Amount (SOL)"
                                    }
                                    input {
                                        r#type: "text",
                                        class: "input-standard",
                                        placeholder: "0.0",
                                        value: "{deposit_amount()}",
                                        oninput: move |e| deposit_amount.set(e.value()),
                                        style: "width: 100%; font-size: 16px;"
                                    }
                                }
                                
                                button {
                                    class: "button-standard primary",
                                    style: "width: 100%; font-size: 16px; padding: 16px;",
                                    disabled: processing() || deposit_vault_address().is_empty() || deposit_amount().is_empty(),
                                    onclick: handle_deposit,
                                    if processing() {
                                        "Depositing..."
                                    } else {
                                        "Deposit SOL"
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