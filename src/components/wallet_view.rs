use dioxus::prelude::*;
use crate::wallet::{Wallet, WalletInfo};
use crate::storage::{
    load_wallets_from_storage, 
    save_wallet_to_storage, 
    load_rpc_from_storage,
    save_rpc_to_storage,
    clear_rpc_storage,
    load_jito_settings_from_storage,
    save_jito_settings_to_storage,
    JitoSettings
};
use crate::currency::{
    SELECTED_CURRENCY, 
    EXCHANGE_RATES,
    initialize_currency_system,
    update_exchange_rates_loop,
    get_current_currency_symbol
};
use crate::currency_utils::{
    format_price_in_selected_currency,
    format_balance_value,
    format_token_value,
    format_token_value_smart,
    format_token_amount, 
    format_price_change,
    get_current_currency_code
};
use crate::components::modals::currency_modal::CurrencyModal;
use crate::components::modals::{WalletModal, RpcModal, SendModalWithHardware, SendTokenModal, HardwareWalletModal, ReceiveModal, JitoModal, StakeModal};
use crate::components::modals::send_modal::HardwareWalletEvent;
use crate::components::common::Token;
use crate::rpc;
use crate::prices;
use crate::hardware::HardwareWallet;
use crate::components::background_themes::BackgroundTheme;
use crate::components::modals::BackgroundModal;
use std::sync::Arc;
use std::collections::HashMap;
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
use arboard::Clipboard as SystemClipboard;

// Define the assets for icons
const ICON_32: Asset = asset!("/assets/icons/32x32.png");
const ICON_SOL: Asset = asset!("/assets/icons/solanaLogo.png");
const ICON_USDC: Asset = asset!("/assets/icons/usdcLogo.png");
const ICON_USDT: Asset = asset!("/assets/icons/usdtLogo.png");
const ICON_JTO: Asset = asset!("/assets/icons/jtoLogo.png");
const ICON_JUP: Asset = asset!("/assets/icons/jupLogo.png");
const ICON_JLP: Asset = asset!("/assets/icons/jlpLogo.png");
const ICON_BONK: Asset = asset!("/assets/icons/bonkLogo.png");

// Action button SVG icons
const ICON_RECEIVE: Asset = asset!("/assets/icons/receive.svg");
const ICON_SEND: Asset = asset!("/assets/icons/send.svg");
const ICON_STAKE: Asset = asset!("/assets/icons/stake.svg");

// JupiterToken struct with PartialEq and Eq for use_memo
#[derive(Clone, Debug, PartialEq, Eq)]
struct JupiterToken {
    address: String,
    name: String,
    symbol: String,
    logo_uri: String,
    tags: Vec<String>,
}

// Helper function to fetch token prices from the Pyth Network
async fn fetch_token_prices(
    mut token_prices: Signal<HashMap<String, f64>>,
    mut prices_loading: Signal<bool>,
    mut price_error: Signal<Option<String>>,
    mut sol_price: Signal<f64>,
    mut daily_change: Signal<f64>,
    mut daily_change_percent: Signal<f64>,
) {
    prices_loading.set(true);
    price_error.set(None);

    // Call the prices module to fetch current prices
    match prices::get_prices().await {
        Ok(prices) => {
            // Update token prices
            token_prices.set(prices.clone());

            // Update SOL price and calculate change values
            if let Some(new_sol_price) = prices.get("SOL") {
                // Calculate absolute change - comparing to the previous price
                let old_price = sol_price();
                let price_diff = new_sol_price - old_price;
                
                // Only update change values if we have a previous price (not first load)
                if old_price > 0.0 {
                    daily_change.set(price_diff);
                    daily_change_percent.set((price_diff / old_price) * 100.0);
                } else {
                    // Default to 1% change for first load
                    daily_change.set(new_sol_price * 0.01);
                    daily_change_percent.set(1.0);
                }
                
                // Update SOL price value
                sol_price.set(*new_sol_price);
            }
            
            println!("Successfully updated token prices: {:?}", prices);
        },
        Err(e) => {
            // Handle error without crashing the app
            price_error.set(Some(format!("Failed to fetch prices: {}", e)));
            println!("Error fetching token prices: {}", e);
        }
    }
    
    prices_loading.set(false);
}

