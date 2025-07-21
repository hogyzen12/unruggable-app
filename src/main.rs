use dioxus::prelude::*;

mod wallet;
mod rpc;
mod prices;
mod transaction;
mod signing;
mod hardware;
mod storage;
mod components;

use components::*;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[route("/")]
    WalletView {},
}

const MAIN_CSS: Asset = asset!("/assets/main.css");

fn main() {
    // Initialize logging for Android
    #[cfg(target_os = "android")]
    {
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Info)
                .with_tag("unruggable")
        );
        log::info!("🤖 Android app starting...");
    }

    // Platform-specific launch
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        log::info!("📱 Launching mobile app...");
        // For mobile platforms, launch as a native app (no server)
        dioxus::LaunchBuilder::new()
            .launch(App);
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        // For desktop/web, use fullstack features
        dioxus::launch(App);
    }
}

#[component]
fn App() -> Element {
    #[cfg(target_os = "android")]
    log::info!("🚀 App component rendering...");
    
    rsx! {
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        Router::<Route> {}
    }
}