use base64::Engine;
use dioxus::document::eval;
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use solana_sdk::{
    hash::Hash,
    message::VersionedMessage,
    signature::Signature,
    transaction::VersionedTransaction,
};

use crate::signing::TransactionSigner;

const PRIVACY_CASH_API_URL: &str = "https://api3.privacycash.org";
const SIGN_MESSAGE: &str = "Privacy Money account sign in";
const PRIVACY_WASM_PATH: &str = "/assets/transaction2.wasm";
const PRIVACY_ZKEY_PATH: &str = "/assets/transaction2.zkey";

#[derive(Serialize, Deserialize, Debug, Clone)]
struct DepositRequest {
    #[serde(rename = "signedTransaction")]
    tx: String,
    #[serde(rename = "senderAddress")]
    public_key: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct PrivacyCashResponse {
    #[serde(rename = "signature")]
    signature: String,
    #[serde(rename = "success")]
    success: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WithdrawRequest {
    #[serde(rename = "serializedProof")]
    pub serializedProof: String,
    #[serde(rename = "treeAccount")]
    pub treeAccount: String,
    #[serde(rename = "nullifier0PDA")]
    pub nullifier0PDA: String,
    #[serde(rename = "nullifier1PDA")]
    pub nullifier1PDA: String,
    #[serde(rename = "nullifier2PDA")]
    pub nullifier2PDA: String,
    #[serde(rename = "nullifier3PDA")]
    pub nullifier3PDA: String,
    #[serde(rename = "treeTokenAccount")]
    pub treeTokenAccount: String,
    #[serde(rename = "globalConfigAccount")]
    pub globalConfigAccount: String,
    #[serde(rename = "recipient")]
    pub recipient: String,
    #[serde(rename = "feeRecipientAccount")]
    pub feeRecipientAccount: String,
    #[serde(rename = "extAmount")]
    pub extAmount: i64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "encryptedOutput1"
    )]
    pub encryptedOutput1: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "encryptedOutput2"
    )]
    pub encryptedOutput2: Option<String>,
    #[serde(rename = "fee")]
    pub fee: u64,
    #[serde(rename = "lookupTableAddress")]
    pub lookupTableAddress: String,
    #[serde(rename = "senderAddress")]
    pub senderAddress: String,

    // SPL optional fields
    #[serde(skip_serializing_if = "Option::is_none", rename = "treeAta")]
    pub treeAta: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "recipientAta")]
    pub recipientAta: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "mintAddress")]
    pub mintAddress: Option<String>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "feeRecipientTokenAccount"
    )]
    pub feeRecipientTokenAccount: Option<String>,
}

pub async fn sign_auth_message(
    signer: &dyn TransactionSigner,
) -> Result<String, Box<dyn std::error::Error>> {
    let signature_bytes = signer.sign_message(SIGN_MESSAGE.as_bytes()).await?;
    Ok(bs58::encode(signature_bytes).into_string())
}

