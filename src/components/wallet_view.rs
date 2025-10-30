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
    delete_wallet_from_storage,
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
    get_current_currency_code,
    format_portfolio_balance
};
use crate::components::modals::currency_modal::CurrencyModal;
use crate::components::modals::{WalletModal, RpcModal, SendModalWithHardware, SendTokenModal, HardwareWalletModal, ReceiveModal, JitoModal, StakeModal, BulkSendModal, SwapModal, TransactionHistoryModal, LendModal, ExportWalletModal, DeleteWalletModal, SquadsModal, CarrotModal, BonkStakingModal};
use crate::components::modals::send_modal::HardwareWalletEvent;
use crate::token_utils::process_tokens_for_display;
use crate::components::common::TokenDisplayData;
use crate::components::common::{Token, TokenSortConfig, TokenFilter, SortCriteria};
use crate::rpc::{self, CollectibleInfo, fetch_collectibles, TokenAccountFilter};
use crate::prices;
use crate::hardware::HardwareWallet;
use crate::hardware::HardwareDeviceType;
use crate::components::background_themes::BackgroundTheme;
use crate::components::modals::BackgroundModal;
use crate::prices::CandlestickData;
use crate::config::tokens::{get_verified_tokens, VerifiedToken};
use std::sync::Arc;
use std::collections::HashMap;
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
use arboard::Clipboard as SystemClipboard;
use std::collections::HashSet;
use rand::{thread_rng, Rng};

// Define the assets for icons
//const ICON_32: Asset = asset!("/assets/icons/32x32.png");
//const ICON_SOL: Asset = asset!("/assets/icons/solanaLogo.png");
//const ICON_USDC: Asset = asset!("/assets/icons/usdcLogo.png");
//const ICON_USDT: Asset = asset!("/assets/icons/usdtLogo.png");
//const ICON_JTO: Asset = asset!("/assets/icons/jtoLogo.png");
//const ICON_JUP: Asset = asset!("/assets/icons/jupLogo.png");
//const ICON_JLP: Asset = asset!("/assets/icons/jlpLogo.png");
//const ICON_BONK: Asset = asset!("/assets/icons/bonkLogo.png");

// Action button SVG icons
//const ICON_RECEIVE: Asset = asset!("/assets/icons/receive.svg");
//const ICON_SEND: Asset = asset!("/assets/icons/send.svg");
//const ICON_STAKE: Asset = asset!("/assets/icons/stake.svg");
//const ICON_BULK: Asset = asset!("/assets/icons/bulk.svg");
//const ICON_SWAP: Asset = asset!("/assets/icons/swap.svg");
//const ICON_LEND: Asset = asset!("/assets/icons/jupLendLogo.svg");
//const LOADING_SPINNER: Asset = asset!("/assets/icons/infinite-spinner.svg");

//const DEVICE_LEDGER: Asset = asset!("assets/icons/ledger_device.webp");
//const DEVICE_UNRGBL: Asset = asset!("assets/icons/unruggable_device.png");
//const DEVICE_SOFTWARE: Asset = asset!("assets/icons/hot_wallet.png");

const ICON_32:     &str = "https://cdn.jsdelivr.net/gh/hogyzen12/solana-mobile@main/assets/icons/32x32.png";
const ICON_SOL:    &str = "https://cdn.jsdelivr.net/gh/hogyzen12/solana-mobile@main/assets/icons/solanaLogo.png";
const ICON_USDC:   &str = "https://cdn.jsdelivr.net/gh/hogyzen12/solana-mobile@main/assets/icons/usdcLogo.png";
const ICON_USDT:   &str = "https://cdn.jsdelivr.net/gh/hogyzen12/solana-mobile@main/assets/icons/usdtLogo.png";
const ICON_JTO:    &str = "https://cdn.jsdelivr.net/gh/hogyzen12/solana-mobile@main/assets/icons/jtoLogo.png";
const ICON_JUP:    &str = "https://cdn.jsdelivr.net/gh/hogyzen12/solana-mobile@main/assets/icons/jupLogo.png";
const ICON_JLP:    &str = "https://cdn.jsdelivr.net/gh/hogyzen12/solana-mobile@main/assets/icons/jlpLogo.png";
const ICON_BONK:   &str = "https://cdn.jsdelivr.net/gh/hogyzen12/solana-mobile@main/assets/icons/bonkLogo.png";

const ICON_RECEIVE:&str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/icons/receive.svg";
const ICON_SEND:   &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/icons/send.svg";
const ICON_STAKE:  &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/icons/stake.svg";
const ICON_BULK:   &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/icons/bulk.svg";
const ICON_SWAP:   &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/icons/swap.svg";
const ICON_LEND:   &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/icons/jupLendLogo.svg";
const ICON_SQUADS: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/icons/squadsLogo.svg";
const ICON_CARROT: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/icons/CARROT.svg";
const ICON_BONK_STAKE: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/icons/BONK.svg";
const ICON_WALLET: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/icons/WALLETS.svg";
const ICON_CREATE: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/icons/ADD_wallet.svg";
const ICON_IMPORT: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/icons/IMPORT_wallet.svg";
const ICON_EXPORT: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/icons/EXPORT_wallet.svg";
const ICON_DELETE: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/icons/DELETE_wallet.svg";
const ICON_RPC: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/icons/RPC.svg";

const DEVICE_LEDGER:&str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/icons/ledger_device.webp";
const DEVICE_UNRGBL:&str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/icons/unruggable_device.png";
const DEVICE_SOFTWARE:&str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/icons/hot_wallet.png";
const LOADING_SPINNER:&str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/icons/infinite-spinner.svg";


// JupiterToken struct with PartialEq and Eq for use_memo
#[derive(Clone, Debug, PartialEq, Eq)]
struct JupiterToken {
    address: String,
    name: String,
    symbol: String,
    logo_uri: String,
    tags: Vec<String>,
}

async fn fetch_token_prices(
    mut token_prices: Signal<HashMap<String, f64>>,
    mut prices_loading: Signal<bool>,
    mut price_error: Signal<Option<String>>,
    mut sol_price: Signal<f64>,
    mut daily_change: Signal<f64>,
    mut daily_change_percent: Signal<f64>,
    mut token_changes: Signal<HashMap<String, (Option<f64>, Option<f64>)>>,
    mut multi_timeframe_data: Signal<HashMap<String, prices::MultiTimeframePriceData>>, // NEW: Add this
) {
    prices_loading.set(true);
    price_error.set(None);

    // Use the new cached function
    match prices::get_cached_prices_and_changes().await {
        Ok((current_prices, multi_data)) => {
            println!("✅ Got prices and multi-timeframe data");
            
            // Set the multi-timeframe data signal
            multi_timeframe_data.set(multi_data.clone());
            
            // Convert to old format for backward compatibility
            let mut old_format_changes = HashMap::new();
            for (token, data) in &multi_data {
                old_format_changes.insert(token.clone(), (data.change_1d_amount, data.change_1d_percentage));
                
                // Print all timeframes for debugging
                println!("📊 {}: 1D={:+.1}%, 3D={:+.1}%, 7D={:+.1}%", 
                         token,
                         data.change_1d_percentage.unwrap_or(0.0),
                         data.change_3d_percentage.unwrap_or(0.0),
                         data.change_7d_percentage.unwrap_or(0.0));
            }
            
            // Set both signals
            token_changes.set(old_format_changes);
            token_prices.set(current_prices.clone());

            // Update SOL price
            if let Some(new_sol_price) = current_prices.get("SOL") {
                let old_price = sol_price();
                let price_diff = new_sol_price - old_price;
                
                if old_price > 0.0 {
                    daily_change.set(price_diff);
                    daily_change_percent.set((price_diff / old_price) * 100.0);
                } else {
                    daily_change.set(0.0);
                    daily_change_percent.set(0.0);
                }
                
                sol_price.set(*new_sol_price);
            }
            
            println!("✅ Successfully updated all price data with cache");
        },
        Err(e) => {
            price_error.set(Some(format!("Failed to fetch prices: {}", e)));
            println!("❌ Error fetching prices: {}", e);
        }
    }
    
    prices_loading.set(false);
}

