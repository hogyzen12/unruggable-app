//! Solana v1 fallback for swap transactions that exceed the legacy/v0 packet limit.
//!
//! The rest of the desktop app remains on the established Solana 3.x types. This
//! module converts a fully resolved instruction list into the Solana 4.1 v1 wire
//! format only when the preferred v0 transaction is larger than 1,232 bytes.

use solana_sdk::{
    hash::Hash, instruction::Instruction, message::VersionedMessage, pubkey::Pubkey,
    signature::Signature, transaction::VersionedTransaction,
};
use solana_sdk_v1::{
    hash::Hash as V1Hash,
    instruction::{AccountMeta as V1AccountMeta, Instruction as V1Instruction},
    message::{v1, VersionedMessage as V1VersionedMessage},
    pubkey::Pubkey as V1Pubkey,
    signature::Signature as V1Signature,
    transaction::VersionedTransaction as V1VersionedTransaction,
};
use std::str::FromStr;

pub const V0_TRANSACTION_SIZE_LIMIT: usize = 1_232;
const DEFAULT_COMPUTE_UNITS_PER_INSTRUCTION: u32 = 200_000;
const MAX_V1_COMPUTE_UNIT_LIMIT: u32 = 1_400_000;
const DEFAULT_V1_LOADED_ACCOUNTS_BYTES: u32 = 64 * 1024 * 1024;
const COMPUTE_BUDGET_PROGRAM_ID: &str = "ComputeBudget111111111111111111111111111111";

pub fn is_v1(bytes: &[u8]) -> bool {
    bytes.first() == Some(&v1::V1_PREFIX)
}

pub fn serialize_with_v1_fallback(
    preferred_message: VersionedMessage,
    payer: &Pubkey,
    instructions: &[Instruction],
    recent_blockhash: Hash,
    allow_v1: bool,
) -> Result<Vec<u8>, String> {
    let preferred = serialize_v0_or_legacy(preferred_message)?;
    if preferred.len() <= V0_TRANSACTION_SIZE_LIMIT {
        return Ok(preferred);
    }
    if !allow_v1 {
        return Err(format!(
            "Swap route is {} bytes and requires Solana v1 support, but this signer does not advertise it",
            preferred.len()
        ));
    }

    let (retained, config) = prepare_v1_instructions(instructions)?;
    let payer = to_v1_pubkey(payer);
    let recent_blockhash = V1Hash::new_from_array(recent_blockhash.to_bytes());
    let instructions = retained.iter().map(to_v1_instruction).collect::<Vec<_>>();
    let message =
        v1::Message::try_compile_with_config(&payer, &instructions, recent_blockhash, config)
            .map_err(|error| {
                format!("Swap route is too large for v0 and cannot compile as v1: {error}")
            })?;
    serialize_v1(message)
}

fn serialize_v0_or_legacy(message: VersionedMessage) -> Result<Vec<u8>, String> {
    let transaction = VersionedTransaction {
        signatures: vec![Signature::default(); message.header().num_required_signatures as usize],
        message,
    };
    bincode::serialize(&transaction)
        .map_err(|error| format!("Failed to serialize swap transaction: {error}"))
}

fn serialize_v1(message: v1::Message) -> Result<Vec<u8>, String> {
    let message = V1VersionedMessage::V1(message);
    let transaction = V1VersionedTransaction {
        signatures: vec![V1Signature::default(); message.header().num_required_signatures as usize],
        message,
    };
    let bytes = wincode::serialize(&transaction)
        .map_err(|error| format!("Failed to serialize Solana v1 transaction: {error}"))?;
    if bytes.len() > v1::MAX_TRANSACTION_SIZE {
        return Err(format!(
            "Swap route is {} bytes and exceeds Solana v1's {}-byte limit",
            bytes.len(),
            v1::MAX_TRANSACTION_SIZE
        ));
    }
    Ok(bytes)
}

