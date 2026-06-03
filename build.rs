use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const TITAN_ENV: &str = "UNRUGGABLE_TITAN_JWT";
const DFLOW_ENV: &str = "UNRUGGABLE_DFLOW_API_KEY";
const JUPITER_ENV: &str = "UNRUGGABLE_JUPITER_API_KEY";
const LOCAL_SECRETS_FILE: &str = "partner_secrets.local";

fn parse_local_secret_file(path: &Path) -> HashMap<String, String> {
    let Ok(contents) = fs::read_to_string(path) else {
        return HashMap::new();
    };

    contents
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return None;
            }

            let (key, value) = trimmed.split_once('=')?;
            let mut value = value.trim().to_string();

            if value.len() >= 2
                && ((value.starts_with('"') && value.ends_with('"'))
                    || (value.starts_with('\'') && value.ends_with('\'')))
            {
                value = value[1..value.len() - 1].to_string();
            }

            Some((key.trim().to_string(), value))
        })
        .collect()
}

fn resolve_secret(name: &str, local_secrets: &HashMap<String, String>) -> Result<String, String> {
    if let Ok(value) = env::var(name) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    if let Some(value) = local_secrets.get(name) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    Err(format!(
        "Missing required partner secret `{name}`. Set it in the environment or in `{LOCAL_SECRETS_FILE}`."
    ))
}

fn obfuscate(secret: &str, key: u8) -> Vec<u8> {
    secret.as_bytes().iter().map(|byte| byte ^ key).collect()
}

fn format_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| byte.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn write_generated_module(out_dir: &Path, titan: &str, dflow: &str, jupiter: &str) {
    let titan_key = 0x5A_u8;
    let dflow_key = 0x33_u8;
    let jupiter_key = 0x6D_u8;

    let titan_bytes = format_bytes(&obfuscate(titan, titan_key));
    let dflow_bytes = format_bytes(&obfuscate(dflow, dflow_key));
    let jupiter_bytes = format_bytes(&obfuscate(jupiter, jupiter_key));

    let generated = format!(
        r#"use once_cell::sync::Lazy;

fn decode_secret(bytes: &[u8], key: u8) -> String {{
    bytes.iter().map(|byte| (byte ^ key) as char).collect()
}}

static TITAN_JWT: Lazy<String> = Lazy::new(|| decode_secret(&[{titan_bytes}], {titan_key}));
static DFLOW_API_KEY: Lazy<String> = Lazy::new(|| decode_secret(&[{dflow_bytes}], {dflow_key}));
static JUPITER_API_KEY: Lazy<String> = Lazy::new(|| decode_secret(&[{jupiter_bytes}], {jupiter_key}));

pub fn titan_jwt() -> &'static str {{
    TITAN_JWT.as_str()
}}

pub fn dflow_api_key() -> &'static str {{
    DFLOW_API_KEY.as_str()
}}

pub fn jupiter_api_key() -> &'static str {{
    JUPITER_API_KEY.as_str()
}}
"#
    );

    let generated_path = out_dir.join("partner_secrets.generated.rs");
    fs::write(generated_path, generated).expect("failed to write generated partner secrets module");
}

fn main() {
    println!("cargo:rerun-if-env-changed={TITAN_ENV}");
    println!("cargo:rerun-if-env-changed={DFLOW_ENV}");
    println!("cargo:rerun-if-env-changed={JUPITER_ENV}");
    println!("cargo:rerun-if-changed={LOCAL_SECRETS_FILE}");

    let local_secrets = parse_local_secret_file(Path::new(LOCAL_SECRETS_FILE));

    let titan = resolve_secret(TITAN_ENV, &local_secrets).expect("missing Titan JWT");
    let dflow = resolve_secret(DFLOW_ENV, &local_secrets).expect("missing Dflow API key");
    let jupiter = resolve_secret(JUPITER_ENV, &local_secrets).expect("missing Jupiter API key");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR missing"));
    write_generated_module(&out_dir, &titan, &dflow, &jupiter);
}