async fn fetch_token_prices_for_discovered_tokens(
    discovered_tokens: HashMap<String, String>, // mint -> symbol mapping
    mut token_prices: Signal<HashMap<String, f64>>,
    mut prices_loading: Signal<bool>,
    mut price_error: Signal<Option<String>>,
    mut sol_price: Signal<f64>,
    mut daily_change: Signal<f64>,
    mut daily_change_percent: Signal<f64>,
    mut token_changes: Signal<HashMap<String, (Option<f64>, Option<f64>)>>,
    mut multi_timeframe_data: Signal<HashMap<String, prices::MultiTimeframePriceData>>,
) {
    prices_loading.set(true);
    price_error.set(None);

    println!("Fetching prices for discovered tokens: {:?}", discovered_tokens);

    // Use the corrected function name from prices.rs
    match prices::get_prices_for_tokens(discovered_tokens).await {
        Ok(current_prices) => {
            println!("Got dynamic prices: {:?}", current_prices);
            
            // Create dummy multi-timeframe data for backward compatibility
            let mut multi_data = HashMap::new();
            for (token, price) in &current_prices {
                multi_data.insert(token.clone(), prices::MultiTimeframePriceData {
                    current_price: *price,
                    change_1d_amount: Some(0.0),
                    change_1d_percentage: Some(0.0),
                    change_3d_amount: Some(0.0),
                    change_3d_percentage: Some(0.0),
                    change_7d_amount: Some(0.0),
                    change_7d_percentage: Some(0.0),
                });
            }
            
            // Set the multi-timeframe data signal
            multi_timeframe_data.set(multi_data.clone());
            
            // Convert to old format for backward compatibility
            let mut old_format_changes = HashMap::new();
            for (token, data) in &multi_data {
                old_format_changes.insert(token.clone(), (data.change_1d_amount, data.change_1d_percentage));
            }
            
            // Set both signals
            token_changes.set(old_format_changes);
            token_prices.set(current_prices.clone());

            // Update SOL price
            if let Some(new_sol_price) = current_prices.get("SOL") {
                let old_price = sol_price();
                let price_diff = new_sol_price - old_price;
                
                if old_price > 0.0 {
                    daily_change.set(price_diff);
                    daily_change_percent.set((price_diff / old_price) * 100.0);
                } else {
                    daily_change.set(0.0);
                    daily_change_percent.set(0.0);
                }
                
                sol_price.set(*new_sol_price);
            }
            
            println!("Successfully updated all price data for {} tokens", current_prices.len());
        },
        Err(e) => {
            price_error.set(Some(format!("Failed to fetch prices: {}", e)));
            println!("Error fetching prices: {}", e);
        }
    }
    
    prices_loading.set(false);
}

// Helper function to get fallback icons
fn get_fallback_icon(symbol: &str) -> String {
    match symbol {
        "USDC" => ICON_USDC.to_string(),
        "USDT" => ICON_USDT.to_string(),
        "JTO" => ICON_JTO.to_string(),
        "JUP" => ICON_JUP.to_string(),
        "JLP" => ICON_JLP.to_string(),
        "BONK" => ICON_BONK.to_string(),
        "SOL" => ICON_SOL.to_string(),
        _ => ICON_32.to_string(),
    }
}

