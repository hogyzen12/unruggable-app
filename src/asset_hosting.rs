const DESKTOP_ASSET_BASE: &str = "/app-assets";

#[cfg(not(any(target_os = "android", target_os = "ios", target_arch = "wasm32")))]
pub fn app_asset(path: &str) -> String {
    format!("{DESKTOP_ASSET_BASE}/{path}")
}

#[cfg(not(any(target_os = "android", target_os = "ios", target_arch = "wasm32")))]
pub fn use_app_asset_handler() {
    use dioxus::desktop::{use_asset_handler, wry::http::Response};

    use_asset_handler("app-assets", |request, responder| {
        let path = request
            .uri()
            .path()
            .strip_prefix("/app-assets/")
            .unwrap_or_default();
        let asset = match path {
            "main.css" => Some((
                include_bytes!("../assets/main.css").as_slice(),
                "text/css; charset=utf-8",
            )),
            "pin-premium.css" => Some((
                include_bytes!("../assets/pin-premium.css").as_slice(),
                "text/css; charset=utf-8",
            )),
            "lock.png" => Some((include_bytes!("../assets/lock.png").as_slice(), "image/png")),
            "key_screen_1.png" => Some((
                include_bytes!("../assets/key_screen_1.png").as_slice(),
                "image/png",
            )),
            "lendLogos/usdc.png" => Some((
                include_bytes!("../assets/lendLogos/usdc.png").as_slice(),
                "image/png",
            )),
            "lendLogos/sol.png" => Some((
                include_bytes!("../assets/lendLogos/sol.png").as_slice(),
                "image/png",
            )),
            "lendLogos/usdt.png" => Some((
                include_bytes!("../assets/lendLogos/usdt.png").as_slice(),
                "image/png",
            )),
            "lendLogos/eurc.png" => Some((
                include_bytes!("../assets/lendLogos/eurc.png").as_slice(),
                "image/png",
            )),
            "lendLogos/usdg.png" => Some((
                include_bytes!("../assets/lendLogos/usdg.png").as_slice(),
                "image/png",
            )),
            "lendLogos/usds.png" => Some((
                include_bytes!("../assets/lendLogos/usds.png").as_slice(),
                "image/png",
            )),
            "icons/icon.png" => Some((
                include_bytes!("../assets/icons/icon.png").as_slice(),
                "image/png",
            )),
            _ => None,
        };

        let response = match asset {
            Some((bytes, content_type)) => Response::builder()
                .header("Content-Type", content_type)
                .header("Cache-Control", "no-store")
                .body(bytes.to_vec()),
            None => Response::builder().status(404).body(Vec::new()),
        };

        if let Ok(response) = response {
            responder.respond(response);
        }
    });
}

#[cfg(any(target_os = "android", target_os = "ios", target_arch = "wasm32"))]
pub fn use_app_asset_handler() {}

#[cfg(any(target_os = "android", target_os = "ios", target_arch = "wasm32"))]
pub fn app_asset(path: &str) -> String {
    use dioxus::prelude::{asset, Asset};

    let asset: Asset = match path {
        "main.css" => asset!("/assets/main.css"),
        "pin-premium.css" => asset!("/assets/pin-premium.css"),
        "lock.png" => asset!("/assets/lock.png"),
        "key_screen_1.png" => asset!("/assets/key_screen_1.png"),
        "lendLogos/usdc.png" => asset!("/assets/lendLogos/usdc.png"),
        "lendLogos/sol.png" => asset!("/assets/lendLogos/sol.png"),
        "lendLogos/usdt.png" => asset!("/assets/lendLogos/usdt.png"),
        "lendLogos/eurc.png" => asset!("/assets/lendLogos/eurc.png"),
        "lendLogos/usdg.png" => asset!("/assets/lendLogos/usdg.png"),
        "lendLogos/usds.png" => asset!("/assets/lendLogos/usds.png"),
        "icons/icon.png" => asset!("/assets/icons/icon.png"),
        other => panic!("unregistered app asset: {other}"),
    };
    asset.to_string()
}
