use soroban_sdk::{contracttype, Address, String, Symbol, Vec};

pub const MAX_BENEFICIARIES: u32 = 8;
pub const TOTAL_BPS: u32 = 10_000;

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum EscrowState {
    Locked,
    Released,
    Refunded,
    Disputed,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Beneficiary {
    pub address: Address,
    pub percent_bps: u32,
    pub role_tag: Symbol,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct EscrowDataV2 {
    pub merchant: Address,
    pub arbiter: Address,
    pub usdc_token: Address,
    pub amount: i128,
    pub platform_fee: i128,
    pub net_amount: i128,
    pub beneficiaries: Vec<Beneficiary>,
    pub state: EscrowState,
    pub order_id: String,
    pub expires_at: u64,
    pub created_at: u64,
}

#[contracttype]
pub enum DataKey {
    Escrow(String),
}
