use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    sysvar,
};
use solana_system_interface::instruction as system_instruction;
use std::str::FromStr;

pub const SLOT_GUARD_PROGRAM_ID: &str = "23MzuyVH6EKGbUHq7GjBY6ydSCVoZQYDmzeKVdDBKWNQ";
pub const HARDWARE_SLOT_WINDOW: u64 = 512;
pub const JITO_TIP_LAMPORTS: u64 = 4_200;
pub const JULES_TIP_ADDRESS: &str = "juLesoSmdTcRtzjCzYzRoHrnF8GhVu6KCV7uxq7nJGp";
pub const JULES_TIP_LAMPORTS: u64 = 100_000;
pub const JITO_TIP_ADDRESSES: [&str; 8] = [
    "HFqU5x63VTqvQss8hp11i4wVV8bD44PvwucfZ2bU7gRe",
    "ADaUMid9yfUytqMBgopwjb2DTLSokTSzL1zt6iGPaS49",
    "ADuUkR4vqLUMWXxW9gh6D6L8pMSawimctcNZ5pGwDcEt",
    "Cw8CFyM9FkoMi7K7Crf6HNQqf4uEMzpKw6QNghXLvLkY",
    "DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL",
    "96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5",
    "DfXygSm4jCyNCybVYYK6DwvWqjKee8pbDmJGcLWNDXjh",
    "3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnizKZ6jT",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TipSummary {
    pub marker_guard_max_slot: Option<u64>,
    pub jito_address: Pubkey,
    pub jito_lamports: u64,
    pub jules_address: Pubkey,
    pub jules_lamports: u64,
}

pub async fn apply_jito_and_jules_tips(
    instructions: &mut Vec<Instruction>,
    payer: &Pubkey,
    rpc_url: &str,
    use_current_v2_marker: bool,
) -> Result<TipSummary, String> {
    let marker_slot = if use_current_v2_marker {
        Some(fetch_current_slot(rpc_url).await?)
    } else {
        None
    };
    apply_jito_and_jules_tips_with_slot(instructions, payer, marker_slot)
}

fn apply_jito_and_jules_tips_with_slot(
    instructions: &mut Vec<Instruction>,
    payer: &Pubkey,
    marker_slot: Option<u64>,
) -> Result<TipSummary, String> {
    let marker_guard_max_slot = marker_slot
        .map(|slot| {
            slot.checked_add(HARDWARE_SLOT_WINDOW)
                .ok_or_else(|| "Slot overflow when calculating marker guard".to_string())
        })
        .transpose()?;
    if let Some(max_slot) = marker_guard_max_slot {
        instructions.insert(0, build_marker_guard_instruction(max_slot)?);
    }

    let jito_address = select_jito_tip_account(payer, marker_slot.unwrap_or_default())?;
    let jules_address = Pubkey::from_str(JULES_TIP_ADDRESS)
        .map_err(|error| format!("Invalid Jules tip address: {error}"))?;

    instructions.push(system_instruction::transfer(
        payer,
        &jito_address,
        JITO_TIP_LAMPORTS,
    ));
    instructions.push(system_instruction::transfer(
        payer,
        &jules_address,
        JULES_TIP_LAMPORTS,
    ));

    Ok(TipSummary {
        marker_guard_max_slot,
        jito_address,
        jito_lamports: JITO_TIP_LAMPORTS,
        jules_address,
        jules_lamports: JULES_TIP_LAMPORTS,
    })
}

pub fn select_jito_tip_account(payer: &Pubkey, entropy_slot: u64) -> Result<Pubkey, String> {
    let payer_bytes = payer.to_bytes();
    let payer_entropy = u64::from_le_bytes(
        payer_bytes[..8]
            .try_into()
            .expect("a Solana public key always contains eight bytes"),
    );
    let index = (payer_entropy ^ entropy_slot) as usize % JITO_TIP_ADDRESSES.len();
    Pubkey::from_str(JITO_TIP_ADDRESSES[index])
        .map_err(|error| format!("Invalid Jito tip address: {error}"))
}

fn build_marker_guard_instruction(max_slot: u64) -> Result<Instruction, String> {
    let program_id = Pubkey::from_str(SLOT_GUARD_PROGRAM_ID)
        .map_err(|error| format!("Invalid marker guard program ID: {error}"))?;
    Ok(Instruction {
        program_id,
        accounts: vec![AccountMeta::new_readonly(sysvar::clock::ID, false)],
        data: max_slot.to_le_bytes().to_vec(),
    })
}

async fn fetch_current_slot(rpc_url: &str) -> Result<u64, String> {
    let response = reqwest::Client::new()
        .post(rpc_url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getSlot",
            "params": [{ "commitment": "processed" }]
        }))
        .send()
        .await
        .map_err(|error| format!("Failed to fetch marker slot: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("Marker-slot RPC error: {}", response.status()));
    }
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|error| format!("Failed to parse marker slot: {error}"))?;
    if let Some(error) = json.get("error") {
        return Err(format!("Marker-slot RPC error: {error}"));
    }
    json.get("result")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| format!("Unexpected marker-slot response: {json}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_system_interface::instruction::SystemInstruction;

    #[test]
    fn matches_configured_tip_amounts_and_addresses_without_hardware_marker() {
        let payer = Pubkey::new_unique();
        let mut instructions = Vec::new();
        let summary = apply_jito_and_jules_tips_with_slot(&mut instructions, &payer, None).unwrap();

        assert_eq!(instructions.len(), 2);
        assert_eq!(summary.marker_guard_max_slot, None);
        assert!(JITO_TIP_ADDRESSES.contains(&summary.jito_address.to_string().as_str()));
        assert_eq!(summary.jito_lamports, 4_200);
        assert_eq!(summary.jules_address.to_string(), JULES_TIP_ADDRESS);
        assert_eq!(summary.jules_lamports, 100_000);
        assert_eq!(
            bincode::deserialize::<SystemInstruction>(&instructions[0].data).unwrap(),
            SystemInstruction::Transfer { lamports: 4_200 }
        );
        assert_eq!(
            bincode::deserialize::<SystemInstruction>(&instructions[1].data).unwrap(),
            SystemInstruction::Transfer { lamports: 100_000 }
        );
    }

    #[test]
    fn current_v2_marker_uses_safe_hardware_window_and_exact_order() {
        let payer = Pubkey::new_unique();
        let primary = system_instruction::transfer(&payer, &Pubkey::new_unique(), 1);
        let mut instructions = vec![primary.clone()];
        let summary =
            apply_jito_and_jules_tips_with_slot(&mut instructions, &payer, Some(1_000)).unwrap();

        assert_eq!(summary.marker_guard_max_slot, Some(1_512));
        assert_eq!(instructions.len(), 4);
        assert_eq!(
            instructions[0].program_id.to_string(),
            SLOT_GUARD_PROGRAM_ID
        );
        assert_eq!(instructions[0].accounts.len(), 1);
        assert_eq!(instructions[0].accounts[0].pubkey, sysvar::clock::ID);
        assert_eq!(
            u64::from_le_bytes(instructions[0].data.clone().try_into().unwrap()),
            1_512
        );
        assert_eq!(instructions[1], primary);
    }

    #[test]
    fn all_firmware_tip_accounts_are_valid_and_unique() {
        let mut accounts = JITO_TIP_ADDRESSES
            .iter()
            .map(|address| Pubkey::from_str(address).unwrap())
            .collect::<Vec<_>>();
        accounts.sort();
        accounts.dedup();
        assert_eq!(accounts.len(), JITO_TIP_ADDRESSES.len());
    }
}
