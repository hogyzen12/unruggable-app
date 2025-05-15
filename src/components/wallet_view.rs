use dioxus::prelude::*;
use crate::wallet::{Wallet, WalletInfo};
use crate::storage::{
    load_wallets_from_storage, 
    save_wallet_to_storage, 
    load_rpc_from_storage,
    save_rpc_to_storage,
    clear_rpc_storage
};
use crate::components::modals::{WalletModal, RpcModal, SendModalWithHardware, HardwareWalletModal, ReceiveModal};
use crate::components::common::Token;
use crate::rpc;
use crate::transaction::TransactionClient;
use crate::hardware::HardwareWallet;
use std::sync::Arc;
use std::collections::HashMap;

// Define the assets for icons
const ICON_32: Asset = asset!("/assets/icons/32x32.png");
const ICON_SOL: Asset = asset!("/assets/icons/solanaLogo.png");
const ICON_USDC: Asset = asset!("/assets/icons/usdcLogo.png");
const ICON_USDT: Asset = asset!("/assets/icons/usdtLogo.png");

// JupiterToken struct with PartialEq and Eq for use_memo
#[derive(Clone, Debug, PartialEq, Eq)]
struct JupiterToken {
    address: String,
    name: String,
    symbol: String,
    logo_uri: String,
    tags: Vec<String>,
}

// Hardcoded verified tokens including USDC and USDT
fn get_verified_tokens() -> HashMap<String, JupiterToken> {
    let mut map = HashMap::new();
    map.insert(
        "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
        JupiterToken {
            address: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            name: "USD Coin".to_string(),
            symbol: "USDC".to_string(),
            logo_uri: "https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v/logo.png".to_string(),
            tags: vec!["stablecoin".to_string()],
        },
    );
    map.insert(
        "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB".to_string(),
        JupiterToken {
            address: "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB".to_string(),
            name: "Tether USD".to_string(),
            symbol: "USDT".to_string(),
            logo_uri: "https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB/logo.png".to_string(),
            tags: vec!["stablecoin".to_string()],
        },
    );
    map
}

