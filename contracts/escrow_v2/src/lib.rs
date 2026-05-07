#![no_std]

// escrow_v2 — N-party royalty split escrow.
//
// This contract supports up to 8 beneficiaries per escrow with basis-point
// allocation that must sum to 10000. Full implementation lands in Tasks 2-5.

use soroban_sdk::{contract, contractimpl, Env};

/// Placeholder contract struct. Real types and entry-points land in Task 2.
#[contract]
pub struct EscrowV2;

#[contractimpl]
impl EscrowV2 {
    /// Stub — will be replaced in Task 2.
    pub fn version(_env: Env) -> u32 {
        2
    }
}
