use dioxus::prelude::*;
use std::sync::Arc;

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
mod ans_resolver;
mod domain_resolver;
mod config;
mod token_utils;
mod squads;
mod carrot;
mod bonk_staking;
mod quantum_vault;
mod titan;
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
const MAIN_CSS_URL: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/main.css";
const PIN_CSS_URL: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/pin-premium.css";

// For local/Android builds, use the asset! macro
//const MAIN_CSS: Asset = asset!("/assets/main.css");
//const PIN_CSS: Asset = asset!("/assets/pin-premium.css");

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
    use bridge::{BridgeServer, BridgeHandler};

    let handler = Arc::new(BridgeHandler::new());
    let handler_clone = Arc::clone(&handler);

    // Start bridge server in a separate thread with its own Tokio runtime
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

        rt.block_on(async {
            let server = Arc::new(BridgeServer::new(7777));

            // Set up request handler using BridgeHandler
            let callback = Arc::new(move |request| {
                handler_clone.handle_request(request)
            });

            server.set_callback(callback);

            if let Err(e) = server.start().await {
                eprintln!("Bridge server error: {}", e);
            }
        });
    });

    handler
}

// Web & Mobile keep the generic launcher:
#[cfg(any(target_arch = "wasm32", target_os = "android", target_os = "ios"))]
fn main() {
    dioxus::launch(App);
}


#[component]
fn App() -> Element {
    // Check if onboarding has been completed
<<<<<<< Updated upstream
    //let mut show_onboarding = use_signal(|| true);
    let mut show_onboarding = use_signal(|| !storage::has_completed_onboarding());
    
    // Check if PIN is set and locked
    let mut is_locked = use_signal(|| storage::has_pin());
    
    // Initialize unified domain resolver (supports SNS .sol + ANS .abc, .bonk, etc.)
    let domain_resolver = Arc::new(domain_resolver::DomainResolver::new(
        "https://johna-k3cr1v-fast-mainnet.helius-rpc.com".to_string()
=======
    let mut show_onboarding = use_signal(|| true);
    //let mut show_onboarding = use_signal(|| !storage::has_completed_onboarding());

    // Check if PIN is set and locked
    let mut is_locked = use_signal(|| storage::has_pin());

    // Initialize SNS resolver with your RPC endpoint
    let sns_resolver = Arc::new(sns::SnsResolver::new(
        "https://johna-k3cr1v-fast-mainnet.helius-rpc.com".to_string() // Use your preferred RPC endpoint
>>>>>>> Stashed changes
    ));

    // Provide domain resolver to the entire app
    use_context_provider(|| domain_resolver);
    
    // Keep SNS resolver for backward compatibility (optional - can remove if not needed elsewhere)
    let sns_resolver = Arc::new(sns::SnsResolver::new(
        "https://johna-k3cr1v-fast-mainnet.helius-rpc.com".to_string()
    ));
    use_context_provider(|| sns_resolver);

    // Start browser bridge on desktop only
    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "android"), not(target_os = "ios")))]
    let bridge_handler = {
        let handler = use_context_provider(|| start_browser_bridge());
        handler
    };

    rsx! {
        // For iOS/macOS builds, uncomment these lines and comment out the asset! lines below
<<<<<<< Updated upstream
        document::Link { rel: "preconnect", href: "https://cdn.jsdelivr.net" }
        document::Link { rel: "stylesheet", href: MAIN_CSS_URL }
        document::Link { rel: "stylesheet", href: PIN_CSS_URL }
        
        // For local/Android builds, use these lines (comment out for iOS/macOS)
        //document::Link { rel: "stylesheet", href: MAIN_CSS }
        //document::Link { rel: "stylesheet", href: PIN_CSS }
        
=======
        //document::Link { rel: "preconnect", href: "https://cdn.jsdelivr.net" }
        //document::Link { rel: "stylesheet", href: MAIN_CSS_URL }
        //document::Link { rel: "stylesheet", href: PIN_CSS_URL }

        // For local/Android builds, use these lines (comment out for iOS/macOS)
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        document::Link { rel: "stylesheet", href: PIN_CSS }

>>>>>>> Stashed changes
        // Show PIN unlock if PIN is set and app is locked
        if is_locked() {
            PinUnlock {
                on_unlock: move |pin: String| {
                    println!("🔓 PIN unlock callback triggered");
                    is_locked.set(false);

                    // Load wallet into browser bridge on desktop
                    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "android"), not(target_os = "ios")))]
                    {
                        println!("🔑 Attempting to load wallet into bridge with PIN");
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