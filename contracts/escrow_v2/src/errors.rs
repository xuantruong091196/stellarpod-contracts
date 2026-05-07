use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum EscrowErrorV2 {
    AlreadyExists = 1,
    NotFound = 2,
    InvalidAmount = 3,
    InvalidFee = 4,
    AlreadyExpired = 5,
    NotLocked = 6,
    NotDisputed = 7,
    NotAuthorized = 8,
    NotExpired = 9,
    InvalidPercent = 10,
    Overflow = 11,
    BpsSumMismatch = 12,
    TooManyBeneficiaries = 13,
    EmptyBeneficiaries = 14,
}
