use soroban_sdk::{Address, Env};
use crate::state::{DataKey, TokenData};
use crate::errors::NftError;

/// Peer-to-peer transfer — no royalty enforcement. Owner must auth.
pub fn transfer(
    env: Env,
    from: Address,
    to: Address,
    token_id: u32,
) -> Result<(), NftError> {
    let mut token: TokenData = env
        .storage()
        .persistent()
        .get(&DataKey::Token(token_id))
        .ok_or(NftError::TokenNotFound)?;
    if token.owner != from {
        return Err(NftError::NotAuthorized);
    }
    from.require_auth();
    token.owner = to;
    env.storage()
        .persistent()
        .set(&DataKey::Token(token_id), &token);
    Ok(())
}

/// Marketplace-only transfer. Caller must be the configured marketplace contract.
/// This is how the marketplace moves NFTs into escrow on `list` and out on `buy`.
pub fn marketplace_transfer(
    env: Env,
    from: Address,
    to: Address,
    token_id: u32,
) -> Result<(), NftError> {
    let marketplace: Address = env
        .storage()
        .instance()
        .get(&DataKey::Marketplace)
        .ok_or(NftError::NotInitialized)?;
    marketplace.require_auth();

    let mut token: TokenData = env
        .storage()
        .persistent()
        .get(&DataKey::Token(token_id))
        .ok_or(NftError::TokenNotFound)?;
    if token.owner != from {
        return Err(NftError::NotAuthorized);
    }
    token.owner = to;
    env.storage()
        .persistent()
        .set(&DataKey::Token(token_id), &token);
    Ok(())
}
