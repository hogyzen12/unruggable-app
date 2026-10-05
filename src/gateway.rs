const DEFAULT_GATEWAY_BASE_URL: &str = "https://gateway-production-2ae3.up.railway.app";

/// Returns the managed API base used by release builds. Developers can point a
/// local debug build at another compatible gateway.
pub fn base_url() -> String {
    std::env::var("UNRUGGABLE_GATEWAY_BASE_URL")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| {
            value.starts_with("https://")
                || cfg!(debug_assertions)
                    && (value.starts_with("http://127.0.0.1")
                        || value.starts_with("http://localhost"))
        })
        .unwrap_or_else(|| DEFAULT_GATEWAY_BASE_URL.to_string())
}

pub fn endpoint(path: &str) -> String {
    join(&base_url(), path)
}

pub fn rpc_url() -> String {
    endpoint("/v1/rpc")
}

fn join(base: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_paths_without_duplicate_slashes() {
        assert_eq!(
            join("https://gateway.example/", "/v1/rpc"),
            "https://gateway.example/v1/rpc"
        );
    }
}
