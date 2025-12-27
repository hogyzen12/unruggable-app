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
mod config;
mod token_utils;
// Temporarily disabled for Solana 3.x testing (these depend on Solana 2.x SDKs)
// mod squads;
// mod carrot;
// mod bonk_staking;
mod titan;
mod pin;
mod timeout;

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

// Web & Mobile keep the generic launcher:
#[cfg(any(target_arch = "wasm32", target_os = "android", target_os = "ios"))]
fn main() {
    dioxus::launch(App);
}


#[component]
fn App() -> Element {
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
    use_context_provider(|| sns_resolver);
    
    // Initialize global TransactionClient and start background TPU initialization
    let transaction_client = Arc::new(transaction::TransactionClient::new(None));
    transaction_client.init_tpu_background();
    
    // Provide TransactionClient to the entire app
    use_context_provider(|| transaction_client);

    rsx! {
        // For iOS/macOS builds, uncomment these lines and comment out the asset! lines below
        document::Link { rel: "preconnect", href: "https://cdn.jsdelivr.net" }
        document::Link { rel: "stylesheet", href: MAIN_CSS_URL }
        document::Link { rel: "stylesheet", href: PIN_CSS_URL }
        
        // For local/Android builds, use these lines (comment out for iOS/macOS)
        //document::Link { rel: "stylesheet", href: MAIN_CSS }
        //document::Link { rel: "stylesheet", href: PIN_CSS }
        
        // Show PIN unlock if PIN is set and app is locked
        if is_locked() {
            PinUnlock {
                on_unlock: move |_| {
                    is_locked.set(false);
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