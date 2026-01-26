use dioxus::document::eval;
use dioxus::prelude::*;
use std::sync::Arc;
use serde_json::Value;
use std::sync::OnceLock;

mod wallet;
mod rpc;
mod prices;
mod transaction;
mod signing;
mod hardware;
mod storage;
mod components;
mod validators;
mod staking;
mod unstaking;
mod currency;
mod currency_utils;
mod sns;
mod config;
mod token_utils;
mod privacycash;
// Temporarily disabled for Solana 3.x testing (these depend on Solana 2.x SDKs)
mod squads;
mod carrot;
mod bonk_staking;
mod titan;
mod quantum_vault;
mod pin;
mod timeout;

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android"), not(target_os = "ios")))]
mod bridge;

use components::*;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[route("/")]
    WalletView {},
}

// MAC and iOS bundling does not adhere to the asset! macro.
// Android does. For apple builds use hosted resources.

// For iOS/macOS builds, uncomment the remote URLs and comment out the asset! macros
//const MAIN_CSS_URL: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@solana-3x-tpu-test/assets/main.css";
//const PIN_CSS_URL: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@solana-3x-tpu-test/assets/pin-premium.css";
const PRIVACY_JS_URL: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@solana-3x-tpu-test/assets/privacy.js";
const PRIVACY_WASM_URL: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@solana-3x-tpu-test/assets/transaction2.wasm";
const PRIVACY_ZKEY_URL: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@solana-3x-tpu-test/assets/transaction2.zkey";
const LIQUID_METAL_JS_URL: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@solana-3x-tpu-test/assets/liquid_metal_component.js";
const LIQUID_METAL_SVG_JS_URL: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@solana-3x-tpu-test/assets/liquid_metal_svg.js";
const LIQUID_METAL_BORDER_JS_URL: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@solana-3x-tpu-test/assets/liquid_metal_border.js";
const LIQUID_METAL_CIRCLE_BORDER_JS_URL: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@solana-3x-tpu-test/assets/liquid_metal_circle_border.js";
const LIQUID_METAL_CIRCLE_JS_URL: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@solana-3x-tpu-test/assets/liquid_metal_circle.js";

// For local/Android builds, use the asset! macro
const MAIN_CSS: Asset = asset!("/assets/main.css");
const PIN_CSS: Asset = asset!("/assets/pin-premium.css");

// ── DESKTOP (macOS/Windows/Linux) ─────────────────────────────────────────────
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android"), not(target_os = "ios")))]
fn main() {
    // Hard-disable Dioxus edit server & devtools in the shipped app
    std::env::set_var("DIOXUS_DISABLE_EDIT", "1");
    std::env::set_var("DX_DISABLE_EDIT", "1");
    std::env::set_var("DIOXUS_DEVTOOLS", "0");

    // Optional: prove it's set when run from Terminal
    eprintln!(
        "DX edits OFF: DIOXUS_DISABLE_EDIT={:?}, DX_DISABLE_EDIT={:?}, DEVTOOLS={:?}",
        std::env::var("DIOXUS_DISABLE_EDIT"),
        std::env::var("DX_DISABLE_EDIT"),
        std::env::var("DIOXUS_DEVTOOLS")
    );

    dioxus::launch(App);
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android"), not(target_os = "ios")))]
fn start_browser_bridge() -> Arc<bridge::BridgeHandler> {
    use bridge::{BridgeHandler, BridgeServer};

    let handler = Arc::new(BridgeHandler::new());
    let handler_clone = Arc::clone(&handler);
    let bridge_enabled = storage::load_bridge_settings_from_storage().enabled;
    handler.set_enabled(bridge_enabled);

    println!("🌉 Starting browser bridge server...");
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

        rt.block_on(async {
            let server = Arc::new(BridgeServer::new(7777));
            let callback: bridge::RequestCallback = Arc::new(move |request| {
                let handler = handler_clone.clone();
                Box::pin(async move { handler.handle_request(request).await })
            });

            server.set_callback(callback);

            if let Err(e) = server.start().await {
                eprintln!("Bridge server error: {}", e);
            }
        });
    });

    handler
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android"), not(target_os = "ios")))]
fn init_bridge_handler() -> Arc<bridge::BridgeHandler> {
    static BRIDGE_HANDLER: OnceLock<Arc<bridge::BridgeHandler>> = OnceLock::new();
    BRIDGE_HANDLER.get_or_init(start_browser_bridge).clone()
}

// Web & Mobile keep the generic launcher:
#[cfg(any(target_arch = "wasm32", target_os = "android", target_os = "ios"))]
fn main() {
    dioxus::launch(App);
}


