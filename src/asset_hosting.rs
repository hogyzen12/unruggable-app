#[cfg(any(target_os = "macos", target_os = "ios", target_os = "windows"))]
const HOSTED_ASSET_BASE: &str =
    "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@dev-app-release/assets";

#[cfg(any(target_os = "macos", target_os = "ios", target_os = "windows"))]
pub fn app_asset(path: &str) -> String {
    format!("{HOSTED_ASSET_BASE}/{path}")
}

#[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "windows")))]
pub fn app_asset(path: &str) -> String {
    format!("/assets/{path}")
}
