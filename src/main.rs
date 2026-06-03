use dioxus::prelude::*;
use std::sync::Arc;

#[cfg(all(
    not(target_arch = "wasm32"),
    not(target_os = "android"),
    not(target_os = "ios")
))]
use serde_json::Value;

#[cfg(not(test))]
#[allow(unused_macros)]
macro_rules! println {
    ($($arg:tt)*) => {{}};
}

#[cfg(not(test))]
#[allow(unused_macros)]
macro_rules! eprintln {
    ($($arg:tt)*) => {{}};
}

mod clipboard;
mod components;
mod config;
mod currency;
mod currency_utils;
mod asset_hosting;
mod hardware;
mod partner_secrets;
mod pin;
mod prices;
mod privacycash;
mod quantum_vault;
mod rpc;
mod signing;
mod sns;
mod staking;
mod storage;
mod timeout;
mod titan;
mod token_utils;
mod transaction;
mod unstaking;
mod validators;
mod wallet;

use components::*;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[route("/")]
    WalletView {},
}

const PREAUTH_SHELL_STYLE: &str = concat!(
    "position:fixed;",
    "inset:0;",
    "background:linear-gradient(180deg, #090b10 0%, #11151d 100%);",
    "color:#ffffff;",
    "overflow:auto;"
);
const AUTO_LOCK_IDLE_MS: u64 = 5 * 60 * 1000;
const ACTIVITY_UPDATE_THROTTLE_MS: u64 = 1_500;
#[cfg(all(
    not(target_arch = "wasm32"),
    not(target_os = "android"),
    not(target_os = "ios")
))]
const DESKTOP_LOCK_MONITOR_SCRIPT: &str = r#"
(() => {
    if (window.__unruggableLockMonitorInstalled) {
        try { dioxus.send({ type: "ready", now: Date.now() }); } catch (_) {}
        return;
    }

    window.__unruggableLockMonitorInstalled = true;

    const safeSend = (type) => {
        try {
            dioxus.send({ type, now: Date.now() });
        } catch (_) {}
    };

    window.addEventListener("blur", () => safeSend("blur"));
    window.addEventListener("focus", () => safeSend("focus"));
    document.addEventListener("visibilitychange", () => {
        safeSend(document.hidden ? "hidden" : "focus");
    });

    safeSend("ready");
})();
"#;

fn current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[derive(Clone, Copy)]
pub(crate) struct AppSecurityContext {
    pub is_locked: Signal<bool>,
    pub last_activity_ms: Signal<u64>,
}

impl AppSecurityContext {
    pub fn record_activity(self) {
        let now = current_timestamp_ms();
        if now.saturating_sub((self.last_activity_ms)()) >= ACTIVITY_UPDATE_THROTTLE_MS {
            let mut last_activity_ms = self.last_activity_ms;
            last_activity_ms.set(now);
        }
    }

    pub fn unlock_now(self) {
        let mut last_activity_ms = self.last_activity_ms;
        last_activity_ms.set(current_timestamp_ms());

        let mut is_locked = self.is_locked;
        is_locked.set(false);
    }

    pub fn lock_now(self, reason: &str) {
        println!("App relock triggered: {}", reason);
        crate::pin::clear_session();

        let mut last_activity_ms = self.last_activity_ms;
        last_activity_ms.set(current_timestamp_ms());

        let mut is_locked = self.is_locked;
        is_locked.set(true);
    }
}

// ── DESKTOP (macOS/Windows/Linux) ─────────────────────────────────────────────
#[cfg(all(
    not(target_arch = "wasm32"),
    not(target_os = "android"),
    not(target_os = "ios")
))]
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

    dioxus::LaunchBuilder::new()
        .with_cfg(desktop!({
            use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
            Config::new().with_window(
                WindowBuilder::new()
                    .with_title("unruggable")
                    .with_inner_size(LogicalSize::new(520.0, 960.0))
                    .with_min_inner_size(LogicalSize::new(480.0, 900.0)),
            )
        }))
        .launch(App);
}

