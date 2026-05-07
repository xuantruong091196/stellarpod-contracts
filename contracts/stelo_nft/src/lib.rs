#![no_std]

mod admin;
mod errors;
mod state;
mod transfer;
mod views;

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env};
pub use errors::NftError;
pub use state::{DataKey, MAX_ROYALTY_BPS, RoyaltyPolicy, RoyaltySplitOnChain, TokenData};

#[contract]
pub struct SteloNft;

#[contractimpl]
impl SteloNft {
    pub fn init(env: Env, admin: Address, marketplace: Address) -> Result<(), NftError> {
        admin::init(env, admin, marketplace)
    }

    pub fn mint(
        env: Env,
        to: Address,
        metadata_hash: BytesN<32>,
        policy: RoyaltyPolicy,
    ) -> Result<u32, NftError> {
        admin::mint(env, to, metadata_hash, policy)
    }

    pub fn set_royalty_policy(
        env: Env,
        token_id: u32,
        policy: RoyaltyPolicy,
    ) -> Result<(), NftError> {
        admin::set_royalty_policy(env, token_id, policy)
    }

    pub fn set_marketplace(env: Env, marketplace: Address) -> Result<(), NftError> {
        admin::set_marketplace(env, marketplace)
    }

    pub fn transfer(
        env: Env,
        from: Address,
        to: Address,
        token_id: u32,
    ) -> Result<(), NftError> {
        transfer::transfer(env, from, to, token_id)
    }

    pub fn marketplace_transfer(
        env: Env,
        from: Address,
        to: Address,
        token_id: u32,
    ) -> Result<(), NftError> {
        transfer::marketplace_transfer(env, from, to, token_id)
    }

    pub fn owner_of(env: Env, token_id: u32) -> Result<Address, NftError> {
        views::owner_of(env, token_id)
    }

    pub fn metadata_hash(env: Env, token_id: u32) -> Result<BytesN<32>, NftError> {
        views::metadata_hash(env, token_id)
    }

    pub fn royalty_policy(env: Env, token_id: u32) -> Result<RoyaltyPolicy, NftError> {
        views::royalty_policy(env, token_id)
    }
}

#[cfg(test)]
mod test;
