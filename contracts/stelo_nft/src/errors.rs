use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum NftError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    NotAuthorized = 3,
    TokenExists = 4,
    TokenNotFound = 5,
    InvalidRoyalty = 6, // > 25% or invalid bps sum
    NotMarketplace = 7,
    InvalidSplit = 8,
}
