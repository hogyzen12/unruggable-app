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
mod currency;
mod currency_utils;
mod sns;
mod config;

use components::*;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[route("/")]
    WalletView {},
}

//    const MAIN_CSS_URL: &str ="https://cdn.jsdelivr.net/gh/hogyzen12/solana-mobile@main/assets/main.css";
const MAIN_CSS: Asset = asset!("/assets/main.css");

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    // Initialize SNS resolver with your RPC endpoint
    let sns_resolver = Arc::new(sns::SnsResolver::new(
        "https://serene-stylish-mound.solana-mainnet.quiknode.pro/5489821bcd1547d9cd7b2d81f90c086e36e0e9f7/".to_string() // Use your preferred RPC endpoint
    ));
    
    // Provide SNS resolver to the entire app
    use_context_provider(|| sns_resolver);

    rsx! {
        //document::Link { rel: "preconnect", href: "https://cdn.jsdelivr.net" }
        //document::Link { rel: "stylesheet", href: MAIN_CSS_URL }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        Router::<Route> {}
    }
}