pub async fn build_deposit_tx(
    authority: &str,
    signature: &str,
    lamports: u64,
    rpc_url: Option<&str>,
) -> Result<VersionedTransaction, String> {
    let wasm_url = PRIVACY_WASM_PATH;
    let zkey_url = PRIVACY_ZKEY_PATH;
    log::info!("[PrivacyCash] wasm_url={} zkey_url={}", wasm_url, zkey_url);
    log::info!(
        "[PrivacyCash] build_deposit_tx authority={} lamports={} rpc_url={:?}",
        authority,
        lamports,
        rpc_url
    );
    let mut eval = eval(
        r#"
        try {
            let [authority, signature, lamports, rpcUrl] = await dioxus.recv();
            if (rpcUrl && rpcUrl.length > 0) {
                globalThis.PRIVACY_CASH_RPC_URL = rpcUrl;
                console.log('PrivacyCash RPC set to', rpcUrl);
            }
            console.log('PrivacyCash assets', { wasmUrl: globalThis.PRIVACY_CASH_WASM_URL, zkeyUrl: globalThis.PRIVACY_CASH_ZKEY_URL });
            if (!window.PrivacyCash) {
                throw new Error('PrivacyCash SDK not loaded');
            }
            if (!globalThis.PRIVACY_CASH_WASM_URL || !globalThis.PRIVACY_CASH_ZKEY_URL) {
                throw new Error('PrivacyCash asset globals not set');
            }
            const wasmRes = await fetch(globalThis.PRIVACY_CASH_WASM_URL);
            const wasmBuf = await wasmRes.arrayBuffer();
            const wasmBytes = Array.from(new Uint8Array(wasmBuf).slice(0, 4));
            const isWasm = wasmBytes[0] === 0 && wasmBytes[1] === 97 && wasmBytes[2] === 115 && wasmBytes[3] === 109;
            if (!isWasm) {
                const ct = wasmRes.headers.get('content-type');
                throw new Error(`WASM fetch failed: status=${wasmRes.status} contentType=${ct} bytes=${wasmBytes.join(',')}`);
            }
            let client = new window.PrivacyCash({
                publicKey: authority,
                signature: signature,
            });
            console.log('PrivacyCash deposit building', { authority, lamports });
            let txb64 = await client.deposit({ lamports: lamports });
            dioxus.send(txb64);
        } catch (err) {
            console.log('PrivacyCash deposit error', err);
            dioxus.send({ error: err?.toString?.() || String(err) });
        }
        "#,
    );

    eval.send(Value::Array(vec![
        Value::String(authority.to_string()),
        Value::String(signature.to_string()),
        Value::Number(lamports.into()),
        Value::String(rpc_url.unwrap_or_default().to_string()),
    ]))
    .map_err(|_| "Failed to send deposit params".to_string())?;

    let res = eval.recv().await.map_err(|_| "Failed to receive deposit tx".to_string())?;

    match res {
        Value::String(tx_str) => {
            let tx_bytes = base64::engine::general_purpose::STANDARD
                .decode(tx_str)
                .map_err(|_| "Failed to decode deposit tx".to_string())?;
            bincode::deserialize::<VersionedTransaction>(&tx_bytes)
                .map_err(|_| "Failed to deserialize deposit tx".to_string())
        }
        Value::Object(obj) => Err(format!(
            "Deposit JS error: {}",
            obj.get("error").and_then(|v| v.as_str()).unwrap_or("unknown")
        )),
        _ => Err("Unexpected response for deposit tx".to_string()),
    }
}

pub async fn build_withdraw_request(
    authority: &str,
    signature: &str,
    lamports: u64,
    recipient: &str,
    rpc_url: Option<&str>,
) -> Result<WithdrawRequest, String> {
    let wasm_url = PRIVACY_WASM_PATH;
    let zkey_url = PRIVACY_ZKEY_PATH;
    log::info!(
        "[PrivacyCash] build_withdraw_request authority={} lamports={} recipient={} rpc_url={:?}",
        authority,
        lamports,
        recipient,
        rpc_url
    );
    let mut eval = eval(
        r#"
        try {
            let [authority, signature, lamports, recipient, rpcUrl] = await dioxus.recv();
            if (rpcUrl && rpcUrl.length > 0) {
                globalThis.PRIVACY_CASH_RPC_URL = rpcUrl;
                console.log('PrivacyCash RPC set to', rpcUrl);
            }
            console.log('PrivacyCash assets', { wasmUrl: globalThis.PRIVACY_CASH_WASM_URL, zkeyUrl: globalThis.PRIVACY_CASH_ZKEY_URL });
            if (!window.PrivacyCash) {
                throw new Error('PrivacyCash SDK not loaded');
            }
            let client = new window.PrivacyCash({
                publicKey: authority,
                signature: signature,
            });
            console.log('PrivacyCash withdraw building', { authority, lamports, recipient });
            let paramsB64 = await client.withdraw({ lamports: lamports, recipientAddress: recipient });
            dioxus.send(paramsB64);
        } catch (err) {
            console.log('PrivacyCash withdraw error', err);
            dioxus.send({ error: err?.toString?.() || String(err) });
        }
        "#,
    );

    eval.send(Value::Array(vec![
        Value::String(authority.to_string()),
        Value::String(signature.to_string()),
        Value::Number(lamports.into()),
        Value::String(recipient.to_string()),
        Value::String(rpc_url.unwrap_or_default().to_string()),
    ]))
    .map_err(|_| "Failed to send withdraw params".to_string())?;

    let res = eval
        .recv()
        .await
        .map_err(|_| "Failed to receive withdraw params".to_string())?;

    match res {
        Value::String(params_str) => {
            let params_bytes = base64::engine::general_purpose::STANDARD
                .decode(&params_str)
                .map_err(|_| "Failed to decode withdraw params".to_string())?;
            serde_json::from_slice::<WithdrawRequest>(&params_bytes)
                .map_err(|_| "Failed to deserialize withdraw params".to_string())
        }
        Value::Object(obj) => Err(format!(
            "Withdraw JS error: {}",
            obj.get("error").and_then(|v| v.as_str()).unwrap_or("unknown")
        )),
        _ => Err("Unexpected response for withdraw params".to_string()),
    }
}