fn prepare_v1_instructions(
    instructions: &[Instruction],
) -> Result<(Vec<Instruction>, v1::TransactionConfig), String> {
    let compute_budget_program = Pubkey::from_str(COMPUTE_BUDGET_PROGRAM_ID)
        .expect("Solana compute-budget program ID is valid");
    let mut retained = Vec::with_capacity(instructions.len());
    let mut compute_unit_limit = None;
    let mut compute_unit_price = None;
    let mut loaded_accounts_limit = None;
    let mut heap_size = None;

    for instruction in instructions {
        if instruction.program_id != compute_budget_program {
            retained.push(instruction.clone());
            continue;
        }
        if !instruction.accounts.is_empty() {
            return Err("Compute-budget instruction unexpectedly contains accounts".to_string());
        }

        match instruction.data.as_slice() {
            [1, bytes @ ..] if bytes.len() == 4 => set_once(
                &mut heap_size,
                u32::from_le_bytes(bytes.try_into().unwrap()),
                "heap-size",
            )?,
            [2, bytes @ ..] if bytes.len() == 4 => set_once(
                &mut compute_unit_limit,
                u32::from_le_bytes(bytes.try_into().unwrap()),
                "compute-unit-limit",
            )?,
            [3, bytes @ ..] if bytes.len() == 8 => set_once(
                &mut compute_unit_price,
                u64::from_le_bytes(bytes.try_into().unwrap()),
                "compute-unit-price",
            )?,
            [4, bytes @ ..] if bytes.len() == 4 => set_once(
                &mut loaded_accounts_limit,
                u32::from_le_bytes(bytes.try_into().unwrap()),
                "loaded-accounts-limit",
            )?,
            _ => {
                return Err(
                    "Swap route contains an unsupported compute-budget instruction".to_string(),
                )
            }
        }
    }

    let default_compute_unit_limit = u32::try_from(retained.len())
        .unwrap_or(u32::MAX)
        .saturating_mul(DEFAULT_COMPUTE_UNITS_PER_INSTRUCTION)
        .min(MAX_V1_COMPUTE_UNIT_LIMIT);
    let compute_unit_limit = compute_unit_limit.unwrap_or(default_compute_unit_limit);
    if compute_unit_limit > MAX_V1_COMPUTE_UNIT_LIMIT {
        return Err(format!(
            "Swap route requests {compute_unit_limit} compute units, above Solana's {MAX_V1_COMPUTE_UNIT_LIMIT} limit"
        ));
    }
    let loaded_accounts_limit = loaded_accounts_limit.unwrap_or(DEFAULT_V1_LOADED_ACCOUNTS_BYTES);
    if loaded_accounts_limit > DEFAULT_V1_LOADED_ACCOUNTS_BYTES {
        return Err(format!(
            "Swap route requests {loaded_accounts_limit} loaded-account bytes, above Solana's {DEFAULT_V1_LOADED_ACCOUNTS_BYTES} limit"
        ));
    }
    if let Some(heap_size) = heap_size {
        if !(v1::MIN_HEAP_SIZE..=v1::MAX_HEAP_SIZE).contains(&heap_size) || heap_size % 1_024 != 0 {
            return Err(format!(
                "Swap route requests an invalid v1 heap size of {heap_size} bytes"
            ));
        }
    }

    let mut config = v1::TransactionConfig::empty()
        .with_compute_unit_limit(compute_unit_limit)
        .with_loaded_accounts_data_size_limit(loaded_accounts_limit);
    if let Some(heap_size) = heap_size {
        config = config.with_heap_size(heap_size);
    }
    if let Some(micro_lamports) = compute_unit_price {
        let priority_fee = (u128::from(micro_lamports)
            .saturating_mul(u128::from(compute_unit_limit))
            .saturating_add(999_999)
            / 1_000_000)
            .min(u128::from(u64::MAX)) as u64;
        config = config.with_priority_fee(priority_fee);
    }
    Ok((retained, config))
}

fn set_once<T>(target: &mut Option<T>, value: T, label: &str) -> Result<(), String> {
    if target.replace(value).is_some() {
        return Err(format!(
            "Swap route contains duplicate {label} compute-budget instructions"
        ));
    }
    Ok(())
}

fn to_v1_pubkey(pubkey: &Pubkey) -> V1Pubkey {
    V1Pubkey::new_from_array(pubkey.to_bytes())
}

