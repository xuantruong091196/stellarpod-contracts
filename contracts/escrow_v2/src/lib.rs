#![no_std]

mod errors;
mod state;
mod lock;
mod release;
mod refund;
mod dispute;

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

    pub fn release(env: Env, caller: Address, order_id: String) -> Result<(), EscrowErrorV2> {
        release::release(env, caller, order_id)
    }

    pub fn refund(env: Env, order_id: String) -> Result<(), EscrowErrorV2> {
        refund::refund(env, order_id)
    }

    pub fn dispute(env: Env, caller: Address, order_id: String) -> Result<(), EscrowErrorV2> {
        dispute::dispute(env, caller, order_id)
    }

    pub fn resolve_dispute(
        env: Env,
        arbiter: Address,
        order_id: String,
        provider_bps: u32,
    ) -> Result<(), EscrowErrorV2> {
        dispute::resolve_dispute(env, arbiter, order_id, provider_bps)
    }
}

#[cfg(test)]
mod test;
