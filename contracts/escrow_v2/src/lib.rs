#![no_std]

mod errors;
mod state;
mod lock;

use soroban_sdk::{contract, contractimpl, Address, Env, String, Vec};
pub use errors::EscrowErrorV2;
pub use state::{Beneficiary, EscrowDataV2, EscrowState, MAX_BENEFICIARIES, TOTAL_BPS};

#[contract]
pub struct EscrowV2;

#[contractimpl]
impl EscrowV2 {
    pub fn lock(
        env: Env,
        merchant: Address,
        arbiter: Address,
        usdc_token: Address,
        amount: i128,
        platform_fee: i128,
        beneficiaries: Vec<Beneficiary>,
        order_id: String,
        expires_at: u64,
    ) -> Result<(), EscrowErrorV2> {
        lock::lock(
            env, merchant, arbiter, usdc_token, amount, platform_fee,
            beneficiaries, order_id, expires_at,
        )
    }
}