fn to_v1_instruction(instruction: &Instruction) -> V1Instruction {
    V1Instruction {
        program_id: to_v1_pubkey(&instruction.program_id),
        accounts: instruction
            .accounts
            .iter()
            .map(|account| V1AccountMeta {
                pubkey: to_v1_pubkey(&account.pubkey),
                is_signer: account.is_signer,
                is_writable: account.is_writable,
            })
            .collect(),
        data: instruction.data.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::instruction::AccountMeta;
    use solana_sdk::message::v0;

    fn compute_budget_instruction(tag: u8, value: &[u8]) -> Instruction {
        let mut data = vec![tag];
        data.extend_from_slice(value);
        Instruction::new_with_bytes(
            Pubkey::from_str(COMPUTE_BUDGET_PROGRAM_ID).unwrap(),
            &data,
            vec![],
        )
    }

    fn unsigned_v0(
        payer: &Pubkey,
        instructions: &[Instruction],
        blockhash: Hash,
    ) -> VersionedMessage {
        VersionedMessage::V0(v0::Message::try_compile(payer, instructions, &[], blockhash).unwrap())
    }

    #[test]
    fn keeps_small_swap_as_v0() {
        let payer = Pubkey::new_unique();
        let blockhash = Hash::new_unique();
        let instructions = vec![Instruction::new_with_bytes(
            Pubkey::new_unique(),
            &[1, 2, 3],
            vec![],
        )];
        let bytes = serialize_with_v1_fallback(
            unsigned_v0(&payer, &instructions, blockhash),
            &payer,
            &instructions,
            blockhash,
            true,
        )
        .unwrap();
        assert!(!is_v1(&bytes));
        assert!(bytes.len() <= V0_TRANSACTION_SIZE_LIMIT);
    }

    #[test]
    fn oversized_v0_falls_back_to_bounded_v1() {
        let payer = Pubkey::new_unique();
        let blockhash = Hash::new_unique();
        let instructions = (0..36)
            .map(|_| {
                Instruction::new_with_bytes(
                    Pubkey::new_unique(),
                    &[7; 16],
                    vec![AccountMeta::new_readonly(Pubkey::new_unique(), false)],
                )
            })
            .collect::<Vec<_>>();
        let message = unsigned_v0(&payer, &instructions, blockhash);
        let preferred = serialize_v0_or_legacy(message.clone()).unwrap();
        assert!(preferred.len() > V0_TRANSACTION_SIZE_LIMIT);

        let bytes =
            serialize_with_v1_fallback(message, &payer, &instructions, blockhash, true).unwrap();
        assert!(is_v1(&bytes));
        assert!(bytes.len() <= v1::MAX_TRANSACTION_SIZE);
        let decoded: V1VersionedTransaction = wincode::deserialize(&bytes).unwrap();
        assert!(matches!(decoded.message, V1VersionedMessage::V1(_)));
    }

    #[test]
    fn oversized_route_fails_before_signing_without_v1_capability() {
        let payer = Pubkey::new_unique();
        let blockhash = Hash::new_unique();
        let instructions = (0..36)
            .map(|_| {
                Instruction::new_with_bytes(
                    Pubkey::new_unique(),
                    &[7; 16],
                    vec![AccountMeta::new_readonly(Pubkey::new_unique(), false)],
                )
            })
            .collect::<Vec<_>>();
        let error = serialize_with_v1_fallback(
            unsigned_v0(&payer, &instructions, blockhash),
            &payer,
            &instructions,
            blockhash,
            false,
        )
        .unwrap_err();
        assert!(error.contains("requires Solana v1 support"));
    }

    #[test]
    fn converts_compute_budget_instructions_into_v1_config() {
        let retained = Instruction::new_with_bytes(Pubkey::new_unique(), &[9], vec![]);
        let instructions = vec![
            compute_budget_instruction(2, &300_000_u32.to_le_bytes()),
            compute_budget_instruction(3, &5_001_u64.to_le_bytes()),
            compute_budget_instruction(4, &131_072_u32.to_le_bytes()),
            compute_budget_instruction(1, &65_536_u32.to_le_bytes()),
            retained.clone(),
        ];

        let (converted, config) = prepare_v1_instructions(&instructions).unwrap();
        assert_eq!(converted, vec![retained]);
        assert_eq!(config.compute_unit_limit, Some(300_000));
        assert_eq!(config.priority_fee, Some(1_501));
        assert_eq!(config.loaded_accounts_data_size_limit, Some(131_072));
        assert_eq!(config.heap_size, Some(65_536));
    }

    #[test]
    fn rejects_unknown_compute_budget_instruction_for_v1() {
        let error = prepare_v1_instructions(&[compute_budget_instruction(99, &[])]).unwrap_err();
        assert!(error.contains("unsupported compute-budget"));
    }

    #[test]
    fn rejects_duplicate_compute_budget_settings_for_v1() {
        let instructions = vec![
            compute_budget_instruction(2, &300_000_u32.to_le_bytes()),
            compute_budget_instruction(2, &400_000_u32.to_le_bytes()),
        ];
        let error = prepare_v1_instructions(&instructions).unwrap_err();
        assert!(error.contains("duplicate compute-unit-limit"));
    }
}