/// Main wallet component
#[component]
pub fn WalletView() -> Element {
    // Wallet management
    let mut wallets = use_signal(|| Vec::<WalletInfo>::new());
    let mut current_wallet_index = use_signal(|| 0);
    let mut show_dropdown = use_signal(|| false);
    let mut show_wallet_modal = use_signal(|| false);
    let mut modal_mode = use_signal(|| "create".to_string());
    let mut show_rpc_modal = use_signal(|| false);
    let mut show_send_modal = use_signal(|| false);
    let mut show_receive_modal = use_signal(|| false);

    // Hardware wallet state
    let mut hardware_wallet = use_signal(|| None as Option<Arc<HardwareWallet>>);
    let mut show_hardware_modal = use_signal(|| false);
    let mut hardware_device_present = use_signal(|| false);
    let mut hardware_connected = use_signal(|| false);
    let mut hardware_pubkey = use_signal(|| None as Option<String>);

    // RPC management
    let mut custom_rpc = use_signal(|| load_rpc_from_storage());
    let mut rpc_input = use_signal(|| custom_rpc().unwrap_or_default());

    // Balance management
    let mut balance = use_signal(|| 0.0);
    let mut sol_price = use_signal(|| 50.0);
    let daily_change = use_signal(|| 42.13);
    let daily_change_percent = use_signal(|| 4.20);

    // Token management
    let mut tokens = use_signal(|| Vec::<Token>::new());

    // Verified tokens loaded with USDC and USDT
    let verified_tokens = use_memo(move || get_verified_tokens());

    // Load wallets from storage on component mount
    use_effect(move || {
        let stored_wallets = load_wallets_from_storage();
        if stored_wallets.is_empty() {
            let new_wallet = Wallet::new("Main Wallet".to_string());
            let wallet_info = new_wallet.to_wallet_info();
            save_wallet_to_storage(&wallet_info);
            wallets.set(vec![wallet_info]);
        } else {
            wallets.set(stored_wallets);
        }
    });

    // Monitor hardware wallet presence - check every 2 seconds
    use_effect(move || {
        spawn(async move {
            loop {
                let is_present = HardwareWallet::is_device_present();
                hardware_device_present.set(is_present);
                
                if !is_present && hardware_connected() {
                    hardware_connected.set(false);
                    hardware_wallet.set(None);
                    hardware_pubkey.set(None);
                }
                
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        });
    });

    // Fetch balance and token accounts when wallet changes or hardware wallet connects
    use_effect(move || {
        let wallets_list = wallets.read();
        let index = current_wallet_index();
        let hw_connected = hardware_connected();
        let hw_pubkey = hardware_pubkey();
        
        let address = if hw_connected && hw_pubkey.is_some() {
            hw_pubkey.clone().unwrap()
        } else if let Some(wallet) = wallets_list.get(index) {
            wallet.address.clone()
        } else {
            return;
        };
        
        let rpc_url = custom_rpc();
        
        // Clone verified_tokens for use in the async closure
        let verified_tokens_clone = verified_tokens.clone();
        
        spawn(async move {
            // Fetch SOL balance
            match rpc::get_balance(&address, rpc_url.as_deref()).await {
                Ok(sol_balance) => {
                    balance.set(sol_balance);
                    println!("Fetched SOL balance: {} SOL for address: {}", sol_balance, address);
                }
                Err(e) => {
                    println!("Failed to fetch balance for address {}: {}", address, e);
                    balance.set(0.0);
                }
            }
            
            // Fetch token accounts
            let filter = Some(rpc::TokenAccountFilter::ProgramId(
                "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string()
            ));
            match rpc::get_token_accounts_by_owner(&address, filter, rpc_url.as_deref()).await {
                Ok(token_accounts) => {
                    println!("Raw token accounts for address {}: {:?}", address, token_accounts);
                    
                    // Access the HashMap inside the Memo using read()
                    let verified_tokens_map = verified_tokens_clone.read();
                    
                    // Filter token accounts
                    let filtered_accounts: Vec<_> = token_accounts
                        .into_iter()
                        .filter(|account| {
                            let is_non_zero = account.amount > 0.0;
                            let is_verified = verified_tokens_map.contains_key(&account.mint);
                            println!(
                                "Token {}: amount={}, is_verified={}, will_include={}",
                                account.mint,
                                account.amount,
                                is_verified,
                                is_non_zero && is_verified
                            );
                            is_non_zero && is_verified
                        })
                        .collect();
                    
                    println!("Filtered token accounts: {:?}", filtered_accounts);
                    
                    let new_tokens = filtered_accounts
                        .into_iter()
                        .map(|account| {
                            let metadata = verified_tokens_map.get(&account.mint).unwrap();
                            let price = match metadata.symbol.as_str() {
                                "USDC" => 1.0,
                                "USDT" => 1.0,
                                _ => 1.0,
                            };
                            let value_usd = account.amount * price;
                            let icon_type = match metadata.symbol.as_str() {
                                "USDC" => ICON_USDC.to_string(),
                                "USDT" => ICON_USDT.to_string(),
                                _ => ICON_32.to_string(),
                            };
                            Token {
                                mint: account.mint.clone(),
                                symbol: metadata.symbol.clone(),
                                name: metadata.name.clone(),
                                icon_type,
                                balance: account.amount,
                                value_usd,
                                price,
                                price_change: 0.0,
                            }
                        })
                        .collect::<Vec<Token>>();
                    
                    println!("Processed tokens for address {}: {:?}", address, new_tokens);
                    
                    let mut all_tokens = vec![Token {
                        mint: "So11111111111111111111111111111111111111112".to_string(),
                        symbol: "SOL".to_string(),
                        name: "Solana".to_string(),
                        icon_type: ICON_SOL.to_string(),
                        balance: balance(),
                        value_usd: balance() * sol_price(),
                        price: sol_price(),
                        price_change: 0.0,
                    }];
                    all_tokens.extend(new_tokens);
                    
                    tokens.set(all_tokens);
                }
                Err(e) => {
                    println!("Failed to fetch token accounts for address {}: {}", address, e);
                    tokens.set(vec![Token {
                        mint: "So11111111111111111111111111111111111111112".to_string(),
                        symbol: "SOL".to_string(),
                        name: "Solana".to_string(),
                        icon_type: ICON_SOL.to_string(),
                        balance: balance(),
                        value_usd: balance() * sol_price(),
                        price: sol_price(),
                        price_change: 0.0,
                    }]);
                }
            }
        });
    });

    let current_wallet = wallets.read().get(current_wallet_index()).cloned();
    
    // Get full address for display
    let full_address = if hardware_connected() && hardware_pubkey().is_some() {
        hardware_pubkey().unwrap()
    } else if let Some(wallet) = current_wallet.as_ref() {
        wallet.address.clone()
    } else {
        "No Wallet".to_string()
    };

    // Truncated address for dropdown
    let wallet_address = if hardware_connected() && hardware_pubkey().is_some() {
        let addr = hardware_pubkey().unwrap();
        if addr.len() >= 8 {
            format!("{}...{}", &addr[..4], &addr[addr.len()-4..])
        } else {
            addr
        }
    } else if let Some(wallet) = current_wallet.as_ref() {
        let addr = &wallet.address;
        if addr.len() >= 8 {
            format!("{}...{}", &addr[..4], &addr[addr.len()-4..])
        } else {
            addr.clone()
        }
    } else {
        "No wallet".to_string()
    };

    // Calculate USD value
    let usd_balance = balance() * sol_price();

    rsx! {
        div {
            class: "wallet-container",
            onclick: move |_| {
                if show_dropdown() {
                    show_dropdown.set(false);
                }
            },
            
            // Header
            div {
                class: "wallet-header",
                div {
                    class: format!(
                        "profile-icon {} {}", 
                        if hardware_device_present() { "hardware-present" } else { "" },
                        if hardware_connected() { "hardware-connected" } else { "" }
                    ),
                    onclick: move |e| {
                        e.stop_propagation();
                        if hardware_device_present() {
                            show_hardware_modal.set(true);
                        }
                    },
                    img { 
                        src: ICON_32,
                        alt: "Profile"
                    }
                    if hardware_device_present() {
                        div {
                            class: if hardware_connected() { 
                                "hardware-indicator connected" 
                            } else { 
                                "hardware-indicator present" 
                            }
                        }
                    }
                }
                div {
                    class: "menu-icon",
                    onclick: move |e| {
                        e.stop_propagation();
                        show_dropdown.set(!show_dropdown());
                    }
                }

                if show_dropdown() {
                    div {
                        class: "dropdown-menu",
                        onclick: move |e| e.stop_propagation(),
                        
                        if let Some(ref wallet) = current_wallet {
                            div {
                                class: "dropdown-item current-wallet",
                                div {
                                    class: "dropdown-icon wallet-icon",
                                    "💼"
                                }
                                div {
                                    class: "wallet-info",
                                    div { class: "wallet-name", "{wallet.name}" }
                                    div { class: "wallet-address", "{wallet_address}" }
                                }
                            }
                        }
                        
                        if hardware_connected() && hardware_pubkey().is_some() {
                            div {
                                class: "dropdown-item hardware-wallet-item active",
                                onclick: move |_| {
                                    show_hardware_modal.set(true);
                                    show_dropdown.set(false);
                                },
                                div {
                                    class: "dropdown-icon hardware-icon",
                                    "🔐"
                                }
                                div {
                                    class: "wallet-info",
                                    div { class: "wallet-name", "Hardware Wallet" }
                                    div { 
                                        class: "wallet-address",
                                        {
                                            match hardware_pubkey() {
                                                Some(pubkey) => {
                                                    if pubkey.len() >= 8 {
                                                        format!("{}...{}", &pubkey[..4], &pubkey[pubkey.len()-4..])
                                                    } else {
                                                        pubkey
                                                    }
                                                },
                                                None => "Connecting...".to_string()
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        
                        div { class: "dropdown-divider" }
                        
                        for (index, wallet) in wallets.read().iter().enumerate() {
                            button {
                                class: if index == current_wallet_index() { 
                                    "dropdown-item wallet-list-item active" 
                                } else { 
                                    "dropdown-item wallet-list-item" 
                                },
                                onclick: move |_| {
                                    current_wallet_index.set(index);
                                    show_dropdown.set(false);
                                    hardware_connected.set(false);
                                    hardware_pubkey.set(None);
                                },
                                div {
                                    class: "dropdown-icon",
                                    "💗"
                                }
                                div {
                                    class: "wallet-info",
                                    div { class: "wallet-name", "{wallet.name}" }
                                    div { 
                                        class: "wallet-address",
                                        {
                                            let addr = &wallet.address;
                                            if addr.len() >= 8 {
                                                format!("{}...{}", &addr[..4], &addr[addr.len()-4..])
                                            } else {
                                                addr.clone()
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        
                        div { class: "dropdown-divider" }
                        
                        button {
                            class: "dropdown-item",
                            onclick: move |_| {
                                modal_mode.set("create".to_string());
                                show_wallet_modal.set(true);
                                show_dropdown.set(false);
                            },
                            div {
                                class: "dropdown-icon action-icon",
                                "➕"
                            }
                            "Create Wallet"
                        }
                        button {
                            class: "dropdown-item",
                            onclick: move |_| {
                                modal_mode.set("import".to_string());
                                show_wallet_modal.set(true);
                                show_dropdown.set(false);
                            },
                            div {
                                class: "dropdown-icon action-icon",
                                "📥"
                            }
                            "Import Wallet"
                        }
                        
                        if hardware_device_present() && !hardware_connected() {
                            button {
                                class: "dropdown-item",
                                onclick: move |_| {
                                    show_hardware_modal.set(true);
                                    show_dropdown.set(false);
                                },
                                div {
                                    class: "dropdown-icon action-icon",
                                    "🔐"
                                }
                                "Connect Hardware Wallet"
                            }
                        }
                        
                        div { class: "dropdown-divider" }
                        
                        button {
                            class: "dropdown-item",
                            onclick: move |_| {
                                show_rpc_modal.set(true);
                                show_dropdown.set(false);
                            },
                            div {
                                class: "dropdown-icon action-icon",
                                "🔗"
                            }
                            "RPC Settings"
                        }
                        button {
                            class: "dropdown-item",
                            onclick: move |_| {
                                show_dropdown.set(false);
                            },
                            div {
                                class: "dropdown-icon action-icon",
                                "⚙️"
                            }
                            "Settings"
                        }
                        button {
                            class: "dropdown-item",
                            onclick: move |_| {
                                show_dropdown.set(false);
                            },
                            div {
                                class: "dropdown-icon action-icon",
                                "🔒"
                            }
                            "Security"
                        }
                        button {
                            class: "dropdown-item",
                            onclick: move |_| {
                                wallets.set(vec![]);
                                current_wallet_index.set(0);
                                show_dropdown.set(false);
                                hardware_connected.set(false);
                                hardware_pubkey.set(None);
                                #[cfg(not(feature = "web"))]
                                {
                                    let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                                    let wallet_file = format!("{home_dir}/.solana_wallet_app/wallets.json");
                                    std::fs::remove_file(wallet_file).ok();
                                }
                            },
                            div {
                                class: "dropdown-icon action-icon",
                                "🚪"
                            }
                            "Logout"
                        }
                    }
                }
            }
            
            if show_wallet_modal() {
                WalletModal {
                    mode: modal_mode(),
                    onclose: move |_| show_wallet_modal.set(false),
                    onsave: move |wallet_info| {
                        save_wallet_to_storage(&wallet_info);
                        wallets.write().push(wallet_info);
                        current_wallet_index.set(wallets.read().len() - 1);
                        show_wallet_modal.set(false);
                    }
                }
            }
            
            if show_rpc_modal() {
                RpcModal {
                    current_rpc: custom_rpc(),
                    onclose: move |_| show_rpc_modal.set(false),
                    onsave: move |new_rpc: String| {
                        if new_rpc.is_empty() {
                            custom_rpc.set(None);
                            clear_rpc_storage();
                        } else {
                            custom_rpc.set(Some(new_rpc.clone()));
                            save_rpc_to_storage(&new_rpc);
                        }
                        show_rpc_modal.set(false);
                        
                        if let Some(wallet) = wallets.read().get(current_wallet_index()) {
                            let address = wallet.address.clone();
                            let rpc_url = custom_rpc();
                            
                            spawn(async move {
                                match rpc::get_balance(&address, rpc_url.as_deref()).await {
                                    Ok(sol_balance) => {
                                        balance.set(sol_balance);
                                    }
                                    Err(e) => {
                                        println!("Failed to fetch balance: {}", e);
                                        balance.set(0.0);
                                    }
                                }
                            });
                        }
                    }
                }
            }

            if show_hardware_modal() {
                HardwareWalletModal {
                    onclose: move |_| show_hardware_modal.set(false),
                    existing_wallet: if hardware_connected() { hardware_wallet() } else { None },
                    ondisconnect: move |_| {
                        hardware_wallet.set(None);
                        hardware_connected.set(false);
                        hardware_pubkey.set(None);
                        show_hardware_modal.set(false);
                    },
                    onsuccess: move |hw_wallet: Arc<HardwareWallet>| {
                        hardware_wallet.set(Some(hw_wallet.clone()));
                        hardware_connected.set(true);
                        show_hardware_modal.set(false);
                        
                        spawn(async move {
                            if let Ok(pubkey) = hw_wallet.get_public_key().await {
                                hardware_pubkey.set(Some(pubkey));
                            }
                        });
                    }
                }
            }
            
            if show_send_modal() {
                SendModalWithHardware {
                    wallet: current_wallet.clone(),
                    hardware_wallet: hardware_wallet(),
                    current_balance: balance(),
                    custom_rpc: custom_rpc(),
                    onclose: move |_| {
                        show_send_modal.set(false);
                        hardware_wallet.set(None);
                    },
                    onsuccess: move |_| {
                        show_send_modal.set(false);
                        hardware_wallet.set(None);
                        if let Some(wallet) = wallets.read().get(current_wallet_index()) {
                            let address = wallet.address.clone();
                            let rpc_url = custom_rpc();
                            
                            spawn(async move {
                                match rpc::get_balance(&address, rpc_url.as_deref()).await {
                                    Ok(sol_balance) => {
                                        balance.set(sol_balance);
                                    }
                                    Err(e) => {
                                        println!("Failed to fetch balance: {}", e);
                                        balance.set(0.0);
                                    }
                                }
                            });
                        }
                    }
                }
            }
            
            if show_receive_modal() {
                ReceiveModal {
                    wallet: current_wallet.clone(),
                    hardware_wallet: hardware_wallet(),
                    onclose: move |_| show_receive_modal.set(false)
                }
            }
            
            // Main content container for balance, address, and actions
            div {
                class: "main-content",
                div {
                    class: "balance-section",
                    div {
                        class: "balance-amount",
                        "${usd_balance:.2}"
                    }
                    div {
                        class: "balance-change",
                        span {
                            class: "change-positive",
                            "+${daily_change:.2}"
                        }
                        span {
                            class: "change-positive",
                            "+{daily_change_percent:.2}%"
                        }
                    }
                    div {
                        class: "balance-sol",
                        "{balance:.4} SOL"
                    }
                }
                
                // Display current wallet address without copy button
                div {
                    class: "current-address-section",
                    div {
                        class: "address-label",
                        if hardware_connected() && hardware_pubkey().is_some() {
                            "Hardware Wallet Address"
                        } else if current_wallet.is_some() {
                            "Wallet Address"
                        } else {
                            "No Wallet Connected"
                        }
                    }
                    div {
                        class: "current-address",
                        if full_address != "No Wallet" {
                            "{full_address}"
                        } else {
                            "---"
                        }
                    }
                }
                
                div {
                    class: "action-buttons",
                    button {
                        class: "action-button",
                        onclick: move |_| show_receive_modal.set(true),
                        div {
                            class: "action-icon",
                            "💰"
                        }
                        span {
                            class: "action-label",
                            "Receive"
                        }
                    }
                    button {
                        class: "action-button",
                        onclick: move |_| show_send_modal.set(true),
                        div {
                            class: "action-icon",
                            "💸"
                        }
                        span {
                            class: "action-label",
                            "Send"
                        }
                    }
                    button {
                        class: "action-button",
                        div {
                            class: "action-icon",
                            "🔄"
                        }
                        span {
                            class: "action-label",
                            "Swap"
                        }
                    }
                }
            }
            
            div {
                class: "tokens-section",
                h3 {
                    class: "tokens-header",
                    "Your Tokens"
                }
                div {
                    class: "token-list",
                    for token in tokens() {
                        div {
                            key: "{token.mint}",
                            class: "token-item",
                            div {
                                class: "token-info",
                                div {
                                    class: "token-icon",
                                    img {
                                        src: "{token.icon_type}",
                                        alt: "{token.symbol}",
                                        width: "24",
                                        height: "24",
                                        onerror: move |_| {
                                            println!("Failed to load image for {}: {}", token.symbol, token.icon_type);
                                        },
                                    }
                                }
                                div {
                                    class: "token-details",
                                    div {
                                        class: "token-name",
                                        "{token.name} ({token.symbol})"
                                    }
                                    div {
                                        class: "token-price-info",
                                        span {
                                            class: "token-price",
                                            "${token.price:.2}"
                                        }
                                        span {
                                            class: "token-change positive",
                                            "+{token.price_change:.1}%"
                                        }
                                    }
                                }
                            }
                            div {
                                class: "token-values",
                                div {
                                    class: "token-value-usd",
                                    "${token.value_usd:.2}"
                                }
                                div {
                                    class: "token-amount",
                                    "{token.balance:.2} {token.symbol}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}