pub async fn build_deposit_spl_tx(
    authority: &str,
    signature: &str,
    base_units: u64,
    mint_address: &str,
    rpc_url: Option<&str>,
) -> Result<VersionedTransaction, String> {
    log::info!(
        "[PrivacyCash] build_deposit_spl_tx authority={} base_units={} mint={} rpc_url={:?}",
        authority,
        base_units,
        mint_address,
        rpc_url
    );
    let mut eval = eval(
        r#"
        try {
            let [authority, signature, baseUnits, mintAddress, rpcUrl] = await dioxus.recv();
            if (rpcUrl && rpcUrl.length > 0) {
                globalThis.PRIVACY_CASH_RPC_URL = rpcUrl;
                console.log('PrivacyCash RPC set to', rpcUrl);
            }
            console.log('PrivacyCash SPL deposit', { authority, baseUnits, mintAddress });
            if (!window.PrivacyCash) {
                throw new Error('PrivacyCash SDK not loaded');
            }
            let client = new window.PrivacyCash({
                publicKey: authority,
                signature: signature,
            });
            let txb64 = await client.depositSPL({ base_units: baseUnits, mintAddress: mintAddress });
            dioxus.send(txb64);
        } catch (err) {
            console.log('PrivacyCash SPL deposit error', err);
            dioxus.send({ error: err?.toString?.() || String(err) });
        }
        "#,
    );

    eval.send(Value::Array(vec![
        Value::String(authority.to_string()),
        Value::String(signature.to_string()),
        Value::Number(base_units.into()),
        Value::String(mint_address.to_string()),
        Value::String(rpc_url.unwrap_or_default().to_string()),
    ]))
    .map_err(|_| "Failed to send deposit SPL params".to_string())?;

    let res = eval
        .recv()
        .await
        .map_err(|_| "Failed to receive deposit SPL tx".to_string())?;

    match res {
        Value::String(tx_str) => {
            let tx_bytes = base64::engine::general_purpose::STANDARD
                .decode(tx_str)
                .map_err(|_| "Failed to decode deposit SPL tx".to_string())?;
            bincode::deserialize::<VersionedTransaction>(&tx_bytes)
                .map_err(|_| "Failed to deserialize deposit SPL tx".to_string())
        }
        Value::Object(obj) => Err(format!(
            "Deposit SPL JS error: {}",
            obj.get("error").and_then(|v| v.as_str()).unwrap_or("unknown")
        )),
        _ => Err("Unexpected response for deposit SPL tx".to_string()),
    }
}

pub async fn build_withdraw_spl_request(
    authority: &str,
    signature: &str,
    base_units: u64,
    recipient: &str,
    mint_address: &str,
    rpc_url: Option<&str>,
) -> Result<WithdrawRequest, String> {
    log::info!(
        "[PrivacyCash] build_withdraw_spl_request authority={} base_units={} recipient={} mint={} rpc_url={:?}",
        authority,
        base_units,
        recipient,
        mint_address,
        rpc_url
    );
    let mut eval = eval(
        r#"
        try {
            let [authority, signature, baseUnits, recipient, mintAddress, rpcUrl] = await dioxus.recv();
            if (rpcUrl && rpcUrl.length > 0) {
                globalThis.PRIVACY_CASH_RPC_URL = rpcUrl;
                console.log('PrivacyCash RPC set to', rpcUrl);
            }
            console.log('PrivacyCash SPL withdraw', { authority, baseUnits, recipient, mintAddress });
            if (!window.PrivacyCash) {
                throw new Error('PrivacyCash SDK not loaded');
            }
            let client = new window.PrivacyCash({
                publicKey: authority,
                signature: signature,
            });
            let paramsB64 = await client.withdrawSPL({
                base_units: baseUnits,
                recipientAddress: recipient,
                mintAddress: mintAddress
            });
            dioxus.send(paramsB64);
        } catch (err) {
            console.log('PrivacyCash SPL withdraw error', err);
            dioxus.send({ error: err?.toString?.() || String(err) });
        }
        "#,
    );

    eval.send(Value::Array(vec![
        Value::String(authority.to_string()),
        Value::String(signature.to_string()),
        Value::Number(base_units.into()),
        Value::String(recipient.to_string()),
        Value::String(mint_address.to_string()),
        Value::String(rpc_url.unwrap_or_default().to_string()),
    ]))
    .map_err(|_| "Failed to send withdraw SPL params".to_string())?;

    let res = eval
        .recv()
        .await
        .map_err(|_| "Failed to receive withdraw SPL params".to_string())?;

    match res {
        Value::String(params_str) => {
            let params_bytes = base64::engine::general_purpose::STANDARD
                .decode(&params_str)
                .map_err(|_| "Failed to decode withdraw SPL params".to_string())?;
            match serde_json::from_slice::<WithdrawRequest>(&params_bytes) {
                Ok(params) => Ok(params),
                Err(err) => {
                    match serde_json::from_str::<WithdrawRequest>(&params_str) {
                        Ok(params) => Ok(params),
                        Err(_) => {
                            let preview = params_str.chars().take(160).collect::<String>();
                            Err(format!(
                                "Failed to deserialize withdraw SPL params: {}; preview={}",
                                err, preview
                            ))
                        }
                    }
                }
            }
        }
        Value::Object(obj) => Err(format!(
            "Withdraw SPL JS error: {}",
            obj.get("error").and_then(|v| v.as_str()).unwrap_or("unknown")
        )),
        _ => Err("Unexpected response for withdraw SPL params".to_string()),
    }
}

