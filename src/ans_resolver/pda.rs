use solana_sdk::hash::hashv;
use solana_sdk::pubkey::Pubkey;

use super::constants::*;

/// Hash a name using the ANS hashing scheme (matches original tldparser implementation)
pub fn get_hashed_name(name: &str) -> Vec<u8> {
    let input = format!("{}{}", HASH_PREFIX, name);
    hashv(&[input.as_bytes()]).to_bytes().to_vec()
}

/// Find a name account from a hashed name
pub fn find_name_account_from_hashed_name(
    hashed_name: &[u8],
    name_class: Option<&Pubkey>,
    parent_name: Option<&Pubkey>,
) -> (Pubkey, u8) {
    // Match original logic: always use 3 seeds
    let name_class_bytes = name_class.cloned().unwrap_or_default().to_bytes();
    let parent_name_bytes = parent_name.cloned().unwrap_or_default().to_bytes();
    
    let seeds: &[&[u8]] = &[hashed_name, &name_class_bytes, &parent_name_bytes];
    
    Pubkey::find_program_address(seeds, &ANS_PROGRAM_ID)
}

/// Find a name account from a name string
pub fn find_name_account_from_name(
    name: &String,
    name_class: Option<&Pubkey>,
    parent_name: Option<&Pubkey>,
) -> (Pubkey, u8) {
    let hashed_name = get_hashed_name(name);
    find_name_account_from_hashed_name(&hashed_name, name_class, parent_name)
}

/// Get the parent name account for a TLD
pub fn get_name_parent_from_tld(tld: &str) -> Pubkey {
    find_name_account_from_name(&tld.to_string(), None, Some(&ORIGIN_TLD_KEY)).0
}

/// Find the TLD house account
pub fn find_tld_house(tld: &str) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"tld_house", tld.as_bytes()],
        &TLD_HOUSE_PROGRAM_ID,
    )
}

/// Find the name house account
pub fn find_name_house(tld_house: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"name_house", tld_house.as_ref()],
        &NAME_HOUSE_PROGRAM_ID,
    )
}

/// Find the NFT record account
pub fn find_nft_record(name_account: &Pubkey, name_house: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"nft_record", name_house.as_ref(), name_account.as_ref()],
        &NAME_HOUSE_PROGRAM_ID,
    )
}

/// Find the main domain account for a user
pub fn find_main_domain(owner: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"main_domain", owner.as_ref()],
        &TLD_HOUSE_PROGRAM_ID,
    )
}