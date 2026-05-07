use soroban_sdk::{Address, BytesN, Env};
use crate::state::{DataKey, RoyaltyPolicy, TokenData};
use crate::errors::NftError;

pub fn owner_of(env: Env, token_id: u32) -> Result<Address, NftError> {
    let t: TokenData = env
        .storage()
        .persistent()
        .get(&DataKey::Token(token_id))
        .ok_or(NftError::TokenNotFound)?;
    Ok(t.owner)
}

pub fn metadata_hash(env: Env, token_id: u32) -> Result<BytesN<32>, NftError> {
    let t: TokenData = env
        .storage()
        .persistent()
        .get(&DataKey::Token(token_id))
        .ok_or(NftError::TokenNotFound)?;
    Ok(t.metadata_hash)
}

pub fn royalty_policy(env: Env, token_id: u32) -> Result<RoyaltyPolicy, NftError> {
    let t: TokenData = env
        .storage()
        .persistent()
        .get(&DataKey::Token(token_id))
        .ok_or(NftError::TokenNotFound)?;
    Ok(t.policy)
}
