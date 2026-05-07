use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum MarketError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    NotAuthorized = 3,
    AlreadyListed = 4,
    NotListed = 5,
    InvalidPrice = 6,
    InvalidSplit = 7,
    Overflow = 8,
    InsufficientPayment = 9,
}