#[component]
fn App() -> Element {
    let (privacy_js_src, wasm_url, zkey_url, liquid_metal_js_src, liquid_metal_svg_js_src, liquid_metal_border_js_src, liquid_metal_circle_border_js_src, liquid_metal_circle_js_src) = if cfg!(any(
        target_arch = "wasm32"
    )) {
        (
            PRIVACY_JS_URL.to_string(),
            PRIVACY_WASM_URL.to_string(),
            PRIVACY_ZKEY_URL.to_string(),
            LIQUID_METAL_JS_URL.to_string(),
            LIQUID_METAL_SVG_JS_URL.to_string(),
            LIQUID_METAL_BORDER_JS_URL.to_string(),
            LIQUID_METAL_CIRCLE_BORDER_JS_URL.to_string(),
            LIQUID_METAL_CIRCLE_JS_URL.to_string(),
        )
    } else {
        let privacy_js = asset!("/assets/privacy.js", AssetOptions::builder().with_hash_suffix(false));
        let privacy_wasm = asset!("/assets/transaction2.wasm", AssetOptions::builder().with_hash_suffix(false));
        let privacy_zkey = asset!("/assets/transaction2.zkey", AssetOptions::builder().with_hash_suffix(false));
        let liquid_metal_js = asset!("/assets/liquid_metal_component.js", AssetOptions::builder().with_hash_suffix(false));
        let liquid_metal_svg_js = asset!("/assets/liquid_metal_svg.js", AssetOptions::builder().with_hash_suffix(false));
        let liquid_metal_border_js = asset!("/assets/liquid_metal_border.js", AssetOptions::builder().with_hash_suffix(false));
        let liquid_metal_circle_border_js = asset!("/assets/liquid_metal_circle_border.js", AssetOptions::builder().with_hash_suffix(false));
        let liquid_metal_circle_js = asset!("/assets/liquid_metal_circle.js", AssetOptions::builder().with_hash_suffix(false));
        (
            privacy_js.to_string(),
            privacy_wasm.to_string(),
            privacy_zkey.to_string(),
            liquid_metal_js.to_string(),
            liquid_metal_svg_js.to_string(),
            liquid_metal_border_js.to_string(),
            liquid_metal_circle_border_js.to_string(),
            liquid_metal_circle_js.to_string(),
        )
    };
    println!("[PrivacyCash] asset wasm url: {}", wasm_url);
    println!("[PrivacyCash] asset zkey url: {}", zkey_url);

    use_effect(move || {
        let wasm_url = wasm_url.clone();
        let zkey_url = zkey_url.clone();
        spawn(async move {
            let mut e = eval(
                r#"
                let [wasmUrl, zkeyUrl] = await dioxus.recv();
                globalThis.PRIVACY_CASH_WASM_URL = wasmUrl;
                globalThis.PRIVACY_CASH_ZKEY_URL = zkeyUrl;
                console.log('PrivacyCash asset globals set', { wasmUrl, zkeyUrl });
                "#,
            );
            let _ = e.send(Value::Array(vec![
                Value::String(wasm_url),
                Value::String(zkey_url),
            ]));
        });
    });
    // Check if onboarding has been completed
    let mut show_onboarding = use_signal(|| true);
    //let mut show_onboarding = use_signal(|| !storage::has_completed_onboarding());
    
    // Check if PIN is set and locked
    let mut is_locked = use_signal(|| storage::has_pin());
    
    // Initialize SNS resolver with your RPC endpoint
    let sns_resolver = Arc::new(sns::SnsResolver::new(
        "https://johna-k3cr1v-fast-mainnet.helius-rpc.com".to_string() // Use your preferred RPC endpoint
    ));

    // Provide SNS resolver to the entire app
    use_context_provider(|| sns_resolver.clone());

    // Provide a shared TransactionClient (no background TPU init to avoid iOS crash)
    let transaction_client = Arc::new(transaction::TransactionClient::new(None));
    use_context_provider(|| transaction_client.clone());

    // Start browser bridge on desktop only
    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "android"), not(target_os = "ios")))]
    let bridge_handler = {
        let handler = use_context_provider(|| init_bridge_handler());
        handler
    };
    
    let wallet = use_signal(|| None as Option<wallet::WalletInfo>);
    
    rsx! {
        // For iOS/macOS builds, uncomment these lines and comment out the asset! lines below
        document::Link { rel: "preconnect", href: "https://cdn.jsdelivr.net" }
        //document::Link { rel: "stylesheet", href: MAIN_CSS_URL }
        //document::Link { rel: "stylesheet", href: PIN_CSS_URL }
        
        // For local/Android builds, use these lines (comment out for iOS/macOS)
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        document::Link { rel: "stylesheet", href: PIN_CSS }

        document::Script { src: privacy_js_src.clone(), defer: true }
        document::Script { src: liquid_metal_js_src.clone(), defer: true }
        document::Script { src: liquid_metal_svg_js_src.clone(), defer: true }
        document::Script { src: liquid_metal_border_js_src.clone(), defer: true }
        document::Script { src: liquid_metal_circle_border_js_src.clone(), defer: true }
        document::Script { src: liquid_metal_circle_js_src.clone(), defer: true }
        
        // Show PIN unlock if PIN is set and app is locked
        if is_locked() {
            PinUnlock {
                on_unlock: move |pin: String| {
                    is_locked.set(false);
                    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "android"), not(target_os = "ios")))]
                    {
                        match bridge_handler.load_wallet_with_pin(&pin) {
                            Ok(_) => println!("✅ Bridge: Wallet loaded for browser"),
                            Err(e) => eprintln!("❌ Bridge: Failed to load wallet: {}", e),
                        }
                    }
                }
            }
        } else if show_onboarding() {
            // Show onboarding on first launch
            OnboardingFlow {
                on_complete: move |_| {
                    show_onboarding.set(false);
                }
            }
        } else {
            // Show main app
            Router::<Route> {}
        }
    }
}
