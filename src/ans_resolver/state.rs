use borsh::{BorshDeserialize, BorshSerialize};
use solana_sdk::pubkey::Pubkey;
use std::io::Error;

/// Name Record Header - the main domain name account
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, Eq, PartialEq)]
pub struct NameRecordHeader {
    /// Parent name account (Pubkey::default() if no parent)
    pub parent_name: Pubkey,
    /// Owner of the name (may be NFT record if wrapped)
    pub owner: Pubkey,
    /// Class of data (Pubkey::default() if unspecified)
    pub nclass: Pubkey,
}

impl NameRecordHeader {
    /// Deserialize from account data (with dynamic data after header)
    pub fn from_account_data(data: &[u8]) -> Result<Self, Error> {
        if data.len() < 96 {
            return Err(Error::new(
                std::io::ErrorKind::InvalidData,
                "Account data too short",
            ));
        }
        // Deserialize just the first 96 bytes (3 pubkeys)
        NameRecordHeader::try_from_slice(&data[..96])
    }
    
    /// Get expiration time from account data
    pub fn get_expires_at(data: &[u8]) -> u64 {
        if data.len() >= 104 {
            u64::from_le_bytes([
                data[96], data[97], data[98], data[99],
                data[100], data[101], data[102], data[103],
            ])
        } else {
            0
        }
    }
}

/// NFT Record - represents a wrapped domain as an NFT
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, Eq, PartialEq)]
pub struct NftRecord {
    /// Record tag/status
    pub tag: u8,
    /// PDA bump seed
    pub bump: u8,
    /// Associated name account
    pub name_account: Pubkey,
    /// Record owner
    pub owner: Pubkey,
    /// NFT mint account
    pub nft_mint_account: Pubkey,
    /// TLD house
    pub tld_house: Pubkey,
}

impl NftRecord {
    /// Deserialize from account data (skips 8-byte discriminator)
    pub fn from_account_data(data: &[u8]) -> Result<Self, Error> {
        if data.len() < 8 {
            return Err(Error::new(
                std::io::ErrorKind::InvalidData,
                "Account data too short",
            ));
        }
        // Skip 8-byte Anchor discriminator
        NftRecord::try_from_slice(&data[8..])
    }
}

/// Main Domain - user's primary domain
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, Eq, PartialEq)]
pub struct MainDomain {
    /// Name account of the main domain
    pub name_account: Pubkey,
    /// TLD of the domain
    pub tld: String,
    /// Domain name
    pub domain: String,
}

impl MainDomain {
    /// Deserialize from account data (skips 8-byte discriminator)
    pub fn from_account_data(data: &[u8]) -> Result<Self, Error> {
        if data.len() < 8 {
            return Err(Error::new(
                std::io::ErrorKind::InvalidData,
                "Account data too short",
            ));
        }
        // Skip 8-byte Anchor discriminator
        MainDomain::try_from_slice(&data[8..])
    }
}