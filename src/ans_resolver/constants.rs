use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;

/// ANS Program ID on Solana mainnet
pub const ANS_PROGRAM_ID: Pubkey = pubkey!("ALTNSZ46uaAUU7XUV6awvdorLGqAsPwa9shm7h4uP2FK");

/// TLD House Program ID
pub const TLD_HOUSE_PROGRAM_ID: Pubkey = pubkey!("TLDHkysf5pCnKsVA4gXpNvmy7psXLPEu4LAdDJthT9S");

/// Name House Program ID  
pub const NAME_HOUSE_PROGRAM_ID: Pubkey = pubkey!("NH3uX6FtVE2fNREAioP7hm5RaozotZxeL6khU1EHx51");

/// Origin TLD key - root of the TLD hierarchy  
pub const ORIGIN_TLD_KEY: Pubkey = pubkey!("3mX9b4AZaQehNoQGfckVcmgmA6bkBoFcbLj9RMmMyNcU");

/// Hash prefix for name derivation
pub const HASH_PREFIX: &str = "ALT Name Service";