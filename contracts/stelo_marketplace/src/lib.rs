#![no_std]

mod buy;
mod errors;
mod list;
mod state;

use soroban_sdk::{contract, contractimpl, Address, Env};
pub use errors::MarketError;
pub use state::{DataKey, Listing};

#[contract]
pub struct SteloMarketplace;

#[contractimpl]
impl SteloMarketplace {
    /// One-time initialization. Admin must auth.
    pub fn init(
        env: Env,
        admin: Address,
        usdc_token: Address,
        platform_address: Address,
        platform_fee_bps: u32,
    ) -> Result<(), MarketError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(MarketError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::UsdcToken, &usdc_token);
        env.storage()
            .instance()
            .set(&DataKey::PlatformAddress, &platform_address);
        env.storage()
            .instance()
            .set(&DataKey::PlatformFeeBps, &platform_fee_bps);
        Ok(())
    }

    /// List an NFT for sale. Moves token into marketplace escrow.
    pub fn list(
        env: Env,
        seller: Address,
        nft_contract: Address,
        token_id: u32,
        price: i128,
    ) -> Result<(), MarketError> {
        list::list(env, seller, nft_contract, token_id, price)
    }

    /// Cancel a listing. Returns NFT from escrow to seller.
    pub fn cancel(
        env: Env,
        seller: Address,
        nft_contract: Address,
        token_id: u32,
    ) -> Result<(), MarketError> {
        list::cancel(env, seller, nft_contract, token_id)
    }

    /// Update listing price. Only the original seller can call this.
    pub fn change_price(
        env: Env,
        seller: Address,
        nft_contract: Address,
        token_id: u32,
        new_price: i128,
    ) -> Result<(), MarketError> {
        list::change_price(env, seller, nft_contract, token_id, new_price)
    }

    /// Buy a listed NFT. Atomically: pays platform fee, royalties, seller;
    /// transfers NFT from escrow to buyer — all in one transaction.
    pub fn buy(
        env: Env,
        buyer: Address,
        nft_contract: Address,
        token_id: u32,
    ) -> Result<(), MarketError> {
        buy::buy(env, buyer, nft_contract, token_id)
    }
}

#[cfg(test)]
mod test;