// Web & Mobile keep the generic launcher:
#[cfg(any(target_arch = "wasm32", target_os = "android", target_os = "ios"))]
fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let main_css_href = crate::asset_hosting::app_asset("main.css");
    let pin_css_href = crate::asset_hosting::app_asset("pin-premium.css");
    let mut show_onboarding =
        use_signal(|| !storage::has_completed_onboarding() || !storage::has_pin());

    // Check if PIN is set and locked
    let is_locked = use_signal(|| storage::has_pin());
    let last_activity_ms = use_signal(current_timestamp_ms);
    let idle_monitor_generation = use_hook(|| std::rc::Rc::new(std::cell::Cell::new(0u64)));
    let app_security = AppSecurityContext {
        is_locked,
        last_activity_ms,
    };

    use_context_provider(move || app_security);

    // Initialize shared clients once so unlock re-renders stay cheap.
    let sns_resolver = use_signal(|| {
        Arc::new(sns::SnsResolver::new(
            "https://johna-k3cr1v-fast-mainnet.helius-rpc.com".to_string(), // Use your preferred RPC endpoint
        ))
    });

    // Provide SNS resolver to the entire app
    use_context_provider({
        let sns_resolver = sns_resolver();
        move || sns_resolver.clone()
    });

    // Provide a shared TransactionClient (no background TPU init to avoid iOS crash)
    let transaction_client = use_signal(|| Arc::new(transaction::TransactionClient::new(None)));
    use_context_provider({
        let transaction_client = transaction_client();
        move || transaction_client.clone()
    });

    let _wallet = use_signal(|| None as Option<wallet::WalletInfo>);

    use_effect(move || {
        spawn(async move {
            let token_started = std::time::Instant::now();
            let token_catalog = crate::config::tokens::load_verified_tokens_async().await;
            println!(
                "Prewarmed verified token catalog: {} entries in {}ms",
                token_catalog.len(),
                token_started.elapsed().as_millis()
            );

            let price_started = std::time::Instant::now();
            match prices::get_cached_prices_and_changes().await {
                Ok((prices, _)) => {
                    println!(
                        "Prewarmed price cache: {} tokens in {}ms",
                        prices.len(),
                        price_started.elapsed().as_millis()
                    );
                }
                Err(e) => {
                    println!("Failed to prewarm price cache: {}", e);
                }
            }
        });
    });

    #[cfg(all(
        not(target_arch = "wasm32"),
        not(target_os = "android"),
        not(target_os = "ios")
    ))]
    {
        let mut desktop_lock_monitor_booted = use_signal(|| false);

        use_effect(move || {
            if desktop_lock_monitor_booted() {
                return;
            }

            desktop_lock_monitor_booted.set(true);

            let mut monitor = document::eval(DESKTOP_LOCK_MONITOR_SCRIPT);
            spawn(async move {
                while let Ok(event) = monitor.recv::<Value>().await {
                    let event_type = event
                        .get("type")
                        .and_then(Value::as_str)
                        .unwrap_or_default();

                    match event_type {
                        "focus" => app_security.record_activity(),
                        "blur" | "hidden" => {
                            if storage::has_pin() && !show_onboarding() && !is_locked() {
                                app_security.lock_now("window backgrounded");
                            }
                        }
                        _ => {}
                    }
                }
            });
        });
    }

    use_effect(move || {
        let locked = is_locked();
        let onboarding_visible = show_onboarding();
        let generation = idle_monitor_generation.get().wrapping_add(1);
        idle_monitor_generation.set(generation);

        if onboarding_visible || locked || !storage::has_pin() {
            return;
        }

        let idle_monitor_generation = idle_monitor_generation.clone();
        spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(15)).await;

                if idle_monitor_generation.get() != generation {
                    break;
                }

                if show_onboarding() || is_locked() || !storage::has_pin() {
                    break;
                }

                let idle_for = current_timestamp_ms().saturating_sub(last_activity_ms());
                if idle_for >= AUTO_LOCK_IDLE_MS {
                    app_security.lock_now("idle timeout");
                    break;
                }
            }
        });
    });

    rsx! {
        document::Link { rel: "preconnect", href: "https://cdn.jsdelivr.net" }
        document::Link { rel: "stylesheet", href: main_css_href }
        document::Link { rel: "stylesheet", href: pin_css_href }

        // Show onboarding first while the flow is being tuned.
        if show_onboarding() {
            div {
                style: PREAUTH_SHELL_STYLE,
                // Show onboarding on first launch
                OnboardingFlow {
                    on_complete: move |_| {
                        show_onboarding.set(false);
                        app_security.unlock_now();
                    }
                }
            }
        } else if is_locked() {
            div {
                style: PREAUTH_SHELL_STYLE,
                PinUnlock {
                    on_unlock: move |_| {
                        println!("App unlock signal received - hiding PIN screen");
                        app_security.unlock_now();
                    }
                }
            }
        } else {
            // Show main app
            div {
                onmousedown: move |_| app_security.record_activity(),
                onclick: move |_| app_security.record_activity(),
                onkeydown: move |_| app_security.record_activity(),
                onwheel: move |_| app_security.record_activity(),
                ontouchstart: move |_| app_security.record_activity(),
                Router::<Route> {}
            }
        }
    }
}