pub async fn get_private_balance_spl(
    authority: &str,
    signature: &str,
    mint_address: &str,
    rpc_url: Option<&str>,
) -> Result<u64, String> {
    log::info!(
        "[PrivacyCash] get_private_balance_spl authority={} mint={} rpc_url={:?}",
        authority,
        mint_address,
        rpc_url
    );
    let mut eval = eval(
        r#"
        try {
            let [authority, signature, mintAddress, rpcUrl] = await dioxus.recv();
            if (rpcUrl && rpcUrl.length > 0) {
                globalThis.PRIVACY_CASH_RPC_URL = rpcUrl;
                console.log('PrivacyCash RPC set to', rpcUrl);
            }
            if (!window.PrivacyCash) {
                throw new Error('PrivacyCash SDK not loaded');
            }
            let client = new window.PrivacyCash({
                publicKey: authority,
                signature: signature,
            });
            console.log('PrivacyCash SPL balance start', { authority, mintAddress });
            let balance = await client.getPrivateBalanceSpl(mintAddress);
            console.log('PrivacyCash SPL balance result', balance);
            const raw = (balance && (balance.base_units ?? balance.lamports ?? balance.amount)) ?? balance;
            dioxus.send(raw);
        } catch (err) {
            console.log('PrivacyCash SPL balance error', err);
            dioxus.send({ error: err?.toString?.() || String(err) });
        }
        "#,
    );

    eval.send(Value::Array(vec![
        Value::String(authority.to_string()),
        Value::String(signature.to_string()),
        Value::String(mint_address.to_string()),
        Value::String(rpc_url.unwrap_or_default().to_string()),
    ]))
    .map_err(|_| "Failed to send balance SPL params".to_string())?;

    let res = eval
        .recv()
        .await
        .map_err(|_| "Failed to receive balance SPL".to_string())?;

    match res {
        Value::Number(balance) => {
            let val = balance
                .as_u64()
                .or_else(|| balance.as_f64().map(|v| v.round() as u64))
                .ok_or_else(|| "Invalid balance response".to_string())?;
            log::info!("[PrivacyCash] SPL private balance {}", val);
            Ok(val)
        }
        Value::Object(obj) => Err(format!(
            "Balance SPL JS error: {}",
            obj.get("error").and_then(|v| v.as_str()).unwrap_or("unknown")
        )),
        _ => Err("Unexpected response for balance SPL".to_string()),
    }
}

