// Anchor-free ANS (AllDomains) resolver for Solana
// Uses only: borsh, solana-client, solana-sdk, tokio
// Compatible with Solana 2.x

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{pubkey::Pubkey, program_pack::Pack};
use spl_token::state::Account;
use std::{
    error::Error,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

pub mod constants;
pub mod pda;
pub mod state;

use pda::*;
use state::*;

/// Resolves an ANS domain (e.g., "miester.abc") to its owner's public key
///
/// # Arguments
/// * `rpc_client` - Solana RPC client
/// * `domain_tld` - Full domain name including TLD (e.g., "miester.abc")
///
/// # Returns
/// * `Ok(Pubkey)` - The owner's public key
/// * `Err` - If domain doesn't exist, is expired, or lookup fails
///
/// # Example
/// ```no_run
/// use solana_client::nonblocking::rpc_client::RpcClient;
/// use std::sync::Arc;
///
/// async fn example() -> Result<(), Box<dyn std::error::Error>> {
///     let rpc = RpcClient::new("https://api.mainnet-beta.solana.com".to_string());
///     let owner = resolve_ans_domain(&rpc, "miester.abc").await?;
///     println!("Owner: {}", owner);
///     Ok(())
/// }
/// ```
pub async fn resolve_ans_domain(
    rpc_client: &RpcClient,
    domain_tld: &str,
) -> Result<Pubkey, Box<dyn Error>> {
    // Normalize to lowercase for case-insensitive lookups
    let normalized = domain_tld.to_lowercase();
    
    // Parse domain.tld format
    let parts: Vec<&str> = normalized.split('.').collect();
    if parts.len() != 2 {
        return Err("Invalid domain format. Expected: domain.tld".into());
    }

    let domain = parts[0];
    let tld = format!(".{}", parts[1]);

    // Get parent name account for the TLD
    let parent_name_account = get_name_parent_from_tld(&tld);

    // Find the name account for the domain
    let (name_account_key, _) =
        find_name_account_from_name(&domain.to_string(), None, Some(&parent_name_account));

    // Fetch and deserialize the name record
    let name_account_data = rpc_client.get_account_data(&name_account_key).await?;
    let name_record = NameRecordHeader::from_account_data(&name_account_data)?;

    // Check if domain is expired
    let expires_at = NameRecordHeader::get_expires_at(&name_account_data);
    if expires_at > 0 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // Grace period: 45 days
        let grace_period = 45 * 24 * 60 * 60;
        
        if now > expires_at + grace_period {
            // Domain is expired
            return Ok(Pubkey::default());
        }
    }

    let owner = name_record.owner;

    // Check if domain is wrapped as NFT
    let (tld_house_key, _) = find_tld_house(&tld);
    let (name_house_key, _) = find_name_house(&tld_house_key);
    let (nft_record_key, _) = find_nft_record(&name_account_key, &name_house_key);

    let final_owner = if owner == nft_record_key {
        // Domain is wrapped - need to find actual NFT holder
        if let Ok(nft_record_data) = rpc_client.get_account_data(&nft_record_key).await {
            if let Ok(nft_record) = NftRecord::from_account_data(&nft_record_data) {
                // Get the token account holding the NFT
                if let Ok(token_accounts) = rpc_client
                    .get_token_largest_accounts(&nft_record.nft_mint_account)
                    .await
                {
                    if let Some(largest) = token_accounts.first() {
                        if let Ok(token_account_pubkey) = Pubkey::from_str(&largest.address) {
                            if let Ok(token_account_data) = rpc_client.get_account_data(&token_account_pubkey).await {
                                if let Ok(token_account) = Account::unpack(&token_account_data) {
                                    token_account.owner
                                } else {
                                    owner
                                }
                            } else {
                                owner
                            }
                        } else {
                            owner
                        }
                    } else {
                        owner
                    }
                } else {
                    owner
                }
            } else {
                owner
            }
        } else {
            owner
        }
    } else {
        owner
    };

    Ok(final_owner)
}