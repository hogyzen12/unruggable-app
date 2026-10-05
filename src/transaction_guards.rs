use solana_sdk::{instruction::Instruction, pubkey::Pubkey};
use solana_system_interface::instruction as system_instruction;
use std::str::FromStr;

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
    pub jito_address: Pubkey,
    pub jito_lamports: u64,
    pub jules_address: Pubkey,
    pub jules_lamports: u64,
}

pub fn append_jito_and_jules_tips(
    instructions: &mut Vec<Instruction>,
    payer: &Pubkey,
) -> Result<TipSummary, String> {
    let jito_address = select_jito_tip_account(payer)?;
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
        jito_address,
        jito_lamports: JITO_TIP_LAMPORTS,
        jules_address,
        jules_lamports: JULES_TIP_LAMPORTS,
    })
}

pub fn select_jito_tip_account(payer: &Pubkey) -> Result<Pubkey, String> {
    let payer_bytes = payer.to_bytes();
    let payer_entropy = u64::from_le_bytes(
        payer_bytes[..8]
            .try_into()
            .expect("a Solana public key always contains eight bytes"),
    );
    // Keep selection deterministic without adding an RPC slot dependency or an
    // on-chain slot-guard instruction to the transaction.
    let index = payer_entropy as usize % JITO_TIP_ADDRESSES.len();
    Pubkey::from_str(JITO_TIP_ADDRESSES[index])
        .map_err(|error| format!("Invalid Jito tip address: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_system_interface::instruction::SystemInstruction;

    #[test]
    fn matches_configured_tip_amounts_and_addresses() {
        let payer = Pubkey::new_unique();
        let mut instructions = Vec::new();
        let summary = append_jito_and_jules_tips(&mut instructions, &payer).unwrap();

        assert_eq!(instructions.len(), 2);
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