// Hardcoded verified tokens including USDC, USDT, JTO, JUP, JLP, and BONK
fn get_verified_tokens() -> HashMap<String, JupiterToken> {
    let mut map = HashMap::new();
    
    // USDC
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
    
    // USDT
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
    
    // JTO
    map.insert(
        "jtojtomepa8beP8AuQc6eXt5FriJwfFMwQx2v2f9mCL".to_string(),
        JupiterToken {
            address: "jtojtomepa8beP8AuQc6eXt5FriJwfFMwQx2v2f9mCL".to_string(),
            name: "Jito".to_string(),
            symbol: "JTO".to_string(),
            logo_uri: "".to_string(),
            tags: vec!["token".to_string()],
        },
    );
    
    // JUP
    map.insert(
        "JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN".to_string(),
        JupiterToken {
            address: "JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN".to_string(),
            name: "Jupiter".to_string(),
            symbol: "JUP".to_string(),
            logo_uri: "".to_string(),
            tags: vec!["token".to_string()],
        },
    );
    
    // JLP
    map.insert(
        "27G8MtK7VtTcCHkpASjSDdkWWYfoqT6ggEuKidVJidD4".to_string(),
        JupiterToken {
            address: "27G8MtK7VtTcCHkpASjSDdkWWYfoqT6ggEuKidVJidD4".to_string(),
            name: "Jupiter LP".to_string(),
            symbol: "JLP".to_string(),
            logo_uri: "".to_string(),
            tags: vec!["token".to_string()],
        },
    );
    
    // BONK
    map.insert(
        "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263".to_string(),
        JupiterToken {
            address: "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263".to_string(),
            name: "Bonk".to_string(),
            symbol: "BONK".to_string(),
            logo_uri: "".to_string(),
            tags: vec!["meme".to_string()],
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
    let mut show_history_modal = use_signal(|| false);
    let mut show_stake_modal = use_signal(|| false);

    // Hardware wallet state
    let mut hardware_wallet = use_signal(|| None as Option<Arc<HardwareWallet>>);
    let mut show_hardware_modal = use_signal(|| false);
    let mut hardware_device_present = use_signal(|| false);
    let mut hardware_connected = use_signal(|| false);
    let mut hardware_pubkey = use_signal(|| None as Option<String>);

    // RPC management
    let mut custom_rpc = use_signal(|| load_rpc_from_storage());
    let mut rpc_input = use_signal(|| custom_rpc().unwrap_or_default());

    //JITO Stuff
    let mut show_jito_modal = use_signal(|| false);
    let mut jito_settings = use_signal(|| load_jito_settings_from_storage());

    // Balance management
    let mut balance = use_signal(|| 0.0);
    let mut sol_price = use_signal(|| 50.0); // Default price - will be updated from Pyth
    let mut token_changes = use_signal(|| HashMap::<String, (Option<f64>, Option<f64>)>::new());
    
    // Change these to ref signals for holding dynamic values
    let mut daily_change = use_signal(|| 0.0);
    let mut daily_change_percent = use_signal(|| 0.0);

    // Token management
    let mut tokens = use_signal(|| Vec::<Token>::new());
    
    // Add a new signal for token prices
    let mut token_prices = use_signal(|| HashMap::<String, f64>::new());
    let mut prices_loading = use_signal(|| false);
    let mut price_error = use_signal(|| None as Option<String>);

    // Verified tokens loaded with USDC and USDT
    let verified_tokens = use_memo(move || get_verified_tokens());

    // Background Selections
    let mut selected_background = use_signal(|| BackgroundTheme::get_presets()[0].clone());
    let mut show_background_modal = use_signal(|| false);

    //Currency
    let mut show_currency_modal = use_signal(|| false);

    //Tokens
    let mut show_send_token_modal = use_signal(|| false);
    let mut selected_token_symbol = use_signal(|| "".to_string());
    let mut selected_token_mint = use_signal(|| "".to_string());
    let mut selected_token_balance = use_signal(|| 0.0);
    let mut selected_token_decimals = use_signal(|| None as Option<u8>);

    //Wallet address expand
    let mut address_expanded = use_signal(|| false);

    fn get_token_price_change(
        symbol: &str, 
        changes_map: &HashMap<String, (Option<f64>, Option<f64>)>
    ) -> f64 {
        // Try exact match first
        if let Some((_, Some(percentage))) = changes_map.get(symbol) {
            return *percentage;
        }
        
        // Try uppercase
        let uppercase = symbol.to_uppercase();
        if let Some((_, Some(percentage))) = changes_map.get(&uppercase) {
            return *percentage;
        }
        
        // Try lowercase
        let lowercase = symbol.to_lowercase();
        if let Some((_, Some(percentage))) = changes_map.get(&lowercase) {
            return *percentage;
        }
        
        // No match found, use default
        3.0
    }

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

    // Create a separate effect for fetching historical data less frequently
    use_effect(move || {
        spawn(async move {
            // Initial fetch of historical changes
            fetch_historical_changes(token_prices, token_changes).await;
            
            // Then fetch every 15 minutes (much less frequent than current prices)
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(900)).await;
                fetch_historical_changes(token_prices, token_changes).await;
            }
        });
    });

    
    async fn fetch_historical_changes(
        token_prices: Signal<HashMap<String, f64>>,
        mut token_changes: Signal<HashMap<String, (Option<f64>, Option<f64>)>>,
    ) {
        // Get a copy of current prices
        let current_prices = token_prices.read().clone();
        
        // Fetch historical changes separately
        match prices::get_historical_changes(&current_prices).await {
            Ok(changes) => {
                println!("PRICE DEBUG: Successfully fetched historical changes: {:#?}", changes);
                token_changes.set(changes); // Just use the original changes directly
                
                // Add this line to confirm the data was set
                println!("PRICE DEBUG: After setting token_changes: {:#?}", token_changes.read());
            },
            Err(e) => {
                println!("PRICE DEBUG: Error fetching historical changes: {}", e);
            }
        }
    }

    // Fetch token prices periodically
    use_effect(move || {
        spawn(async move {
            // Initial fetch
            fetch_token_prices(token_prices, prices_loading, price_error, sol_price, daily_change, daily_change_percent).await;
            
            // Then fetch every 2 minutes (120 seconds)
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(120)).await;
                fetch_token_prices(token_prices, prices_loading, price_error, sol_price, daily_change, daily_change_percent).await;
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
        let token_prices_snapshot = token_prices.read().clone();
        
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
                    
                    // Get snapshots of current prices and historical changes
                    let token_prices_snapshot = token_prices_snapshot.clone();
                    let token_changes_snapshot = token_changes.read().clone();
                    println!("PRICE DEBUG: token_changes_snapshot in token creation: {:#?}", token_changes_snapshot);
                    
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
                            
                            // Get real price from token_prices if available
                            let price = token_prices_snapshot.get(&metadata.symbol)
                                .copied()
                                .unwrap_or_else(|| {
                                    match metadata.symbol.as_str() {
                                        "USDC" => 1.0,
                                        "USDT" => 1.0,
                                        _ => 1.0,
                                    }
                                });
                            
                                let symbol = metadata.symbol.as_str();
                                println!("PRICE DEBUG: Looking up price change for {}", symbol);
                                println!("PRICE DEBUG: token_changes_snapshot keys: {:?}", token_changes_snapshot.keys().collect::<Vec<_>>());
                                let price_change = get_token_price_change(symbol, &token_changes_snapshot);
                                println!("PRICE DEBUG: Got price change for {}: {}", symbol, price_change);
                                
                                // Use this price_change variable here
                                let value_usd = account.amount * price;
                                let icon_type = match metadata.symbol.as_str() {
                                    "USDC" => ICON_USDC.to_string(),
                                    "USDT" => ICON_USDT.to_string(),
                                    "JTO" => ICON_JTO.to_string(),
                                    "JUP" => ICON_JUP.to_string(),
                                    "JLP" => ICON_JLP.to_string(),
                                    "BONK" => ICON_BONK.to_string(),
                                    _ => ICON_32.to_string(),
                                };
                                
                            let value_usd = account.amount * price;
                            let icon_type = match metadata.symbol.as_str() {
                                "USDC" => ICON_USDC.to_string(),
                                "USDT" => ICON_USDT.to_string(),
                                "JTO" => ICON_JTO.to_string(),
                                "JUP" => ICON_JUP.to_string(),
                                "JLP" => ICON_JLP.to_string(),
                                "BONK" => ICON_BONK.to_string(),
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
                                price_change,
                            }
                        })
                        .collect::<Vec<Token>>();
                    
                    println!("Processed tokens for address {}: {:?}", address, new_tokens);
                    
                    // Get the most recent SOL price
                    let current_sol_price = token_prices_snapshot.get("SOL").copied().unwrap_or(sol_price());
                    
                    // Get SOL percentage change from historical data                    
                    let sol_price_change = get_token_price_change("SOL", &token_changes_snapshot);

                    let mut all_tokens = vec![Token {
                        mint: "So11111111111111111111111111111111111111112".to_string(),
                        symbol: "SOL".to_string(),
                        name: "Solana".to_string(),
                        icon_type: ICON_SOL.to_string(),
                        balance: balance(),
                        value_usd: balance() * current_sol_price,
                        price: current_sol_price,
                        price_change: sol_price_change,
                    }];
                    all_tokens.extend(new_tokens);
                    
                    tokens.set(all_tokens);
                }
                Err(e) => {
                    println!("Failed to fetch token accounts for address {}: {}", address, e);
                    
                    // Get the most recent SOL price
                    let current_sol_price = token_prices_snapshot.get("SOL").copied().unwrap_or(sol_price());
                    
                    // Get SOL percentage change from historical data (if available)
                    let token_changes_snapshot = token_changes.read().clone();
                    // Get SOL percentage change from historical data
                    let sol_price_change = get_token_price_change("SOL", &token_changes_snapshot);
                    
                    tokens.set(vec![Token {
                        mint: "So11111111111111111111111111111111111111112".to_string(),
                        symbol: "SOL".to_string(),
                        name: "Solana".to_string(),
                        icon_type: ICON_SOL.to_string(),
                        balance: balance(),
                        value_usd: balance() * current_sol_price,
                        price: current_sol_price,
                        price_change: sol_price_change,
                    }]);
                }
            }
        });
    });

    use_effect(move || {
        spawn(async move {
            // Initialize currency system
            initialize_currency_system().await;
            
            // Start exchange rate update loop
            update_exchange_rates_loop().await;
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

    // Calculate USD value using current SOL price
    let usd_balance = balance() * sol_price();

    let (start, middle, end) = if full_address != "No Wallet" && full_address.len() > 8 {
        (
            &full_address[..4],
            &full_address[4..full_address.len() - 4],
            &full_address[full_address.len() - 4..],
        )
    } else {
        ("", full_address.as_str(), "")
    };

    let short_address = format!("{}...{}", start, end);    

    rsx! {
        div {
            class: "wallet-container-dynamic",
            style: {
                format!(
                    "background-image: linear-gradient(to bottom, rgba(0, 0, 0, 0.1) 0%, rgba(0, 0, 0, 0.3) 40%, rgba(0, 0, 0, 0.7) 100%), url('{}'); background-size: cover; background-position: center top; background-repeat: no-repeat; background-attachment: fixed;",
                    selected_background.read().url
                )
            },
            onclick: move |_| {
                if show_dropdown() {
                    show_dropdown.set(false);
                }
            },
            
            // Header
            div {
                class: "wallet-header-enhanced",
                // Left side - Profile/Hardware icon
                div {
                    class: {
                        let mut classes = "profile-icon".to_string();
                        if hardware_device_present() {
                            classes.push_str(" hardware-present");
                        }
                        if hardware_connected() {
                            classes.push_str(" hardware-connected");
                        }
                        classes
                    },
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
                    div {
                        class: {
                            let indicator_class = if hardware_connected() { 
                                "hardware-indicator connected" 
                            } else if hardware_device_present() { 
                                "hardware-indicator present" 
                            } else {
                                "hardware-indicator default"
                            };
                            indicator_class.to_string()
                        }
                    }
                }

                // Center - Wallet Address Section with working CSS highlighting
                div {
                    class: "header-address-section",
                    div {
                        class: "header-address-label",
                        if hardware_connected() && hardware_pubkey().is_some() {
                            "Hardware Wallet"
                        } else if current_wallet.is_some() {
                            "Wallet"
                        } else {
                            "No Wallet"
                        }
                    }
                
                    div {
                        class: {
                            let mut class = "header-address-display expandable".to_string();
                            if address_expanded() {
                                class.push_str(" expanded");
                            }
                            class
                        },
                        onclick: move |_| {
                            address_expanded.set(!address_expanded());
                        
                            let address_to_copy = full_address.clone();
                        
                            #[cfg(target_arch = "wasm32")]
                            {
                                log::info!("Clipboard copy not supported on web platform.");
                            }
                        
                            #[cfg(target_os = "android")]
                            {
                                log::info!("Clipboard copy not supported on Android platform.");
                            }
                        
                            #[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
                            {
                                std::thread::spawn(move || {
                                    if let Ok(mut clipboard) = SystemClipboard::new() {
                                        let _ = clipboard.set_text(address_to_copy);
                                    }
                                });
                            }
                        },                                                                                                                                                                             
                        div {
                            class: "short-address",
                            hidden: address_expanded(),
                            "{short_address}"
                        }
                        div {
                            class: "full-address",
                            hidden: !address_expanded(),
                            span { class: "highlight", "{start}" }
                            span { class: "subtle", "{middle}" }
                            span { class: "highlight", "{end}" }
                        }
                    }
                    
                }

                // Right side - Menu icon
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
                        
                        // Current wallet display
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
                        
                        // Hardware wallet display (unchanged)
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
                        
                        // NEW: Currency Selector
                        button {
                            class: "dropdown-item currency-selector",
                            onclick: move |_| {
                                show_currency_modal.set(true);
                                show_dropdown.set(false);
                            },
                            div {
                                class: "dropdown-icon action-icon",
                                "💱"
                            }
                            div {
                                class: "currency-display",
                                "Currency: "
                                span {
                                    class: "current-symbol",
                                    "{get_current_currency_code()}"
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
                        
                        // Existing action buttons (unchanged)
                        button {
                            class: "dropdown-item",
                            onclick: move |_| {
                                modal_mode.set("create".to_string());
                                show_wallet_modal.set(true);
                                show_dropdown.set(false);
                            },
                            div {
                                class: "dropdown-icon action-icon",
                                "+"
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
                                show_jito_modal.set(true);
                                show_dropdown.set(false);
                            },
                            div {
                                class: "dropdown-icon action-icon",
                                "⚡"
                            }
                            "JITO Settings"
                        }

                        button {
                            class: "dropdown-item",
                            onclick: move |_| {
                                show_background_modal.set(true);
                                show_dropdown.set(false);
                            },
                            div {
                                class: "dropdown-icon action-icon",
                                "🎨"
                            }
                            "Change Background"
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

            if show_jito_modal() {
                JitoModal {
                    current_settings: jito_settings(),
                    onclose: move |_| show_jito_modal.set(false),
                    onsave: move |new_settings| {
                        jito_settings.set(new_settings);
                        save_jito_settings_to_storage(&new_settings);
                        show_jito_modal.set(false);
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
                        // Don't reset hardware_wallet here
                    },
                    onsuccess: move |_| {
                        show_send_modal.set(false);
                        // Don't reset hardware_wallet here either
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
                    },
                    // Add new event handler for hardware wallet status changes
                    onhardware: move |event: HardwareWalletEvent| {
                        // Update the hardware wallet connection state in the parent component
                        hardware_connected.set(event.connected);
                        hardware_pubkey.set(event.pubkey);
                        
                        // If disconnected, also set the hardware_wallet to None
                        if !event.connected {
                            hardware_wallet.set(None);
                        }
                    }
                }
            }
            
            if show_send_token_modal() {
                SendTokenModal {
                    wallet: current_wallet.clone(),
                    hardware_wallet: hardware_wallet(),
                    token_symbol: selected_token_symbol(),
                    token_mint: selected_token_mint(),
                    token_balance: selected_token_balance(),
                    token_decimals: selected_token_decimals(),
                    custom_rpc: custom_rpc(),
                    onclose: move |_| {
                        show_send_token_modal.set(false);
                        selected_token_symbol.set("".to_string());
                        selected_token_mint.set("".to_string());
                        selected_token_balance.set(0.0);
                        selected_token_decimals.set(None);
                    },
                    onsuccess: move |signature| {
                        show_send_token_modal.set(false);
                        selected_token_symbol.set("".to_string());
                        selected_token_mint.set("".to_string());
                        selected_token_balance.set(0.0);
                        selected_token_decimals.set(None);
                        println!("Token transaction successful: {}", signature);
                        
                        // Refresh balances after successful transaction
                        if let Some(wallet) = wallets.read().get(current_wallet_index()) {
                            let address = wallet.address.clone();
                            let rpc_url = custom_rpc();
                            
                            spawn(async move {
                                match rpc::get_balance(&address, rpc_url.as_deref()).await {
                                    Ok(sol_balance) => {
                                        balance.set(sol_balance);
                                    }
                                    Err(e) => {
                                        println!("Failed to refresh balance after token send: {}", e);
                                    }
                                }
                            });
                        }
                    },
                    onhardware: move |event: HardwareWalletEvent| {
                        hardware_connected.set(event.connected);
                        hardware_pubkey.set(event.pubkey);
                        
                        // If disconnected, also set the hardware_wallet to None
                        if !event.connected {
                            hardware_wallet.set(None);
                        }
                    },
                }
            }
            
            if show_receive_modal() {
                ReceiveModal {
                    wallet: current_wallet.clone(),
                    hardware_wallet: hardware_wallet(),
                    onclose: move |_| show_receive_modal.set(false)
                }
            }

            if show_stake_modal() {
                StakeModal {
                    wallet: current_wallet.clone(),
                    hardware_wallet: hardware_wallet(),
                    current_balance: balance(),
                    custom_rpc: custom_rpc(),
                    onclose: move |_| {
                        show_stake_modal.set(false);
                    },
                    onsuccess: move |_| {
                        show_stake_modal.set(false);
                        // Refresh balance after staking
                        if let Some(wallet) = wallets.read().get(current_wallet_index()) {
                            let address = wallet.address.clone();
                            let rpc_url = custom_rpc();
                            
                            spawn(async move {
                                match rpc::get_balance(&address, rpc_url.as_deref()).await {
                                    Ok(sol_balance) => {
                                        balance.set(sol_balance);
                                    }
                                    Err(e) => {
                                        println!("Error refreshing balance after stake: {}", e);
                                    }
                                }
                            });
                        }
                    }
                }
            }

            if show_background_modal() {
                BackgroundModal {
                    current_background: selected_background(),
                    onclose: move |_| show_background_modal.set(false),
                    onselect: move |theme: BackgroundTheme| {
                        selected_background.set(theme);
                        show_background_modal.set(false);
                    }
                }
            }

            if show_currency_modal() {
                CurrencyModal {
                    onclose: move |_| show_currency_modal.set(false)
                }
            }
                        
            // Main content container for balance, address, and actions
            div {
                class: "main-content",
                div {
                    class: "balance-section-enhanced",
                    div {
                        class: "balance-amount-bold",
                        // Show loading state for total portfolio value when prices are refreshing
                        if prices_loading() {
                            "Loading..."
                        } else {
                            // Calculate total portfolio value (sum of all token values) and round to nearest dollar
                            {
                                let total_value = tokens.read().iter().fold(0.0, |acc, token| acc + token.value_usd);
                                format_price_in_selected_currency(total_value)
                            }
                        }
                    }
                    // SOL balance row - clean and simple, no USD value
                    //div {
                    //    class: "balance-sol-row",
                    //    span {
                    //        class: "balance-sol",
                    //        "{balance:.4} SOL"
                    //    }
                    //}
                    // Display price refresh error if any
                    if let Some(error) = price_error() {
                        div {
                            class: "price-error",
                            "Price data error: {error}"
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
                            img {
                                src: "{ICON_RECEIVE}",
                                alt: "Receive",
                                width: "24",
                                height: "24",
                            }
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
                            img {
                                src: "{ICON_SEND}",
                                alt: "Send",
                                width: "24",
                                height: "24",
                            }
                        }
                        span {
                            class: "action-label",
                            "Send"
                        }
                    }
                    button {
                        class: "action-button",
                        onclick: move |_| show_stake_modal.set(true),
                        div {
                            class: "action-icon",
                            img {
                                src: "{ICON_STAKE}",
                                alt: "Stake",
                                width: "24",
                                height: "24",
                            }
                        }
                        span {
                            class: "action-label",
                            "Stake"
                        }
                    }
                    //button {
                    //    class: "action-button",
                    //    div {
                    //        class: "action-icon",
                    //        "🔄"
                    //    }
                    //    span {
                    //        class: "action-label",
                    //        "Swap"
                    //    }
                    //}
                    //button {
                    //    class: "action-button",
                    //    onclick: move |_| show_history_modal.set(true),
                    //    div {
                    //        class: "action-icon history-icon",
                    //        "📜"
                    //    }
                    //    span {
                    //        class: "action-label",
                    //        "History"
                    //    }
                    //}
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
                                        onerror: {
                                            let symbol = token.symbol.clone();
                                            let icon_type = token.icon_type.clone();
                                            move |_| {
                                                println!("Failed to load image for {}: {}", symbol, icon_type);
                                            }
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
                                            class: if token.price_change >= 0.0 {
                                                "token-change positive"
                                            } else {
                                                "token-change negative"
                                            },
                                            if token.price_change >= 0.0 {
                                                "+{token.price_change:.1}%"
                                            } else {
                                                "{token.price_change:.1}%"
                                            }
                                        }
                                    }
                                }
                            }
                            // Send button positioned absolutely - doesn't affect layout
                            button {
                                class: "token-send-button",
                                onclick: {
                                    let token_symbol = token.symbol.clone();
                                    let token_mint = token.mint.clone();
                                    let token_balance = token.balance;
                                    move |_| {
                                        if token_symbol == "SOL" {
                                            // Open SOL send modal (existing behavior)
                                            show_send_modal.set(true);
                                        } else {
                                            // Open token send modal for SPL tokens
                                            selected_token_symbol.set(token_symbol.clone());
                                            selected_token_mint.set(token_mint.clone());
                                            selected_token_balance.set(token_balance);
                                            
                                            // Set decimals based on known tokens, default to 6
                                            let decimals = match token_symbol.as_str() {
                                                "USDC" | "USDT" => Some(6),
                                                "JLP" => Some(6),
                                                "JUP" => Some(6),
                                                "JTO" => Some(9),
                                                "BONK" => Some(5),
                                                _ => Some(6), // Default for most SPL tokens
                                            };
                                            selected_token_decimals.set(decimals);
                                            
                                            show_send_token_modal.set(true);
                                            
                                            println!("Send {} (mint: {}) clicked", token_symbol, token_mint);
                                        }
                                    }
                                },
                                title: "Send {token.symbol}",
                                div {
                                    class: "token-send-icon",
                                    img {
                                        src: "{ICON_SEND}",
                                        alt: "Send",
                                        width: "14",
                                        height: "14",
                                    }
                                }
                            }
                            // Keep original token-values structure
                            div {
                                class: "token-values",
                                div {
                                    class: "token-value-usd",
                                    "{format_token_value_smart(token.balance, token.price)}"
                                }
                                div {
                                    class: "token-amount",
                                    "{format_token_amount(token.balance, &token.symbol)}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}