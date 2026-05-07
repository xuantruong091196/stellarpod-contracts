use soroban_sdk::{Address, Env, IntoVal, Symbol};
use crate::state::{DataKey, Listing};
use crate::errors::MarketError;

pub fn list(
    env: Env,
    seller: Address,
    nft_contract: Address,
    token_id: u32,
    price: i128,
) -> Result<(), MarketError> {
    if price <= 0 {
        return Err(MarketError::InvalidPrice);
    }
    seller.require_auth();
    let key = DataKey::Listing(nft_contract.clone(), token_id);
    if env.storage().persistent().has(&key) {
        return Err(MarketError::AlreadyListed);
    }

    // Move NFT from seller into marketplace escrow via marketplace_transfer.
    // The stelo_nft contract enforces this can only be called by the marketplace.
    let market_addr = env.current_contract_address();
    env.invoke_contract::<()>(
        &nft_contract,
        &Symbol::new(&env, "marketplace_transfer"),
        soroban_sdk::vec![
            &env,
            seller.clone().into_val(&env),
            market_addr.into_val(&env),
            token_id.into_val(&env),
        ],
    );

    let listing = Listing {
        seller,
        nft_contract,
        token_id,
        price,
        created_at: env.ledger().timestamp(),
    };
    env.storage().persistent().set(&key, &listing);
    env.storage()
        .persistent()
        .extend_ttl(&key, 2_592_000, 7_776_000);
    env.events().publish(
        (Symbol::new(&env, "listing_created"),),
        (listing.seller.clone(), listing.token_id, listing.price),
    );
    Ok(())
}

pub fn cancel(
    env: Env,
    seller: Address,
    nft_contract: Address,
    token_id: u32,
) -> Result<(), MarketError> {
    seller.require_auth();
    let key = DataKey::Listing(nft_contract.clone(), token_id);
    let listing: Listing = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(MarketError::NotListed)?;
    if listing.seller != seller {
        return Err(MarketError::NotAuthorized);
    }

    // Return NFT from marketplace escrow back to seller
    let market_addr = env.current_contract_address();
    env.invoke_contract::<()>(
        &nft_contract,
        &Symbol::new(&env, "marketplace_transfer"),
        soroban_sdk::vec![
            &env,
            market_addr.into_val(&env),
            seller.clone().into_val(&env),
            token_id.into_val(&env),
        ],
    );
    env.storage().persistent().remove(&key);
    env.events()
        .publish((Symbol::new(&env, "listing_cancelled"),), (seller, token_id));
    Ok(())
}

pub fn change_price(
    env: Env,
    seller: Address,
    nft_contract: Address,
    token_id: u32,
    new_price: i128,
) -> Result<(), MarketError> {
    seller.require_auth();
    if new_price <= 0 {
        return Err(MarketError::InvalidPrice);
    }
    let key = DataKey::Listing(nft_contract.clone(), token_id);
    let mut listing: Listing = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(MarketError::NotListed)?;
    if listing.seller != seller {
        return Err(MarketError::NotAuthorized);
    }
    let old_price = listing.price;
    listing.price = new_price;
    env.storage().persistent().set(&key, &listing);
    env.events().publish(
        (Symbol::new(&env, "price_changed"),),
        (seller, token_id, old_price, new_price),
    );
    Ok(())
}
