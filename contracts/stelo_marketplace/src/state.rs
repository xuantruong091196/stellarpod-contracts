use soroban_sdk::{contracttype, Address};

#[contracttype]
#[derive(Clone, Debug)]
pub struct Listing {
    pub seller: Address,
    pub nft_contract: Address,
    pub token_id: u32,
    pub price: i128, // USDC stroops
    pub created_at: u64,
}

#[contracttype]
pub enum DataKey {
    Admin,
    UsdcToken,
    PlatformFeeBps, // u32
    PlatformAddress,
    Listing(Address, u32), // (nft_contract, token_id)
}
