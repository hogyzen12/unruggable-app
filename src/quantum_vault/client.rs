use crate::quantum_vault::types::{VaultInfo, SplitResult};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    compute_budget::ComputeBudgetInstruction,
    instruction::{AccountMeta, Instruction},
    native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use solana_winternitz::privkey::WinternitzPrivkey;
use std::str::FromStr;

// Deployed Blueshift quantum vault program on mainnet
const QUANTUM_VAULT_PROGRAM_ID: &str = "5gyqnhRbYmy2KQaLLVS5F8NJ81EwG2KsJdCcV7w11BUZ";

/// Client for interacting with quantum vaults
pub struct QuantumVaultClient {
    rpc_client: RpcClient,
    program_id: Pubkey,
}

impl QuantumVaultClient {
    /// Create a new quantum vault client
    pub fn new(rpc_url: Option<&str>) -> Result<Self, String> {
        let url = rpc_url.unwrap_or("https://api.mainnet-beta.solana.com");
        Ok(Self {
            rpc_client: RpcClient::new_with_commitment(
                url.to_string(),
                CommitmentConfig::confirmed(),
            ),
            program_id: Pubkey::from_str(QUANTUM_VAULT_PROGRAM_ID)
                .map_err(|e| format!("Invalid program ID: {}", e))?,
        })
    }

    /// Generate a new Winternitz keypair and derive vault address
    pub fn generate_new_vault(&self) -> (WinternitzPrivkey, Pubkey, u8, [u8; 32]) {
        let privkey = WinternitzPrivkey::generate();
        let pubkey_hash = privkey.pubkey().merklize();
        let (vault_address, bump) = Pubkey::find_program_address(&[&pubkey_hash], &self.program_id);
        (privkey, vault_address, bump, pubkey_hash)
    }

    /// Derive vault address from public key hash
    pub fn derive_vault_address(&self, pubkey_hash: &[u8; 32]) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[pubkey_hash], &self.program_id)
    }

    /// Get vault information
    pub fn get_vault_info(&self, vault_address: &Pubkey, pubkey_hash: [u8; 32], bump: u8) -> Result<VaultInfo, String> {
        match self.rpc_client.get_account(vault_address) {
            Ok(account) => Ok(VaultInfo {
                address: *vault_address,
                balance: account.lamports,
                exists: true,
                is_quantum_vault: account.owner == self.program_id,
                owner: account.owner,
                bump,
                pubkey_hash,
            }),
            Err(_) => Ok(VaultInfo {
                address: *vault_address,
                balance: 0,
                exists: false,
                is_quantum_vault: false,
                owner: Pubkey::default(),
                bump,
                pubkey_hash,
            }),
        }
    }

    /// Get vault balance in lamports
    pub fn get_vault_balance(&self, vault_address: &Pubkey) -> Result<u64, String> {
        self.rpc_client
            .get_balance(vault_address)
            .map_err(|e| format!("Failed to get vault balance: {}", e))
    }

    /// Get vault balance in SOL
    pub fn get_vault_balance_sol(&self, vault_address: &Pubkey) -> Result<f64, String> {
        let lamports = self.get_vault_balance(vault_address)?;
        Ok(lamports as f64 / LAMPORTS_PER_SOL as f64)
    }

    /// Create a new quantum vault on-chain
    pub async fn create_vault(
        &self,
        payer: &Keypair,
        pubkey_hash: &[u8; 32],
        bump: u8,
    ) -> Result<String, String> {
        let (vault_pda, _) = self.derive_vault_address(pubkey_hash);

        let instruction_data = [
            &[0u8].as_ref(), // OpenVault discriminator
            pubkey_hash.as_ref(),
            &[bump].as_ref(),
        ]
        .concat();

        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(vault_pda, false),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            ],
            data: instruction_data,
        };

        let recent_blockhash = self
            .rpc_client
            .get_latest_blockhash()
            .map_err(|e| format!("Failed to get blockhash: {}", e))?;

        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&payer.pubkey()),
            &[payer],
            recent_blockhash,
        );

        let signature = self
            .rpc_client
            .send_and_confirm_transaction(&transaction)
            .map_err(|e| format!("Failed to create vault: {}", e))?;

        Ok(signature.to_string())
    }

    /// Deposit SOL to a vault
    pub async fn deposit_to_vault(
        &self,
        payer: &Keypair,
        vault_address: &Pubkey,
        amount: u64,
    ) -> Result<String, String> {
        let instruction = system_instruction::transfer(&payer.pubkey(), vault_address, amount);

        let recent_blockhash = self
            .rpc_client
            .get_latest_blockhash()
            .map_err(|e| format!("Failed to get blockhash: {}", e))?;

        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&payer.pubkey()),
            &[payer],
            recent_blockhash,
        );

        let signature = self
            .rpc_client
            .send_and_confirm_transaction(&transaction)
            .map_err(|e| format!("Failed to deposit: {}", e))?;

        Ok(signature.to_string())
    }

    /// Split a vault using quantum-resistant Winternitz signature
    pub async fn split_vault(
        &self,
        payer: &Keypair,
        vault_privkey: &WinternitzPrivkey,
        vault_address: &Pubkey,
        split_vault_address: &Pubkey,
        refund_vault_address: &Pubkey,
        split_amount: u64,
        bump: u8,
    ) -> Result<SplitResult, String> {
        // Create Winternitz signature
        let mut message = [0u8; 72];
        message[0..8].clone_from_slice(&split_amount.to_le_bytes());
        message[8..40].clone_from_slice(&split_vault_address.to_bytes());
        message[40..].clone_from_slice(&refund_vault_address.to_bytes());

        let signature = vault_privkey.sign(&message.as_ref());
        let sig_bytes: [u8; 896] = signature.into();

        let compute_budget = ComputeBudgetInstruction::set_compute_unit_limit(1_000_000);

        let instruction_data = [
            &[1u8].as_ref(), // SplitVault discriminator
            sig_bytes.as_ref(),
            split_amount.to_le_bytes().as_ref(),
            &[bump].as_ref(),
        ]
        .concat();

        let split_instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(*vault_address, false),
                AccountMeta::new(*split_vault_address, false),
                AccountMeta::new(*refund_vault_address, false),
            ],
            data: instruction_data,
        };

        let recent_blockhash = self
            .rpc_client
            .get_latest_blockhash()
            .map_err(|e| format!("Failed to get blockhash: {}", e))?;

        let transaction = Transaction::new_signed_with_payer(
            &[compute_budget, split_instruction],
            Some(&payer.pubkey()),
            &[payer],
            recent_blockhash,
        );

        let tx_signature = self
            .rpc_client
            .send_and_confirm_transaction(&transaction)
            .map_err(|e| format!("Failed to split vault: {}", e))?;

        // Get final balances
        let split_balance = self.get_vault_balance(split_vault_address)?;
        let refund_balance = self.get_vault_balance(refund_vault_address)?;

        Ok(SplitResult {
            transaction_signature: tx_signature.to_string(),
            split_vault: *split_vault_address,
            refund_vault: *refund_vault_address,
            split_amount: split_balance,
            refund_amount: refund_balance,
        })
    }
}