#[component]
fn CandlestickChart(
    data: Vec<CandlestickData>,
    symbol: String,
    timeframe: String, // Just pass the timeframe as a simple string
) -> Element {
    println!("🎯 Rendering candlestick chart for {} with {} candles ({})", symbol, data.len(), timeframe);
    
    if data.is_empty() {
        return rsx! {
            div {
                class: "chart-error",
                "No chart data available"
            }
        };
    }

    // Chart dimensions
    let width = 350.0;  // Increase from 300.0
    let height = 160.0; // Increase from 120.0
    let margin = 15.0;  // Increase margin slightly
    let chart_width = width - (margin * 2.0);
    let chart_height = height - (margin * 2.0);

    // Find price range
    let min_price = data.iter().map(|c| c.low).fold(f64::INFINITY, f64::min);
    let max_price = data.iter().map(|c| c.high).fold(0.0, f64::max);
    let price_range = max_price - min_price;

    // Scale functions
    let price_to_y = |price: f64| -> f64 {
        if price_range > 0.0 {
            margin + ((max_price - price) / price_range) * chart_height
        } else {
            height / 2.0
        }
    };

    let index_to_x = |index: usize| -> f64 {
        margin + (index as f64 / (data.len() - 1).max(1) as f64) * chart_width
    };

    // Candle width
    let candle_width = if data.len() > 1 {
        (chart_width / data.len() as f64 * 0.8).max(1.0).min(8.0)
    } else {
        4.0
    };

    rsx! {
        div {
            class: "candlestick-chart-container",
            svg {
                width: "{width}",
                height: "{height}",
                view_box: "0 0 {width} {height}",
                style: "background: rgba(0, 0, 0, 0.3); border-radius: 8px;",
                
                // Background grid lines (optional)
                defs {
                    pattern {
                        id: "grid-{symbol}",
                        width: "20",
                        height: "20",
                        pattern_units: "userSpaceOnUse",
                        path {
                            d: "M 20 0 L 0 0 0 20",
                            fill: "none",
                            stroke: "rgba(255, 255, 255, 0.05)",
                            stroke_width: "1",
                        }
                    }
                }
                rect {
                    width: "{width}",
                    height: "{height}",
                    fill: "url(#grid-{symbol})",
                }
                
                // Draw candlesticks
                for (i, candle) in data.iter().enumerate() {
                    {
                        let x = index_to_x(i);
                        let open_y = price_to_y(candle.open);
                        let close_y = price_to_y(candle.close);
                        let high_y = price_to_y(candle.high);
                        let low_y = price_to_y(candle.low);
                        
                        let is_bullish = candle.close >= candle.open;
                        let body_top = if is_bullish { close_y } else { open_y };
                        let body_bottom = if is_bullish { open_y } else { close_y };
                        let body_height = (body_bottom - body_top).abs().max(1.0);
                        
                        let color = if is_bullish { "#22c55e" } else { "#ef4444" };
                        
                        rsx! {
                            g {
                                key: "{i}",
                                // High-Low line (wick)
                                line {
                                    x1: "{x}",
                                    y1: "{high_y}",
                                    x2: "{x}",
                                    y2: "{low_y}",
                                    stroke: "{color}",
                                    stroke_width: "1",
                                    opacity: "0.8",
                                }
                                // Candle body
                                rect {
                                    x: "{x - candle_width / 2.0}",
                                    y: "{body_top}",
                                    width: "{candle_width}",
                                    height: "{body_height}",
                                    fill: if is_bullish { "none" } else { color },
                                    stroke: "{color}",
                                    stroke_width: "1",
                                    opacity: "0.9",
                                }
                            }
                        }
                    }
                }
                
                // Price labels (min/max)
                text {
                    x: "{margin}",
                    y: "{margin - 2.0}",
                    fill: "#888",
                    font_size: "10",
                    font_family: "monospace",
                    "${max_price:.2}"
                }
                text {
                    x: "{margin}",
                    y: "{height - 2.0}",
                    fill: "#888",
                    font_size: "10",
                    font_family: "monospace",
                    "${min_price:.2}"
                }
            }
            
            // Chart summary below
            div {
                class: "chart-summary",
                span {
                    "Range: ${min_price:.2} - ${max_price:.2}"
                }
                span {
                    {
                        let latest = data.last().unwrap();
                        let change = latest.close - data.first().unwrap().close;
                        let change_pct = (change / data.first().unwrap().close) * 100.0;
                        let period_label = match timeframe.as_str() {
                            "1H" => "3D",
                            "1D" => "30D", 
                            _ => "Period",
                        };
                        if change >= 0.0 {
                            format!("{}: +{:.1}%", period_label, change_pct)
                        } else {
                            format!("{}: {:.1}%", period_label, change_pct)
                        }
                    }
                }
            }
        }
    }
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
    let mut show_swap_modal = use_signal(|| false);
    let mut show_squads_modal = use_signal(|| false);
    let mut show_carrot_modal = use_signal(|| false);
    let mut show_bonk_staking_modal = use_signal(|| false);
    
    // Integrations collapse/expand state
    let mut show_integrations = use_signal(|| false);

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

    //Additional Wallet features
    let mut show_export_modal = use_signal(|| false);
    let mut show_delete_confirmation = use_signal(|| false);

    // Balance management
    let mut balance = use_signal(|| 0.0);
    let mut sol_price = use_signal(|| 50.0); // Default price - will be updated from Pyth
    let mut token_changes = use_signal(|| HashMap::<String, (Option<f64>, Option<f64>)>::new());
    
    // Change these to ref signals for holding dynamic values
    let mut daily_change = use_signal(|| 0.0);
    let mut daily_change_percent = use_signal(|| 0.0);

    // Token management
    let mut tokens = use_signal(|| Vec::<Token>::new());
    // Add these after existing signals
    let mut token_sort_config = use_signal(|| TokenSortConfig::default());
    let mut token_filter = use_signal(|| TokenFilter::default());
    let mut show_sort_menu = use_signal(|| false);
    
    // Add a new signal for token prices
    let mut token_prices = use_signal(|| HashMap::<String, f64>::new());
    let mut prices_loading = use_signal(|| false);
    let mut price_error = use_signal(|| None as Option<String>);

    let verified_tokens = use_memo(move || {
        get_verified_tokens().clone()
    });

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

    // Bulk send state management
    let mut bulk_send_mode = use_signal(|| false);
    let mut selected_tokens = use_signal(|| HashSet::<String>::new()); // Using mint addresses as keys
    let mut show_bulk_send_modal = use_signal(|| false);

    let mut multi_timeframe_data = use_signal(|| HashMap::<String, prices::MultiTimeframePriceData>::new());
    let mut expanded_tokens = use_signal(|| HashSet::<String>::new());
    let mut portfolio_expanded = use_signal(|| false);

    // Dropdown charts on price tap
    // Dropdown charts on price tap
    let mut chart_data = use_signal(|| HashMap::<String, Vec<CandlestickData>>::new());
    let mut chart_loading = use_signal(|| HashSet::<String>::new());
    let mut selected_timeframe = use_signal(|| HashMap::<String, String>::new()); // Per-token timeframe
    let mut chart_timeframe_data = use_signal(|| HashMap::<String, HashMap<String, Vec<CandlestickData>>>::new());

    let mut show_lend_modal = use_signal(|| false);
    let mut active_tab = use_signal(|| "tokens".to_string());
    let mut collectibles = use_signal(|| Vec::<CollectibleInfo>::new());
    let mut collectibles_loading = use_signal(|| false);

    // Add this signal near your other hardware wallet signals in wallet_view.rs
    let mut hardware_device_type = use_signal(|| None as Option<HardwareDeviceType>);
    let mut refresh_trigger = use_signal(|| 0u32);
    let mut is_refreshing = use_signal(|| false);
    
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
    
    async fn fetch_chart_data(
        symbol: String,
        mut chart_data: Signal<HashMap<String, Vec<CandlestickData>>>,
        mut chart_loading: Signal<HashSet<String>>,
    ) {
        println!("🚀 Starting chart data fetch for {}", symbol);
        
        // Add to loading set
        {
            let mut loading_set = chart_loading();
            loading_set.insert(symbol.clone());
            chart_loading.set(loading_set);
            println!("📊 Added {} to loading set", symbol);
        }

        match prices::get_candlestick_data(&symbol, 30).await { // Get 30 days of data
            Ok(data) => {
                println!("✅ Got {} candlesticks for {}", data.len(), symbol);
                
                // Log first few candlesticks for debugging
                for (i, candle) in data.iter().take(3).enumerate() {
                    println!("🕯️  Candle {}: O=${:.2} H=${:.2} L=${:.2} C=${:.2} T={}", 
                            i, candle.open, candle.high, candle.low, candle.close, candle.timestamp);
                }
                
                // Validate data quality
                let valid_candles = data.iter().filter(|c| c.open > 0.0 && c.high > 0.0 && c.low > 0.0 && c.close > 0.0).count();
                println!("📈 Valid candles: {}/{}", valid_candles, data.len());
                
                if valid_candles > 0 {
                    let mut chart_map = chart_data();
                    chart_map.insert(symbol.clone(), data);
                    chart_data.set(chart_map);
                    println!("💾 Saved chart data for {} to state", symbol);
                } else {
                    println!("❌ No valid candlestick data for {}", symbol);
                }
            },
            Err(e) => {
                println!("❌ Error fetching chart data for {}: {}", symbol, e);
            }
        }

        // Remove from loading set
        {
            let mut loading_set = chart_loading();
            loading_set.remove(&symbol);
            chart_loading.set(loading_set);
            println!("✅ Removed {} from loading set", symbol);
        }
    }

    async fn fetch_chart_data_with_timeframe(
        symbol: String,
        timeframe: String,
        mut chart_data: Signal<HashMap<String, Vec<CandlestickData>>>,
        mut chart_loading: Signal<HashSet<String>>,
    ) {
        println!("🚀 Starting chart data fetch for {} ({})", symbol, timeframe);
        
        let cache_key = format!("{}_{}", symbol, timeframe);
        
        // Add to loading set
        {
            let mut loading_set = chart_loading();
            loading_set.insert(cache_key.clone());
            chart_loading.set(loading_set);
        }

        let (days, resolution) = match timeframe.as_str() {
            "1H" => (3, "60"),   // 7 days of hourly data
            "1D" => (30, "1D"),  // 30 days of daily data
            _ => (30, "1D"),     // Default fallback
        };

        match prices::get_candlestick_data_with_resolution(&symbol, days, resolution).await {
            Ok(data) => {
                println!("✅ Got {} candlesticks for {} ({})", data.len(), symbol, timeframe);
                
                if !data.is_empty() {
                    let mut chart_map = chart_data();
                    chart_map.insert(cache_key.clone(), data);
                    chart_data.set(chart_map);
                    println!("💾 Saved chart data for {} ({}) to state", symbol, timeframe);
                }
            },
            Err(e) => {
                println!("❌ Error fetching chart data for {} ({}): {}", symbol, timeframe, e);
            }
        }

        // Remove from loading set
        {
            let mut loading_set = chart_loading();
            loading_set.remove(&cache_key);
            chart_loading.set(loading_set);
        }
    }

    fn get_token_price_change(
        symbol: &str, 
        changes_map: &HashMap<String, (Option<f64>, Option<f64>)>
    ) -> f64 {
        println!("Looking up price change for {}", symbol);
        println!("Available tokens in changes_map: {:?}", changes_map.keys().collect::<Vec<_>>());
        
        // Try exact match first - get the PERCENTAGE (second value in tuple)
        if let Some((_, Some(percentage))) = changes_map.get(symbol) {
            println!("✅ Found exact match for {}: {:.4}%", symbol, percentage);
            return *percentage;
        }
        
        // Try uppercase
        let uppercase = symbol.to_uppercase();
        if let Some((_, Some(percentage))) = changes_map.get(&uppercase) {
            println!("✅ Found uppercase match for {}: {:.4}%", symbol, percentage);
            return *percentage;
        }
        
        // Try lowercase
        let lowercase = symbol.to_lowercase();
        if let Some((_, Some(percentage))) = changes_map.get(&lowercase) {
            println!("✅ Found lowercase match for {}: {:.4}%", symbol, percentage);
            return *percentage;
        }
        
        // Check if we have the data but it's None
        if changes_map.contains_key(symbol) {
            println!("❌ {} found in map but percentage is None", symbol);
            return 0.0; // Return 0% instead of random
        }
        
        println!("❌ {} not found in changes_map at all", symbol);
        
        // Instead of random values, return 0.0 and log the issue
        // This will make it obvious when historical data is missing
        0.0
    }

    use_effect(move || {
        spawn(async move {
            // Initial fetch
            fetch_token_prices(token_prices, prices_loading, price_error, sol_price, daily_change, daily_change_percent, token_changes, multi_timeframe_data).await;
            
            // Then fetch every 2 minutes (120 seconds)
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(120)).await;
                fetch_token_prices(token_prices, prices_loading, price_error, sol_price, daily_change, daily_change_percent, token_changes, multi_timeframe_data).await;
            }
        });
    });

    // 5. Helper function to extract multi-timeframe data
    fn get_multi_timeframe_changes(
        symbol: &str,
        multi_data: &HashMap<String, prices::MultiTimeframePriceData>
    ) -> (f64, f64, f64) {
        if let Some(data) = multi_data.get(symbol) {
            (
                data.change_1d_percentage.unwrap_or(0.0),
                data.change_3d_percentage.unwrap_or(0.0),
                data.change_7d_percentage.unwrap_or(0.0)
            )
        } else {
            (0.0, 0.0, 0.0)
        }
    }

    // Fetch balance and token accounts when wallet changes or hardware wallet connects
    use_effect(move || {
        let wallets_list = wallets.read();
        let index = current_wallet_index();
        let hw_connected = hardware_connected();
        let hw_pubkey = hardware_pubkey();
        let _ = refresh_trigger();
        
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
            
            
            // Fetch token accounts from BOTH Token and Token-2022 programs
            println!("Fetching token accounts from both Token and Token-2022 programs...");
            
            // Fetch from standard Token program
            let filter_token = Some(rpc::TokenAccountFilter::ProgramId(
                "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string()
            ));
            let token_accounts = rpc::get_token_accounts_by_owner(&address, filter_token, rpc_url.as_deref()).await
                .unwrap_or_else(|e| {
                    println!("Failed to fetch Token program accounts: {}", e);
                    vec![]
                });
            
            // Fetch from Token-2022 program
            let filter_token22 = Some(rpc::TokenAccountFilter::ProgramId(
                "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".to_string()
            ));
            let token22_accounts = rpc::get_token_accounts_by_owner(&address, filter_token22, rpc_url.as_deref()).await
                .unwrap_or_else(|e| {
                    println!("Failed to fetch Token-2022 program accounts: {}", e);
                    vec![]
                });
            
            // Merge both sets of token accounts
            let mut all_token_accounts = token_accounts;
            let token22_count = token22_accounts.len();
            all_token_accounts.extend(token22_accounts);
            
            println!("Found {} token accounts total ({} Token + {} Token-2022)", 
                all_token_accounts.len(), 
                all_token_accounts.len() - token22_count,
                token22_count
            );
            
            if !all_token_accounts.is_empty() {
                    println!("Raw token accounts for address {}: {:?}", address, all_token_accounts);
                    
                    // Access the HashMap inside the Memo using read()
                    let verified_tokens_map = &verified_tokens_clone();
                    
                    // Get snapshots of current prices and historical changes
                    let token_prices_snapshot = token_prices_snapshot.clone();
                    let token_changes_snapshot = token_changes.read().clone();
                    println!("PRICE DEBUG: token_changes_snapshot in token creation: {:#?}", token_changes_snapshot);
                    
                    let all_non_zero_accounts: Vec<_> = all_token_accounts
                        .into_iter()
                        .filter(|account| {
                            let is_non_zero = account.amount > 0.0;
                            println!(
                                "Token {}: amount={}, will_include={}",
                                account.mint,
                                account.amount,
                                is_non_zero
                            );
                            is_non_zero  // <- INCLUDE ALL NON-ZERO TOKENS
                        })
                        .collect();

                    println!("All non-zero token accounts: {} tokens", all_non_zero_accounts.len());

                    // STEP 2: Fetch token metadata from Jupiter Token API
                    let mint_addresses: Vec<String> = all_non_zero_accounts.iter()
                        .map(|account| account.mint.clone())
                        .collect();

                    let token_metadata = if !mint_addresses.is_empty() {
                        match prices::get_token_metadata(mint_addresses).await {
                            Ok(metadata) => {
                                println!("Successfully fetched metadata for {} tokens", metadata.len());
                                metadata
                            },
                            Err(e) => {
                                println!("Error fetching token metadata: {}", e);
                                HashMap::new()
                            }
                        }
                    } else {
                        HashMap::new()
                    };

                    // STEP 3: Build mint->symbol mapping for price fetching (updated)
                    let mut mint_to_symbol_map = HashMap::new();
                    for account in &all_non_zero_accounts {
                        let symbol = if let Some(metadata) = token_metadata.get(&account.mint) {
                            // Use metadata from Jupiter Token API
                            metadata.symbol.clone()
                        } else if let Some(verified_token) = verified_tokens_map.get(&account.mint) {
                            // Use verified token name
                            verified_token.symbol.clone()
                        } else {
                            // Use truncated mint address as symbol for unknown tokens
                            if account.mint.len() >= 8 {
                                format!("{}...{}", &account.mint[..4], &account.mint[account.mint.len()-4..])
                            } else {
                                account.mint.clone()
                            }
                        };
                        mint_to_symbol_map.insert(account.mint.clone(), symbol);
                    }

                    // STEP 4: Fetch prices for ALL tokens
                    println!("Fetching prices for {} discovered tokens", mint_to_symbol_map.len());
                    let token_prices_result = if !mint_to_symbol_map.is_empty() {
                        prices::get_prices_for_tokens(mint_to_symbol_map.clone()).await
                    } else {
                        Ok(HashMap::new())
                    };

                    let dynamic_token_prices = match token_prices_result {
                        Ok(prices) => {
                            println!("Successfully fetched prices for {} tokens", prices.len());
                            prices
                        },
                        Err(e) => {
                            println!("Error fetching dynamic prices: {}", e);
                            HashMap::new()
                        }
                    };

                    // STEP 5: Create tokens for display with metadata
                    let new_tokens = all_non_zero_accounts
                        .into_iter()
                        .map(|account| {
                            let symbol = mint_to_symbol_map.get(&account.mint)
                                .cloned()
                                .unwrap_or_else(|| format!("UNKNOWN_{}", &account.mint[..6]));
                            
                            // Get token metadata from Jupiter API or verified tokens
                            let (token_name, icon_url) = if let Some(metadata) = token_metadata.get(&account.mint) {
                                (metadata.name.clone(), metadata.icon.clone())
                            } else if let Some(verified_token) = verified_tokens_map.get(&account.mint) {
                                (verified_token.name.clone(), Some(verified_token.logo_uri.clone()))
                            } else {
                                (format!("Token {}", &symbol), None)
                            };
                            
                            // Get price from dynamic prices, fallback to hardcoded snapshot, then to 1.0
                            let price = dynamic_token_prices.get(&symbol)
                                .copied()
                                .or_else(|| token_prices_snapshot.get(&symbol).copied())
                                .unwrap_or_else(|| {
                                    match symbol.as_str() {
                                        "USDC" | "USDT" => 1.0,
                                        _ => 0.0, // Show $0 for unknown token prices
                                    }
                                });
                            
                            // Get multi-timeframe changes
                            let multi_data_snapshot = multi_timeframe_data.read().clone();
                            let (change_1d, change_3d, change_7d) = get_multi_timeframe_changes(&symbol, &multi_data_snapshot);
                            
                            println!("Creating token {}: price=${:.4}, 1D={:.1}%, 3D={:.1}%, 7D={:.1}%", 
                                    symbol, price, change_1d, change_3d, change_7d);
                            
                            let value_usd = account.amount * price;
                            
                            // Determine icon to use - prioritize real icons from metadata
                            let icon_type = if let Some(icon_url) = icon_url {
                                if !icon_url.is_empty() {
                                    icon_url  // Use real icon from Jupiter Token API
                                } else {
                                    get_fallback_icon(&symbol)  // Use fallback for empty URLs
                                }
                            } else {
                                get_fallback_icon(&symbol)  // Use fallback for no metadata
                            };
                            
                        Token {
                            mint: account.mint.clone(),
                            symbol: symbol.clone(),
                            name: token_name,
                            icon_type,
                            balance: account.amount,
                            value_usd,
                            price,
                            price_change: change_1d,
                            price_change_1d: change_1d,
                            price_change_3d: change_3d,
                            price_change_7d: change_7d,
                            decimals: account.decimals,
                        }
                        })
                        .collect::<Vec<Token>>();
                        
                    // Get the most recent SOL price
                    let current_sol_price = token_prices_snapshot.get("SOL").copied().unwrap_or(sol_price());

                    // Get multi-timeframe changes  
                    let multi_data_snapshot = multi_timeframe_data.read().clone();
                    let (sol_change_1d, sol_change_3d, sol_change_7d) = get_multi_timeframe_changes("SOL", &multi_data_snapshot);

                    let mut all_tokens_raw = {
                    let mut raw_tokens = vec![Token {
                        mint: "So11111111111111111111111111111111111111112".to_string(),
                        symbol: "SOL".to_string(),
                        name: "Solana".to_string(),
                        icon_type: ICON_SOL.to_string(),
                        balance: balance(),
                        value_usd: balance() * current_sol_price,
                        price: current_sol_price,
                        price_change: sol_change_1d,
                        price_change_1d: sol_change_1d,
                        price_change_3d: sol_change_3d,
                        price_change_7d: sol_change_7d,
                        decimals: 9, // SOL has 9 decimals
                    }];
                    raw_tokens.extend(new_tokens);
                    raw_tokens
                };

                // Use the new processing system
                let processed_tokens = process_tokens_for_display(
                    all_tokens_raw,
                    &token_prices_snapshot,
                    &token_sort_config.read(),
                    &token_filter.read(),
                );

                // Convert back to Token structs for compatibility
                let final_tokens: Vec<Token> = processed_tokens
                    .into_iter()
                    .map(|td| td.token)
                    .collect();

                tokens.set(final_tokens);
            } else {
                println!("No token accounts found for address {}", address);
                
                // Get the most recent SOL price
                let current_sol_price = token_prices_snapshot.get("SOL").copied().unwrap_or(sol_price());
                
                // Get multi-timeframe changes
                let multi_data_snapshot = multi_timeframe_data.read().clone();
                let (sol_change_1d, sol_change_3d, sol_change_7d) = get_multi_timeframe_changes("SOL", &multi_data_snapshot);
                
                tokens.set(vec![Token {
                    mint: "So11111111111111111111111111111111111111112".to_string(),
                    symbol: "SOL".to_string(),
                    name: "Solana".to_string(),
                    icon_type: ICON_SOL.to_string(),
                    balance: balance(),
                    value_usd: balance() * current_sol_price,
                    price: current_sol_price,
                    price_change: sol_change_1d,
                    price_change_1d: sol_change_1d,
                    price_change_3d: sol_change_3d,
                    price_change_7d: sol_change_7d,
                    decimals: 9, // SOL has 9 decimals
                }]);
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

    use_effect(move || {
        if active_tab() == "collectibles" && collectibles().is_empty() && !collectibles_loading() {
            collectibles_loading.set(true);
            
            // Get the wallet address - CORRECTED to use .address instead of .public_key
            let wallet_address = if let Some(hw_pubkey) = hardware_pubkey() {
                hw_pubkey
            } else if !wallets().is_empty() {
                wallets()[current_wallet_index()].address.clone()  // ← FIXED: use .address
            } else {
                collectibles_loading.set(false);
                return; // No wallet available
            };
            
            let rpc_url = custom_rpc();
            
            spawn(async move {
                match fetch_collectibles(&wallet_address, rpc_url.as_deref()).await {
                    Ok(nfts) => {
                        println!("✅ Fetched {} collectibles", nfts.len());
                        collectibles.set(nfts);
                    },
                    Err(e) => {
                        println!("❌ Failed to fetch collectibles: {}", e);
                        collectibles.set(vec![]);
                    }
                }
                collectibles_loading.set(false);
            });
        }
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
                    style: "position: relative; overflow: visible;",
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
                        },
                        style: "position: absolute !important; top: auto !important; right: -2px !important; bottom: -2px !important; left: auto !important; margin: 0 !important; padding: 0 !important; display: block !important; transform: none !important; transition: background-color 0.3s ease, box-shadow 0.3s ease !important; transition-property: background-color, box-shadow !important;"
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
                        //if let Some(ref wallet) = current_wallet {
                        //    div {
                        //        class: "dropdown-item current-wallet current-wallet-highlighted", // ← ADD custom class
                        //        div {
                        //            class: "dropdown-icon wallet-icon",
                        //            "💼"
                        //        }
                        //        div {
                        //            class: "wallet-info",
                        //            div { class: "wallet-name", "{wallet.name}" }
                        //            div { class: "wallet-address", "{wallet_address}" }
                        //        }
                        //    }
                        //}
                        
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
                                    img {
                                        src: "{ICON_WALLET}",
                                        alt: "Wallet",
                                        style: "width: 24px; height: 24px;"
                                    }
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
                                img {
                                    src: "{ICON_CREATE}",
                                    alt: "Import",
                                    style: "width: 24px; height: 24px;"
                                }
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
                                img {
                                    src: "{ICON_IMPORT}",
                                    alt: "Import",
                                    style: "width: 24px; height: 24px;"
                                }
                            }
                            "Import Wallet"
                        }

                        if current_wallet.is_some() && !hardware_connected() {
                            button {
                                class: "dropdown-item",
                                onclick: move |_| {
                                    show_export_modal.set(true);
                                    show_dropdown.set(false);
                                },
                                div {
                                    class: "dropdown-icon action-icon",
                                    img {
                                        src: "{ICON_EXPORT}",
                                        alt: "Export",
                                        style: "width: 24px; height: 24px;"
                                    }
                                }
                                "Export Wallet"
                            }
                        }

                        // NEW: Delete Wallet button (only show if there's a current wallet and not hardware)
                        if current_wallet.is_some() && !hardware_connected() {
                            button {
                                class: "dropdown-item delete-item",
                                onclick: move |_| {
                                    show_delete_confirmation.set(true);
                                    show_dropdown.set(false);
                                },
                                div {
                                    class: "dropdown-icon action-icon danger",
                                    img {
                                        src: "{ICON_DELETE}",
                                        alt: "Delete",
                                        style: "width: 24px; height: 24px;"
                                    }
                                }
                                "Delete Wallet"
                            }
                        }
                        
                        //if hardware_device_present() && !hardware_connected() {
                        //    button {
                        //        class: "dropdown-item",
                        //        onclick: move |_| {
                        //            show_hardware_modal.set(true);
                        //            show_dropdown.set(false);
                        //        },
                        //        div {
                        //            class: "dropdown-icon action-icon",
                        //            "🔐"
                        //        }
                        //        "Connect Hardware Wallet"
                        //    }
                        //}
                        
                        div { class: "dropdown-divider" }
                        
                        button {
                            class: "dropdown-item",
                            onclick: move |_| {
                                show_rpc_modal.set(true);
                                show_dropdown.set(false);
                            },
                            div {
                                class: "dropdown-icon action-icon",
                                img {
                                    src: "{ICON_RPC}",
                                    alt: "RPC",
                                    style: "width: 24px; height: 24px;"
                                }
                            }
                            "RPC Settings"
                        }
                
                        //button {
                        //    class: "dropdown-item",
                        //    onclick: move |_| {
                        //        show_background_modal.set(true);
                        //        show_dropdown.set(false);
                        //    },
                        //    div {
                        //        class: "dropdown-icon action-icon",
                        //        "🎨"
                        //    }
                        //    "Change Background"
                        //}

                        //button {
                        //    class: "dropdown-item",
                        //    onclick: move |_| {
                        //        show_jito_modal.set(true);
                        //        show_dropdown.set(false);
                        //    },
                        //    div {
                        //        class: "dropdown-icon action-icon",
                        //        "⚡"
                        //    }
                        //    "JITO Settings"
                        //}
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

            // Export Wallet Modal
            if show_export_modal() {
                ExportWalletModal {
                    wallet: wallets.read().get(current_wallet_index()).cloned(),
                    onclose: move |_| show_export_modal.set(false)
                }
            }

            // Delete Wallet Confirmation Modal  
            if show_delete_confirmation() {
                DeleteWalletModal {
                    wallet: wallets.read().get(current_wallet_index()).cloned(),
                    onconfirm: move |_| {
                        // Get the current wallet info for deletion - separate the read operation
                        let current_index = current_wallet_index();
                        let wallet_address_to_delete = {
                            // This scope ensures the read lock is dropped before we try to write
                            wallets.read().get(current_index).map(|w| w.address.clone())
                        };
                        
                        if let Some(wallet_address) = wallet_address_to_delete {
                            // Delete the wallet from storage
                            delete_wallet_from_storage(&wallet_address);
                            
                            // Reload wallets from storage (now we can safely write)
                            wallets.set(load_wallets_from_storage());
                            
                            // Reset current index if needed
                            let wallet_count = wallets.read().len();
                            if wallet_count == 0 {
                                current_wallet_index.set(0);
                            } else if current_index >= wallet_count {
                                current_wallet_index.set(wallet_count - 1);
                            }
                            
                            // Reset balance
                            balance.set(0.0);
                        }
                        show_delete_confirmation.set(false);
                    },
                    onclose: move |_| show_delete_confirmation.set(false)
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
                        hardware_device_type.set(None);
                        show_hardware_modal.set(false);
                    },
                    onsuccess: move |hw_wallet: Arc<HardwareWallet>| {
                        hardware_wallet.set(Some(hw_wallet.clone()));
                        hardware_connected.set(true);
                        show_hardware_modal.set(false);
                        
                        let hw_clone = hw_wallet.clone();
                        spawn(async move {
                            if let Ok(pubkey) = hw_wallet.get_public_key().await {
                                hardware_pubkey.set(Some(pubkey));
                            }
                            
                            // Get and set the device type - clone it for the println
                            if let Some(dev_type) = hw_clone.get_device_type().await {
                                println!("🔧 Set device type to: {:?}", dev_type); // Use it first
                                hardware_device_type.set(Some(dev_type)); // Then move it
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

            // Show Transaction History modal
            if show_history_modal() {
                TransactionHistoryModal {
                    // Use the already-computed address that respects hardware wallet overrides
                    address: full_address.clone(),
                    custom_rpc: custom_rpc(),
                    onclose: move |_| {
                        show_history_modal.set(false);
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

            if show_bulk_send_modal() {
                BulkSendModal {
                    selected_token_mints: selected_tokens(),
                    all_tokens: tokens(),
                    wallet: current_wallet.clone(),
                    hardware_wallet: hardware_wallet(),
                    current_balance: balance(),
                    custom_rpc: custom_rpc(),
                    onclose: move |_| {
                        show_bulk_send_modal.set(false);
                        bulk_send_mode.set(false);
                        selected_tokens.set(HashSet::new());
                    },
                    onsuccess: move |signature| {
                        show_bulk_send_modal.set(false);
                        bulk_send_mode.set(false);
                        selected_tokens.set(HashSet::new());
                        println!("Bulk send transaction successful: {}", signature);
                        
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
                                        println!("Failed to refresh balance after bulk send: {}", e);
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

            if show_swap_modal() {
                SwapModal {
                    tokens: tokens(),  // Use tokens() instead of filtered_tokens
                    wallet: current_wallet.clone(),  // Use current_wallet instead of current_wallet_opt()
                    hardware_wallet: hardware_wallet(),  // Use hardware_wallet() to get the value
                    //current_balance: balance(),  // Use balance() instead of sol_balance()
                    custom_rpc: custom_rpc(),  // Use custom_rpc() instead of custom_rpc_url()
                    onclose: move |_| show_swap_modal.set(false),
                    onsuccess: move |signature| {
                        show_swap_modal.set(false);
                        // You can add success handling here if needed
                        println!("Swap successful: {}", signature);
                    }
                }
            }

            if show_lend_modal() {
                LendModal {
                    tokens: tokens(),
                    wallet: current_wallet.clone(),
                    hardware_wallet: hardware_wallet(),
                    custom_rpc: custom_rpc(),
                    onclose: move |_| show_lend_modal.set(false),
                    onsuccess: {
                        let wallet_for_refresh = current_wallet.clone();
                        move |signature| {
                            println!("✅ Lend completed with signature: {}", signature);
                            show_lend_modal.set(false);
                            // Refresh balances after successful lend
                            if let Some(wallet) = wallet_for_refresh.clone() {
                                let address = wallet.address.clone();
                                let rpc_url = custom_rpc();
                                
                                spawn(async move {
                                    match rpc::get_balance(&address, rpc_url.as_deref()).await {
                                        Ok(sol_balance) => {
                                            balance.set(sol_balance);
                                        }
                                        Err(e) => {
                                            println!("Failed to refresh balance after lend: {}", e);
                                        }
                                    }
                                });
                            }
                        }
                    }
                }
            }

            if show_squads_modal() {
                SquadsModal {
                    wallet: current_wallet.clone(),
                    hardware_wallet: hardware_wallet(),
                    custom_rpc: custom_rpc(),
                    onclose: move |_| show_squads_modal.set(false),
                }
            }
            
            if show_carrot_modal() {
                CarrotModal {
                    wallet: current_wallet.clone(),
                    hardware_wallet: hardware_wallet(),
                    custom_rpc: custom_rpc(),
                    onclose: move |_| show_carrot_modal.set(false),
                }
            }
            
            if show_bonk_staking_modal() {
                BonkStakingModal {
                    tokens: tokens(),
                    wallet: current_wallet.clone(),
                    hardware_wallet: hardware_wallet(),
                    custom_rpc: custom_rpc(),
                    onclose: move |_| show_bonk_staking_modal.set(false),
                    onsuccess: move |sig| {
                        println!("BONK stake successful: {}", sig);
                        // Trigger wallet refresh
                        refresh_trigger.set(refresh_trigger() + 1);
                    },
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
                    class: "balance-section-segmented",
                    
                    // Left side - Balance content
                    div {
                        class: "balance-content",
                        
                        div {
                            class: "balance-label",
                            "Your Balance"
                        }
                        
                        div {
                            class: "balance-amount-large",
                            if prices_loading() {
                                "Loading..."
                            } else {
                                // Calculate total portfolio value (sum of all token values) and round to nearest dollar
                                {
                                    let total_value = tokens.read().iter().fold(0.0, |acc, token| acc + token.value_usd);
                                    format_portfolio_balance(total_value)
                                }
                            }
                        }
                    }
                    
                    // Right side - Device/Wallet indicator
                    div {
                        class: "device-indicator",
                        onclick: move |e| {
                            e.stop_propagation();
                            
                            if is_refreshing() {
                                println!("⏳ Already refreshing, please wait...");
                                return;
                            }
                            
                            println!("🔄 Tapped device indicator - showing spinner...");
                            is_refreshing.set(true);
                            refresh_trigger.set(refresh_trigger() + 1);
                            
                            spawn(async move {
                                tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                                is_refreshing.set(false);
                                println!("✅ Refresh animation complete");
                            });
                        },
                        
                        if is_refreshing() {
                            img { 
                                src: LOADING_SPINNER,
                                alt: "Refreshing...",
                                style: "cursor: pointer;"
                            }
                        } else if hardware_connected() {
                            match hardware_device_type() {
                                Some(HardwareDeviceType::ESP32) => rsx! {
                                    img { 
                                        src: DEVICE_UNRGBL,
                                        alt: "Unruggable Hardware Wallet - Tap to Refresh",
                                        style: "cursor: pointer;"
                                    }
                                },
                                Some(HardwareDeviceType::Ledger) => rsx! {
                                    img { 
                                        src: DEVICE_LEDGER,
                                        alt: "Ledger Hardware Wallet - Tap to Refresh",
                                        style: "cursor: pointer;"
                                    }
                                },
                                None => rsx! {
                                    img { 
                                        src: DEVICE_UNRGBL,
                                        alt: "Hardware Wallet - Tap to Refresh",
                                        style: "cursor: pointer;"
                                    }
                                }
                            }
                        } else {
                            img { 
                                src: DEVICE_SOFTWARE,
                                alt: "Software Wallet - Tap to Refresh",
                                style: "cursor: pointer;"
                            }
                        }
                    }
                }
                
                // Replace the current action-buttons div with this segmented version
                div {
                    class: "action-buttons-segmented",
                    
                    // Primary action buttons row (always visible)
                    div {
                        class: "action-buttons-grid",
                        
                        button {
                            class: "action-button-segmented",
                            onclick: move |_| show_receive_modal.set(true),
                            
                            div {
                                class: "action-icon-segmented",
                                img { 
                                    src: "{ICON_RECEIVE}",
                                    alt: "Receive"
                                }
                            }
                            
                            div {
                                class: "action-label-segmented",
                                "Receive"
                            }
                        }
                        
                        button {
                            class: "action-button-segmented",
                            onclick: move |_| {
                                if bulk_send_mode() {
                                    // Exit bulk mode
                                    bulk_send_mode.set(false);
                                    selected_tokens.set(HashSet::new());
                                } else {
                                    // Enter bulk send mode
                                    bulk_send_mode.set(true);
                                    selected_tokens.set(HashSet::new()); // Clear previous selections
                                }
                            },
                            
                            div {
                                class: "action-icon-segmented",
                                if bulk_send_mode() {
                                    div {
                                        style: "font-size: 24px; color: white;",
                                        "❌" // Keep cancel emoji
                                    }
                                } else {
                                    img {
                                        src: "{ICON_BULK}",
                                        alt: "Send"
                                    }
                                }
                            }
                            
                            div {
                                class: "action-label-segmented",
                                if bulk_send_mode() {
                                    "Cancel"
                                } else {
                                    "Send"
                                }
                            }
                        }
                        
                        button {
                            class: "action-button-segmented",
                            onclick: move |_| show_stake_modal.set(true),
                            
                            div {
                                class: "action-icon-segmented",
                                img { 
                                    src: "{ICON_STAKE}",
                                    alt: "Stake"
                                }
                            }
                            
                            div {
                                class: "action-label-segmented",
                                "Stake"
                            }
                        }
                        
                        button {
                            class: "action-button-segmented",
                            onclick: move |_| show_swap_modal.set(true),
                            
                            div {
                                class: "action-icon-segmented",
                                img { 
                                    src: "{ICON_SWAP}",
                                    alt: "Swap"
                                }
                            }
                            
                            div {
                                class: "action-label-segmented",
                                "Swap"
                            }
                        }
                        
                        // Integrations button (replaces Lend in primary row)
                        button {
                            class: "action-button-segmented",
                            onclick: move |_| {
                                show_integrations.set(!show_integrations());
                                println!("Integrations button clicked - showing: {}", !show_integrations());
                            },
                            
                            div {
                                class: "action-icon-segmented",
                                div {
                                    style: "font-size: 20px; color: white;",
                                    if show_integrations() {
                                        "▼"
                                    } else {
                                        "▶"
                                    }
                                }
                            }
                            
                            div {
                                class: "action-label-segmented",
                                "Integrations"
                            }
                        }
                    }
                    
                    // Integrations row (conditional - only shown when integrations are expanded)
                    if show_integrations() {
                        div {
                            class: "integrations-row",
                            
                            button {
                                class: "action-button-segmented",
                                onclick: move |_| {
                                    println!("Lend button clicked!");
                                    show_lend_modal.set(true);
                                },
                                
                                div {
                                    class: "action-icon-segmented",
                                    img { 
                                        src: "{ICON_LEND}",
                                        alt: "Lend"
                                    }
                                }
                                
                                div {
                                    class: "action-label-segmented",
                                    "Lend"
                                }
                            }
                            
                            button {
                                class: "action-button-segmented",
                                onclick: move |_| {
                                    println!("Squads button clicked!");
                                    show_squads_modal.set(true);
                                },
                                
                                div {
                                    class: "action-icon-segmented",
                                    img { 
                                        src: "{ICON_SQUADS}",
                                        alt: "Squads"
                                    }
                                }
                                
                                div {
                                    class: "action-label-segmented",
                                    "Squads"
                                }
                            }
                            
                            button {
                                class: "action-button-segmented",
                                onclick: move |_| {
                                    println!("Carrot button clicked!");
                                    show_carrot_modal.set(true);
                                },
                                
                                div {
                                    class: "action-icon-segmented",
                                    img { 
                                        src: "{ICON_CARROT}",
                                        alt: "Carrot"
                                    }
                                }
                                
                                div {
                                    class: "action-label-segmented",
                                    "Carrot"
                                }
                            }
                            
                            button {
                                class: "action-button-segmented",
                                onclick: move |_| {
                                    println!("BONK Stake button clicked!");
                                    show_bonk_staking_modal.set(true);
                                },
                                
                                div {
                                    class: "action-icon-segmented",
                                    img { 
                                        src: "{ICON_BONK_STAKE}",
                                        alt: "BONK Stake"
                                    }
                                }
                                
                                div {
                                    class: "action-label-segmented",
                                    "BONK Stake"
                                }
                            }
                        }
                    }
                }
            }
            
            div {
                class: "tokens-section",
                
                // Tab headers with existing bulk send functionality
                div {
                    class: "tokens-tabs-header",
                    div {
                        class: "tabs-container",
                        button {
                            class: if active_tab() == "tokens" { "tab-button active" } else { "tab-button" },
                            onclick: move |_| active_tab.set("tokens".to_string()),
                            if bulk_send_mode() && active_tab() == "tokens" {
                                "Select Tokens"
                            } else {
                                "Your Tokens"
                            }
                        }
                        button {
                            class: if active_tab() == "collectibles" { "tab-button active" } else { "tab-button" },
                            onclick: move |_| active_tab.set("collectibles".to_string()),
                            "Collectibles"
                        }
                    }
                    
                    // Show bulk send button only when on tokens tab and in bulk mode with selections
                    if active_tab() == "tokens" && bulk_send_mode() && !selected_tokens().is_empty() {
                        button {
                            class: "bulk-send-confirm-button",
                            onclick: move |_| {
                                show_bulk_send_modal.set(true);
                            },
                            "Send ({selected_tokens().len()})"
                        }
                    }
                }
                
                // Tab content
                match active_tab().as_str() {
                    "tokens" => rsx! {
                        div {
                            class: "token-list",
                            for token in tokens() {
                                {
                                    // Clone all the values we'll need to avoid borrow checker issues
                                    let token_mint = token.mint.clone();
                                    let token_symbol = token.symbol.clone();
                                    let token_name = token.name.clone();
                                    let token_icon = token.icon_type.clone();
                                    let token_price = token.price;
                                    let token_balance = token.balance;
                                    let token_value_usd = token.value_usd;
                                    
                                    rsx! {
                                        div {
                                            key: "{token_mint}",
                                            class: if bulk_send_mode() && selected_tokens().contains(&token_mint) {
                                                "token-item token-item-selected"
                                            } else {
                                                "token-item"
                                            },
                                            // Add click handler for bulk selection
                                            onclick: {
                                                let mint_clone = token_mint.clone();
                                                let is_bulk_mode = bulk_send_mode();
                                                move |_| {
                                                    if is_bulk_mode {
                                                        let mut current_selected = selected_tokens();
                                                        if current_selected.contains(&mint_clone) {
                                                            current_selected.remove(&mint_clone);
                                                        } else {
                                                            current_selected.insert(mint_clone.clone());
                                                        }
                                                        selected_tokens.set(current_selected);
                                                    }
                                                }
                                            },
                                            
                                            // Main token row
                                            div {
                                                class: "token-row-main",
                                                
                                                // Add selection checkbox when in bulk mode
                                                if bulk_send_mode() {
                                                    div {
                                                        class: "token-selection-checkbox",
                                                        input {
                                                            r#type: "checkbox",
                                                            checked: selected_tokens().contains(&token_mint),
                                                            onclick: move |e| e.stop_propagation(),
                                                        }
                                                    }
                                                }
                                                
                                                div {
                                                    class: "token-info",
                                                    div {
                                                        class: "token-icon",
                                                        img {
                                                            src: "{token_icon}",
                                                            alt: "{token_symbol}",
                                                            width: "32",
                                                            height: "32",
                                                            style: "border-radius: 50%;",
                                                            onerror: {
                                                                let symbol = token_symbol.clone();
                                                                let icon_type = token_icon.clone();
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
                                                            "{token_name} ({token_symbol})"
                                                        }
                                                        div {
                                                            class: "token-price-info",
                                                            // Clickable price section for charts
                                                            div {
                                                                class: "token-price-container",
                                                                onclick: {
                                                                    let mint_clone = token_mint.clone();
                                                                    let symbol_clone = token_symbol.clone();
                                                                    let is_stablecoin = matches!(token_symbol.as_str(), "USDC" | "USDT");
                                                                    move |e| {
                                                                        e.stop_propagation();
                                                                        // Only allow expansion for non-stablecoins
                                                                        if !is_stablecoin {
                                                                            let mut current_expanded = expanded_tokens();
                                                                            let is_expanding = !current_expanded.contains(&mint_clone);
                                                                            
                                                                            if current_expanded.contains(&mint_clone) {
                                                                                current_expanded.remove(&mint_clone);
                                                                            } else {
                                                                                current_expanded.insert(mint_clone.clone());
                                                                                
                                                                                // Fetch chart data when expanding
                                                                                if is_expanding {
                                                                                    let cache_key = format!("{}_1D", symbol_clone);
                                                                                    if !chart_data().contains_key(&cache_key) {
                                                                                        spawn(fetch_chart_data_with_timeframe(symbol_clone.clone(), "1D".to_string(), chart_data, chart_loading));
                                                                                    }
                                                                                }
                                                                            }
                                                                            expanded_tokens.set(current_expanded);
                                                                        }
                                                                    }
                                                                },
                                                                span {
                                                                    class: "token-price",
                                                                    "${token_price:.2}"
                                                                }
                                                                // Show expand indicator for non-stablecoins
                                                                if !matches!(token_symbol.as_str(), "USDC" | "USDT") {
                                                                    span {
                                                                        class: "price-expand-indicator",
                                                                        if expanded_tokens().contains(&token_mint) {
                                                                            "▼"
                                                                        } else {
                                                                            "▶"
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                
                                                // Individual send button - ONLY show when NOT in bulk mode
                                                if !bulk_send_mode() {
                                                    button {
                                                        class: "token-send-button",
                                                        onclick: {
                                                            let symbol_clone = token_symbol.clone();
                                                            let mint_clone = token_mint.clone();
                                                            let token_decimals = match token_symbol.as_str() {
                                                                "SOL" => Some(9),
                                                                "USDC" | "USDT" => Some(6),
                                                                _ => Some(9),
                                                            };
                                                            
                                                            move |e| {
                                                                e.stop_propagation();
                                                                if symbol_clone == "SOL" {
                                                                    show_send_modal.set(true);
                                                                } else {
                                                                    selected_token_symbol.set(symbol_clone.clone());
                                                                    selected_token_mint.set(mint_clone.clone());
                                                                    selected_token_balance.set(token_balance);
                                                                    selected_token_decimals.set(token_decimals);
                                                                    show_send_token_modal.set(true);
                                                                }
                                                            }
                                                        },
                                                        title: "Send {token_symbol}",
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
                                                }
                                                
                                                div {
                                                    class: "token-values",
                                                    div {
                                                        class: "token-value-usd",
                                                        "{format_token_value_smart(token_balance, token_price)}"
                                                    }
                                                    div {
                                                        class: "token-amount",
                                                        "{format_token_amount(token_balance, &token_symbol)}"
                                                    }
                                                }
                                            }
                                            
                                            // Chart section (spans full width BELOW the token row)
                                            if !matches!(token_symbol.as_str(), "USDC" | "USDT") && expanded_tokens().contains(&token_mint) {
                                                div {
                                                    class: "token-chart-expanded-fullwidth",
                                                    
                                                    {
                                                        let current_timeframe = selected_timeframe.read().get(&token_symbol).cloned().unwrap_or("1D".to_string());
                                                        let cache_key = format!("{}_{}", token_symbol, current_timeframe);
                                                        let has_chart_data = chart_data().contains_key(&cache_key);
                                                        let chart_data_clone = chart_data().get(&cache_key).cloned();
                                                        
                                                        // Clone the timeframe for multiple uses
                                                        let timeframe_for_buttons = current_timeframe.clone();
                                                        let timeframe_for_chart = current_timeframe.clone();
                                                        
                                                        rsx! {
                                                            // Timeframe selector buttons
                                                            div {
                                                                class: "chart-timeframe-selector",
                                                                
                                                                button {
                                                                    class: if timeframe_for_buttons == "1H" { "timeframe-btn active" } else { "timeframe-btn" },
                                                                    onclick: {
                                                                        let symbol_clone = token_symbol.clone();
                                                                        move |_| {
                                                                            let mut timeframes = selected_timeframe();
                                                                            timeframes.insert(symbol_clone.clone(), "1H".to_string());
                                                                            selected_timeframe.set(timeframes);
                                                                            
                                                                            let symbol_for_fetch = symbol_clone.clone();
                                                                            spawn(async move {
                                                                                fetch_chart_data_with_timeframe(symbol_for_fetch, "1H".to_string(), chart_data, chart_loading).await;
                                                                            });
                                                                        }
                                                                    },
                                                                    "1H"
                                                                }
                                                                
                                                                button {
                                                                    class: if timeframe_for_buttons == "1D" { "timeframe-btn active" } else { "timeframe-btn" },
                                                                    onclick: {
                                                                        let symbol_clone = token_symbol.clone();
                                                                        move |_| {
                                                                            let mut timeframes = selected_timeframe();
                                                                            timeframes.insert(symbol_clone.clone(), "1D".to_string());
                                                                            selected_timeframe.set(timeframes);
                                                                            
                                                                            let symbol_for_fetch = symbol_clone.clone();
                                                                            spawn(async move {
                                                                                fetch_chart_data_with_timeframe(symbol_for_fetch, "1D".to_string(), chart_data, chart_loading).await;
                                                                            });
                                                                        }
                                                                    },
                                                                    "1D"
                                                                }
                                                            }
                                                            
                                                            // Show loading state
                                                            if chart_loading().contains(&cache_key) {
                                                                div {
                                                                    class: "chart-loading",
                                                                    "📊 Loading chart data..."
                                                                }
                                                            }
                                                            // Show the actual chart
                                                            else if has_chart_data {
                                                                if let Some(candlesticks) = chart_data_clone {
                                                                    CandlestickChart {
                                                                        data: candlesticks,
                                                                        symbol: token_symbol.clone(),
                                                                        timeframe: timeframe_for_chart,
                                                                    }
                                                                }
                                                            }
                                                            // Show error state
                                                            else {
                                                                div {
                                                                    class: "chart-error",
                                                                    "📈 No chart data available"
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
                    },
                    "collectibles" => rsx! {
                        div {
                            class: "collectibles-list",
                            if collectibles_loading() {
                                div {
                                    class: "empty-state",
                                    div {
                                        class: "empty-icon",
                                        "⏳"
                                    }
                                    div {
                                        class: "empty-message",
                                        "Loading collectibles..."
                                    }
                                    div {
                                        class: "empty-description",
                                        "Fetching your NFTs and digital assets"
                                    }
                                }
                            } else if collectibles().is_empty() {
                                div {
                                    class: "empty-state",
                                    div {
                                        class: "empty-icon",
                                        "🎨"
                                    }
                                    div {
                                        class: "empty-message",
                                        "No collectibles found"
                                    }
                                    div {
                                        class: "empty-description",
                                        "Your NFTs and collectibles will appear here"
                                    }
                                }
                            } else {
                                div {
                                    class: "collectibles-grid",
                                    for collectible in collectibles() {
                                        {
                                            // Clone the values we need to avoid borrow checker issues
                                            let collectible_mint = collectible.mint.clone();
                                            let collectible_name = collectible.name.clone();
                                            let collectible_collection = collectible.collection.clone();
                                            let collectible_image = collectible.image.clone();
                                            let collectible_verified = collectible.verified;
                                            
                                            rsx! {
                                                div {
                                                    key: "{collectible_mint}",
                                                    class: "collectible-item",
                                                    onclick: {
                                                        let name_clone = collectible_name.clone();
                                                        move |_| {
                                                            println!("Clicked collectible: {}", name_clone);
                                                        }
                                                    },
                                                    div {
                                                        class: "collectible-image",
                                                        img {
                                                            src: "{collectible_image}",
                                                            alt: "{collectible_name}",
                                                            onerror: {
                                                                let name_clone = collectible_name.clone();
                                                                move |_| {
                                                                    println!("Failed to load collectible image: {}", name_clone);
                                                                }
                                                            },
                                                        }
                                                    }
                                                    div {
                                                        class: "collectible-info",
                                                        div {
                                                            class: "collectible-name",
                                                            "{collectible_name}"
                                                        }
                                                        div {
                                                            class: "collectible-collection",
                                                            "{collectible_collection}"
                                                        }
                                                        if collectible_verified {
                                                            div {
                                                                class: "collectible-verified",
                                                                "✅"
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
                    },
                    _ => rsx! { div {} }
                }
            }
        }
    }
}