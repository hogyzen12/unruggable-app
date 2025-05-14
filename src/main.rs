use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

mod wallet;
mod rpc;
mod transaction;
use wallet::{Wallet, WalletInfo};
use transaction::TransactionClient;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[route("/")]
    WalletView {},
}

const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");
const MAIN_CSS: Asset = asset!("/assets/main.css");
const ICON_32: Asset = asset!("/assets/icons/32x32.png");

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        Router::<Route> {}
    }
}

/// Token structure for wallet holdings
#[derive(Clone, Debug)]
struct Token {
    symbol: String,
    name: String,
    icon_type: String,
    balance: f64,
    value_usd: f64,
    price: f64,
    price_change: f64,
}

/// Main wallet component
#[component]
fn WalletView() -> Element {
    // Wallet management
    let mut wallets = use_signal(|| Vec::<WalletInfo>::new());
    let mut current_wallet_index = use_signal(|| 0);
    let mut show_dropdown = use_signal(|| false);
    let mut show_wallet_modal = use_signal(|| false);
    let mut modal_mode = use_signal(|| "create".to_string());
    let mut show_rpc_modal = use_signal(|| false);
    let mut show_send_modal = use_signal(|| false);
    
    // RPC management
    let mut custom_rpc = use_signal(|| load_rpc_from_storage());
    let mut rpc_input = use_signal(|| custom_rpc().unwrap_or_default());
    
    // Balance management
    let mut balance = use_signal(|| 0.0);
    let mut sol_price = use_signal(|| 50.0); // Mock SOL price
    let daily_change = use_signal(|| 42.13);
    let daily_change_percent = use_signal(|| 4.20);
    
    // Load wallets from storage on component mount
    use_effect(move || {
        let stored_wallets = load_wallets_from_storage();
        if stored_wallets.is_empty() {
            // Create a default wallet if none exist
            let new_wallet = Wallet::new("Main Wallet".to_string());
            let wallet_info = new_wallet.to_wallet_info();
            save_wallet_to_storage(&wallet_info);
            wallets.set(vec![wallet_info]);
        } else {
            wallets.set(stored_wallets);
        }
    });
    
    // Fetch balance when wallet changes
    use_effect(move || {
        let wallets_list = wallets.read();
        let index = current_wallet_index();
        
        if let Some(wallet) = wallets_list.get(index) {
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
    });
    
    let current_wallet = wallets.read().get(current_wallet_index()).cloned();
    let wallet_address = current_wallet
        .as_ref()
        .map(|w| {
            let addr = &w.address;
            if addr.len() >= 8 {
                format!("{}...{}", &addr[..4], &addr[addr.len()-4..])
            } else {
                addr.clone()
            }
        })
        .unwrap_or_else(|| "No wallet".to_string());
    
    // Calculate USD value
    let usd_balance = balance() * sol_price();
    
    // Mock token holdings (with SOL balance)
    let tokens = use_signal(move || vec![
        Token {
            symbol: "SOL".to_string(),
            name: "Solana".to_string(),
            icon_type: "solana".to_string(),
            balance: balance(),
            value_usd: usd_balance,
            price: sol_price(),
            price_change: 0.9,
        },
        Token {
            symbol: "USDC".to_string(),
            name: "USDC".to_string(),
            icon_type: "usdc".to_string(),
            balance: 843.25,
            value_usd: 843.25,
            price: 1.00,
            price_change: 0.01,
        },
        Token {
            symbol: "BONK".to_string(),
            name: "Bonk".to_string(),
            icon_type: "bonk".to_string(),
            balance: 60000.00,
            value_usd: 1073.10,
            price: 0.000018,
            price_change: 13.0,
        },
        Token {
            symbol: "JUP".to_string(),
            name: "Jup".to_string(),
            icon_type: "jup".to_string(),
            balance: 1.25,
            value_usd: 700.40,
            price: 1.25,
            price_change: 2.4,
        },
    ]);

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
                    class: "profile-icon",
                    img { 
                        src: ICON_32,
                        alt: "Profile"
                    }
                }
                div {
                    class: "menu-icon",
                    onclick: move |e| {
                        e.stop_propagation();
                        show_dropdown.set(!show_dropdown());
                    }
                }

                // Dropdown menu
                if show_dropdown() {
                    div {
                        class: "dropdown-menu",
                        onclick: move |e| e.stop_propagation(),
                        
                        // Current wallet display
                        if let Some(wallet) = &current_wallet {
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
                        
                        div { class: "dropdown-divider" }
                        
                        // Wallet list
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
                                // Handle settings
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
                                // Handle security
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
                                // Handle logout - clear all wallets and reset
                                wallets.set(vec![]);
                                current_wallet_index.set(0);
                                show_dropdown.set(false);
                                // Clear storage
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
            
            // Wallet Modal
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
            
            // RPC Modal
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
                        
                        // Refresh balance with new RPC
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
            
            // Send Modal
            if show_send_modal() {
                SendModal {
                    wallet: current_wallet.clone(),
                    current_balance: balance(),
                    custom_rpc: custom_rpc(),
                    onclose: move |_| show_send_modal.set(false),
                    onsuccess: move |_| {
                        show_send_modal.set(false);
                        // Refresh balance after successful transaction
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
            
            // Balance section
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
            
            // Action buttons
            div {
                class: "action-buttons",
                button {
                    class: "action-button",
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
                button {
                    class: "action-button",
                    div {
                        class: "action-icon",
                        "💳"
                    }
                    span {
                        class: "action-label",
                        "Buy"
                    }
                }
            }
            
            // Tokens section
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
                            key: "{token.symbol}",
                            class: "token-item",
                            div {
                                class: "token-info",
                                div {
                                    class: "token-icon {token.icon_type}",
                                    match token.icon_type.as_str() {
                                        "solana" => "⚡",
                                        "usdc" => "$",
                                        "bonk" => "🐕",
                                        "jup" => "🌊",
                                        _ => "?"
                                    }
                                }
                                div {
                                    class: "token-details",
                                    div {
                                        class: "token-name",
                                        "{token.name}"
                                    }
                                    div {
                                        class: "token-price-info",
                                        span {
                                            class: "token-price",
                                            "${token.price:.2}"
                                        }
                                        span {
                                            class: if token.price_change >= 0.0 { "token-change positive" } else { "token-change negative" },
                                            if token.price_change >= 0.0 { "+" } else { "" }
                                            "{token.price_change:.1}%"
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
                                    "{token.balance:.2}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// RPC Modal Component
#[component]
fn RpcModal(current_rpc: Option<String>, onclose: EventHandler<()>, onsave: EventHandler<String>) -> Element {
    let mut rpc_url = use_signal(|| current_rpc.clone().unwrap_or_default());
    let mut error_message = use_signal(|| None as Option<String>);
    let mut testing = use_signal(|| false);
    
    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| onclose.call(()),
            
            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                
                h2 { class: "modal-title", "RPC Settings" }
                
                // Show error if any
                if let Some(error) = error_message() {
                    div {
                        class: "error-message",
                        "{error}"
                    }
                }
                
                div {
                    class: "wallet-field",
                    label { "RPC URL:" }
                    input {
                        value: "{rpc_url}",
                        oninput: move |e| rpc_url.set(e.value()),
                        placeholder: "https://your-rpc-url.com"
                    }
                    div {
                        class: "help-text",
                        "Leave empty to use default RPC"
                    }
                }
                
                if let Some(current) = current_rpc {
                    div {
                        class: "info-message",
                        "Current RPC: {current}"
                    }
                }
                
                div { class: "modal-buttons",
                    button {
                        class: "modal-button cancel",
                        onclick: move |_| onclose.call(()),
                        "Cancel"
                    }
                    button {
                        class: "modal-button secondary",
                        onclick: move |_| {
                            testing.set(true);
                            error_message.set(None);
                            let test_rpc = rpc_url();
                            
                            spawn(async move {
                                // Test the RPC with a known address
                                match rpc::get_balance("11111111111111111111111111111111", 
                                    if test_rpc.is_empty() { None } else { Some(&test_rpc) }).await {
                                    Ok(_) => {
                                        error_message.set(None);
                                        testing.set(false);
                                    }
                                    Err(e) => {
                                        error_message.set(Some(format!("RPC test failed: {}", e)));
                                        testing.set(false);
                                    }
                                }
                            });
                        },
                        disabled: testing(),
                        if testing() { "Testing..." } else { "Test RPC" }
                    }
                    button {
                        class: "modal-button primary",
                        onclick: move |_| {
                            onsave.call(rpc_url());
                        },
                        "Save"
                    }
                }
            }
        }
    }
}

// Send Modal Component
#[component]
fn SendModal(
    wallet: Option<WalletInfo>,
    current_balance: f64,
    custom_rpc: Option<String>,
    onclose: EventHandler<()>,
    onsuccess: EventHandler<String>,
) -> Element {
    let mut recipient = use_signal(|| "".to_string());
    let mut amount = use_signal(|| "".to_string());
    let mut sending = use_signal(|| false);
    let mut error_message = use_signal(|| None as Option<String>);
    
    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| onclose.call(()),
            
            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                
                h2 { class: "modal-title", "Send SOL" }
                
                // Show error if any
                if let Some(error) = error_message() {
                    div {
                        class: "error-message",
                        "{error}"
                    }
                }
                
                div {
                    class: "wallet-field",
                    label { "Available Balance:" }
                    div { class: "balance-display", "{current_balance:.4} SOL" }
                }
                
                div {
                    class: "wallet-field",
                    label { "Recipient Address:" }
                    input {
                        value: "{recipient}",
                        oninput: move |e| recipient.set(e.value()),
                        placeholder: "Enter Solana address"
                    }
                }
                
                div {
                    class: "wallet-field",
                    label { "Amount (SOL):" }
                    input {
                        r#type: "number",
                        value: "{amount}",
                        oninput: move |e| amount.set(e.value()),
                        placeholder: "0.0",
                        step: "0.0001",
                        min: "0"
                    }
                }
                
                div { class: "modal-buttons",
                    button {
                        class: "modal-button cancel",
                        onclick: move |_| onclose.call(()),
                        "Cancel"
                    }
                    button {
                        class: "modal-button primary",
                        onclick: move |_| {
                            if let Some(wallet_info) = &wallet {
                                error_message.set(None);
                                sending.set(true);
                                
                                let wallet_info = wallet_info.clone();
                                let recipient_address = recipient();
                                let amount_str = amount();
                                let rpc_url = custom_rpc.clone();
                                
                                spawn(async move {
                                    // Validate inputs
                                    let amount_value = match amount_str.parse::<f64>() {
                                        Ok(amt) if amt > 0.0 => amt,
                                        _ => {
                                            error_message.set(Some("Invalid amount".to_string()));
                                            sending.set(false);
                                            return;
                                        }
                                    };
                                    
                                    if amount_value > current_balance {
                                        error_message.set(Some("Insufficient balance".to_string()));
                                        sending.set(false);
                                        return;
                                    }
                                    
                                    // Validate recipient address
                                    if let Err(e) = bs58::decode(&recipient_address).into_vec() {
                                        error_message.set(Some(format!("Invalid recipient address: {}", e)));
                                        sending.set(false);
                                        return;
                                    }
                                    
                                    // Load wallet from wallet info
                                    match Wallet::from_wallet_info(&wallet_info) {
                                        Ok(wallet) => {
                                            let client = TransactionClient::new(rpc_url.as_deref());
                                            
                                            // Send transaction with amount in SOL
                                            match client.send_sol(&wallet, &recipient_address, amount_value).await {
                                                Ok(signature) => {
                                                    println!("Transaction sent: {}", signature);
                                                    onsuccess.call(signature);
                                                }
                                                Err(e) => {
                                                    error_message.set(Some(format!("Transaction failed: {}", e)));
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            error_message.set(Some(format!("Failed to load wallet: {}", e)));
                                        }
                                    }
                                    
                                    sending.set(false);
                                });
                            }
                        },
                        disabled: sending() || recipient().is_empty() || amount().is_empty(),
                        if sending() { "Sending..." } else { "Send" }
                    }
                }
            }
        }
    }
}

// Wallet Modal Component
#[component]
fn WalletModal(mode: String, onclose: EventHandler<()>, onsave: EventHandler<WalletInfo>) -> Element {
    let mut wallet_name = use_signal(|| "".to_string());
    let mut import_key = use_signal(|| "".to_string());
    let mut show_generated_key = use_signal(|| false);
    let mut generated_wallet = use_signal(|| None as Option<Wallet>);
    let mut error_message = use_signal(|| None as Option<String>);
    
    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| onclose.call(()),
            
            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                
                h2 { class: "modal-title",
                    if mode == "create" { "Create New Wallet" } else { "Import Wallet" }
                }
                
                // Show error if any
                if let Some(error) = error_message() {
                    div {
                        class: "error-message",
                        "{error}"
                    }
                }
                
                if mode == "create" {
                    if let Some(wallet) = generated_wallet() {
                        // Show generated wallet details
                        div {
                            class: "generated-wallet",
                            div { class: "wallet-field",
                                label { "Wallet Name:" }
                                input {
                                    value: "{wallet_name}",
                                    oninput: move |e| wallet_name.set(e.value()),
                                    placeholder: "My Wallet"
                                }
                            }
                            div { class: "wallet-field",
                                label { "Public Address:" }
                                div { class: "address-display", "{wallet.get_public_key()}" }
                            }
                            div { class: "wallet-field",
                                label { "Private Key:" }
                                div { class: "private-key-warning",
                                    "⚠️ Keep this safe! Never share it with anyone!"
                                }
                                if show_generated_key() {
                                    div { class: "private-key-display", 
                                        "{wallet.get_private_key()}"
                                    }
                                    div { 
                                        class: "key-format-info",
                                        "Solana Keypair (64 bytes) - Compatible with Solana CLI and other wallets"
                                    }
                                    
                                    // Optionally show just the private key too
                                    div { 
                                        class: "private-key-section",
                                        label { "Private Key Only (32 bytes):" }
                                        div { class: "private-key-display", 
                                            "{wallet.get_private_key_only()}"
                                        }
                                    }
                                    div { 
                                        class: "copy-hint",
                                        "Make sure to copy this key before saving!"
                                    }
                                } else {
                                    button {
                                        class: "show-key-button",
                                        onclick: move |_| show_generated_key.set(true),
                                        "Show Private Key"
                                    }
                                }
                            }
                        }
                    } else {
                        div {
                            class: "wallet-field",
                            label { "Wallet Name:" }
                            input {
                                value: "{wallet_name}",
                                oninput: move |e| wallet_name.set(e.value()),
                                placeholder: "My Wallet"
                            }
                        }
                        div {
                            class: "info-message",
                            "Click 'Generate Wallet' to create a new wallet"
                        }
                    }
                } else {
                    // Import mode
                    div {
                        class: "wallet-field",
                        label { "Wallet Name:" }
                        input {
                            value: "{wallet_name}",
                            oninput: move |e| wallet_name.set(e.value()),
                            placeholder: "Imported Wallet"
                        }
                    }
                    div {
                        class: "wallet-field",
                        label { "Private Key:" }
                        textarea {
                            value: "{import_key}",
                            oninput: move |e| import_key.set(e.value()),
                            placeholder: "Enter your base58 encoded private key or Solana keypair"
                        }
                        div {
                            class: "help-text",
                            "Supports both 32-byte private keys and 64-byte Solana keypairs"
                        }
                    }
                }
                
                div { class: "modal-buttons",
                    button {
                        class: "modal-button cancel",
                        onclick: move |_| onclose.call(()),
                        "Cancel"
                    }
                    if mode == "create" {
                        if generated_wallet().is_none() {
                            button {
                                class: "modal-button primary",
                                onclick: move |_| {
                                    let new_wallet = Wallet::new(
                                        if wallet_name().is_empty() { 
                                            "New Wallet".to_string() 
                                        } else { 
                                            wallet_name() 
                                        }
                                    );
                                    generated_wallet.set(Some(new_wallet));
                                },
                                "Generate Wallet"
                            }
                        } else {
                            button {
                                class: "modal-button primary",
                                onclick: move |_| {
                                    if let Some(wallet) = generated_wallet() {
                                        let mut wallet_info = wallet.to_wallet_info();
                                        wallet_info.name = if wallet_name().is_empty() {
                                            wallet.name.clone()
                                        } else {
                                            wallet_name()
                                        };
                                        onsave.call(wallet_info);
                                    }
                                },
                                disabled: !show_generated_key(),
                                if !show_generated_key() {
                                    "Show Private Key First"
                                } else {
                                    "Save Wallet"
                                }
                            }
                        }
                    } else {
                        button {
                            class: "modal-button primary",
                            onclick: move |_| {
                                if !import_key().is_empty() {
                                    match import_wallet_from_key(&import_key(), wallet_name()) {
                                        Ok(wallet_info) => onsave.call(wallet_info),
                                        Err(e) => {
                                            error_message.set(Some(e));
                                        }
                                    }
                                } else {
                                    error_message.set(Some("Please enter a private key".to_string()));
                                }
                            },
                            "Import"
                        }
                    }
                }
            }
        }
    }
}

// Helper functions
fn save_wallet_to_storage(wallet_info: &WalletInfo) {
    let mut wallets = load_wallets_from_storage();
    wallets.push(wallet_info.clone());
    
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        let serialized = serde_json::to_string(&wallets).unwrap();
        storage.set_item("wallets", &serialized).unwrap();
    }
    
    #[cfg(not(feature = "web"))]
    {
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let wallet_file = format!("{home_dir}/.solana_wallet_app/wallets.json");
        std::fs::create_dir_all(format!("{home_dir}/.solana_wallet_app")).ok();
        std::fs::write(wallet_file, serde_json::to_string(&wallets).unwrap()).ok();
    }
}

fn load_wallets_from_storage() -> Vec<WalletInfo> {
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        storage.get_item("wallets")
            .unwrap()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default()
    }
    
    #[cfg(not(feature = "web"))]
    {
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let wallet_file = format!("{home_dir}/.solana_wallet_app/wallets.json");
        std::fs::read_to_string(wallet_file)
            .ok()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default()
    }
}

fn import_wallet_from_key(private_key: &str, name: String) -> Result<WalletInfo, String> {
    let private_key = private_key.trim();
    
    // Try to decode the base58 key
    let key_bytes = bs58::decode(private_key)
        .into_vec()
        .map_err(|e| format!("Invalid base58 format: {}", e))?;
    
    // Create wallet with proper name
    let wallet_name = if name.is_empty() { 
        "Imported Wallet".to_string() 
    } else { 
        name 
    };
    
    let wallet = Wallet::from_private_key(&key_bytes, wallet_name)?;
    
    Ok(wallet.to_wallet_info())
}

// Add RPC storage functions
fn save_rpc_to_storage(rpc_url: &str) {
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        storage.set_item("custom_rpc", rpc_url).unwrap();
    }
    
    #[cfg(not(feature = "web"))]
    {
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let rpc_file = format!("{home_dir}/.solana_wallet_app/rpc.txt");
        std::fs::create_dir_all(format!("{home_dir}/.solana_wallet_app")).ok();
        std::fs::write(rpc_file, rpc_url).ok();
    }
}

fn load_rpc_from_storage() -> Option<String> {
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        storage.get_item("custom_rpc").unwrap()
    }
    
    #[cfg(not(feature = "web"))]
    {
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let rpc_file = format!("{home_dir}/.solana_wallet_app/rpc.txt");
        std::fs::read_to_string(rpc_file).ok()
    }
}

fn clear_rpc_storage() {
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        storage.remove_item("custom_rpc").unwrap();
    }
    
    #[cfg(not(feature = "web"))]
    {
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let rpc_file = format!("{home_dir}/.solana_wallet_app/rpc.txt");
        std::fs::remove_file(rpc_file).ok();
    }
}
