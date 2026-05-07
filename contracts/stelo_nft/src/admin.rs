use soroban_sdk::{Address, BytesN, Env};
use crate::state::{DataKey, MAX_ROYALTY_BPS, RoyaltyPolicy, TokenData};
use crate::errors::NftError;

pub fn init(env: Env, admin: Address, marketplace: Address) -> Result<(), NftError> {
    if env.storage().instance().has(&DataKey::Admin) {
        return Err(NftError::AlreadyInitialized);
    }
    admin.require_auth();
    env.storage().instance().set(&DataKey::Admin, &admin);
    env.storage().instance().set(&DataKey::Marketplace, &marketplace);
    env.storage().instance().set(&DataKey::NextTokenId, &0u32);
    Ok(())
}

fn require_admin(env: &Env) -> Result<Address, NftError> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(NftError::NotInitialized)?;
    admin.require_auth();
    Ok(admin)
}

fn validate_policy(p: &RoyaltyPolicy) -> Result<(), NftError> {
    if p.total_bps > MAX_ROYALTY_BPS {
        return Err(NftError::InvalidRoyalty);
    }
    let mut sum: u32 = 0;
    for s in p.splits.iter() {
        sum = sum.checked_add(s.percent_bps).ok_or(NftError::InvalidRoyalty)?;
    }
    if sum != p.total_bps {
        return Err(NftError::InvalidSplit);
    }
    Ok(())
}

pub fn mint(
    env: Env,
    to: Address,
    metadata_hash: BytesN<32>,
    policy: RoyaltyPolicy,
) -> Result<u32, NftError> {
    require_admin(&env)?;
    validate_policy(&policy)?;

    let mut next: u32 = env
        .storage()
        .instance()
        .get(&DataKey::NextTokenId)
        .unwrap_or(0);
    next = next.checked_add(1).ok_or(NftError::InvalidSplit)?; // overflow guard

    let token = TokenData {
        owner: to,
        metadata_hash,
        policy,
    };
    env.storage().persistent().set(&DataKey::Token(next), &token);
    env.storage().instance().set(&DataKey::NextTokenId, &next);
    Ok(next)
}

pub fn set_royalty_policy(
    env: Env,
    token_id: u32,
    policy: RoyaltyPolicy,
) -> Result<(), NftError> {
    require_admin(&env)?;
    validate_policy(&policy)?;
    let mut token: TokenData = env
        .storage()
        .persistent()
        .get(&DataKey::Token(token_id))
        .ok_or(NftError::TokenNotFound)?;
    token.policy = policy;
    env.storage()
        .persistent()
        .set(&DataKey::Token(token_id), &token);
    Ok(())
}

pub fn set_marketplace(env: Env, marketplace: Address) -> Result<(), NftError> {
    require_admin(&env)?;
    env.storage()
        .instance()
        .set(&DataKey::Marketplace, &marketplace);
    Ok(())
}