pub async fn get_private_balance(
    authority: &str,
    signature: &str,
    rpc_url: Option<&str>,
) -> Result<u64, String> {
    let wasm_url = PRIVACY_WASM_PATH;
    let zkey_url = PRIVACY_ZKEY_PATH;
    log::info!(
        "[PrivacyCash] get_private_balance authority={} rpc_url={:?}",
        authority,
        rpc_url
    );
    let mut eval = eval(
        r#"
        try {
            let [authority, signature, rpcUrl] = await dioxus.recv();
            if (rpcUrl && rpcUrl.length > 0) {
                globalThis.PRIVACY_CASH_RPC_URL = rpcUrl;
                console.log('PrivacyCash RPC set to', rpcUrl);
            }
            console.log('PrivacyCash assets', { wasmUrl: globalThis.PRIVACY_CASH_WASM_URL, zkeyUrl: globalThis.PRIVACY_CASH_ZKEY_URL });
            if (!window.PrivacyCash) {
                throw new Error('PrivacyCash SDK not loaded');
            }
            let client = new window.PrivacyCash({
                publicKey: authority,
                signature: signature,
            });
            let balance = await client.getPrivateBalance();
            dioxus.send(balance);
        } catch (err) {
            console.log('PrivacyCash balance error', err);
            dioxus.send({ error: err?.toString?.() || String(err) });
        }
        "#,
    );

    eval.send(Value::Array(vec![
        Value::String(authority.to_string()),
        Value::String(signature.to_string()),
        Value::String(rpc_url.unwrap_or_default().to_string()),
    ]))
    .map_err(|_| "Failed to send balance params".to_string())?;

    let res = eval
        .recv()
        .await
        .map_err(|_| "Failed to receive balance".to_string())?;

    match res {
        Value::Number(balance) => balance
            .as_u64()
            .ok_or_else(|| "Invalid balance response".to_string()),
        Value::Object(obj) => Err(format!(
            "Balance JS error: {}",
            obj.get("error").and_then(|v| v.as_str()).unwrap_or("unknown")
        )),
        _ => Err("Unexpected response for balance".to_string()),
    }
}

pub async fn sign_transaction(
    signer: &dyn TransactionSigner,
    tx: &mut VersionedTransaction,
    recent_blockhash: Hash,
) -> Result<(), Box<dyn std::error::Error>> {
    match &mut tx.message {
        VersionedMessage::V0(message) => {
            message.recent_blockhash = recent_blockhash;
        }
        VersionedMessage::Legacy(message) => {
            message.recent_blockhash = recent_blockhash;
        }
    }

    let message_bytes = tx.message.serialize();
    let signature_bytes = signer.sign_message(&message_bytes).await?;

    if signature_bytes.len() != 64 {
        return Err(format!(
            "Invalid signature length: expected 64, got {}",
            signature_bytes.len()
        )
        .into());
    }

    let mut sig_array = [0u8; 64];
    sig_array.copy_from_slice(&signature_bytes);
    if tx.signatures.is_empty() {
        tx.signatures.push(Signature::from(sig_array));
    } else {
        tx.signatures[0] = Signature::from(sig_array);
    }

    Ok(())
}

pub async fn submit_deposit(authority: &str, tx: &VersionedTransaction) -> Result<String, String> {
    log::info!("PrivacyCash deposit -> {}", PRIVACY_CASH_API_URL);
    let req = DepositRequest {
        tx: base64::engine::general_purpose::STANDARD
            .encode(bincode::serialize(tx).map_err(|_| "Failed to serialize tx".to_string())?),
        public_key: authority.to_string(),
    };

    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/deposit", PRIVACY_CASH_API_URL))
        .json(&req)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = res.status();
    let body = res.text().await.map_err(|e| e.to_string())?;
    let json: Value = serde_json::from_str(&body)
        .map_err(|e| format!("decode error: {e}; body={body}"))?;

    let success = json.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
    if !success {
        let err_msg = json
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("PrivacyCash deposit failed");
        return Err(err_msg.to_string());
    }

    if !status.is_success() {
        return Err(format!("PrivacyCash deposit http {}: {}", status, body));
    }

    let signature = json
        .get("signature")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("missing signature; body={body}"))?;

    Ok(signature.to_string())
}

pub async fn submit_withdraw(req: &WithdrawRequest) -> Result<String, String> {
    let endpoint = if req.mintAddress.is_some() {
        "/withdraw/spl"
    } else {
        "/withdraw"
    };
    log::info!("PrivacyCash withdraw -> {}{}", PRIVACY_CASH_API_URL, endpoint);
    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}{}", PRIVACY_CASH_API_URL, endpoint))
        .json(req)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = res.status();
    let body = res.text().await.map_err(|e| e.to_string())?;
    let json: Value = serde_json::from_str(&body)
        .map_err(|e| format!("decode error: {e}; body={body}"))?;

    let success = json.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
    if !success {
        let err_msg = json
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("PrivacyCash withdraw failed");
        return Err(err_msg.to_string());
    }

    if !status.is_success() {
        return Err(format!("PrivacyCash withdraw http {}: {}", status, body));
    }

    let signature = json
        .get("signature")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("missing signature; body={body}"))?;

    Ok(signature.to_string())
}
