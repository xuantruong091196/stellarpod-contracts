use soroban_sdk::{contracttype, Address, BytesN, Vec};

pub const MAX_ROYALTY_BPS: u32 = 2500; // 25% cap on total royalty

#[contracttype]
#[derive(Clone, Debug)]
pub struct RoyaltySplitOnChain {
    pub address: Address,
    pub percent_bps: u32,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct RoyaltyPolicy {
    pub splits: Vec<RoyaltySplitOnChain>,
    pub total_bps: u32, // <= MAX_ROYALTY_BPS; sum of splits.percent_bps
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct TokenData {
    pub owner: Address,
    pub metadata_hash: BytesN<32>,
    pub policy: RoyaltyPolicy, // empty splits + total_bps=0 means "no royalty"
}

#[contracttype]
pub enum DataKey {
    Admin,
    Marketplace,
    NextTokenId,
    Token(u32),
}
