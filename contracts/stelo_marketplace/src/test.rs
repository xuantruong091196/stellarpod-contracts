#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, token, Address, Env};

fn setup() -> (
    Env,
    SteloMarketplaceClient<'static>,
    Address,
    Address,
    Address,
    token::StellarAssetClient<'static>,
) {
    let env = Env::default();
    env.mock_all_auths();
    let contract = env.register_contract(None, SteloMarketplace);
    let client = SteloMarketplaceClient::new(&env, &contract);

    let admin = Address::generate(&env);
    let platform = Address::generate(&env);

    // Mock USDC via Stellar asset contract
    let usdc_admin = Address::generate(&env);
    let usdc_token = env.register_stellar_asset_contract_v2(usdc_admin.clone());
    let usdc_asset = token::StellarAssetClient::new(&env, &usdc_token.address());

    // 2.5% platform fee
    client.init(&admin, &usdc_token.address(), &platform, &250u32);

    (env, client, admin, platform, usdc_token.address(), usdc_asset)
}

#[test]
fn double_init_rejected() {
    let (env, client, _, _, _, _) = setup();
    let result = client.try_init(
        &Address::generate(&env),
        &Address::generate(&env),
        &Address::generate(&env),
        &250u32,
    );
    assert_eq!(result, Err(Ok(MarketError::AlreadyInitialized)));
}

#[test]
fn cancel_unknown_listing_rejected() {
    let (env, client, _, _, _, _) = setup();
    let seller = Address::generate(&env);
    let fake_nft = Address::generate(&env);
    let result = client.try_cancel(&seller, &fake_nft, &1u32);
    assert_eq!(result, Err(Ok(MarketError::NotListed)));
}

#[test]
fn buy_unknown_listing_rejected() {
    let (env, client, _, _, _, _) = setup();
    let buyer = Address::generate(&env);
    let fake_nft = Address::generate(&env);
    let result = client.try_buy(&buyer, &fake_nft, &1u32);
    assert_eq!(result, Err(Ok(MarketError::NotListed)));
}

#[test]
fn change_price_rejects_zero() {
    let (env, client, _, _, _, _) = setup();
    let seller = Address::generate(&env);
    let fake_nft = Address::generate(&env);
    let result = client.try_change_price(&seller, &fake_nft, &1u32, &0i128);
    assert_eq!(result, Err(Ok(MarketError::InvalidPrice)));
}

#[test]
fn change_price_rejects_negative() {
    let (env, client, _, _, _, _) = setup();
    let seller = Address::generate(&env);
    let fake_nft = Address::generate(&env);
    let result = client.try_change_price(&seller, &fake_nft, &1u32, &-100i128);
    assert_eq!(result, Err(Ok(MarketError::InvalidPrice)));
}

// Integration tests with real stelo_nft cross-contract calls are deferred
// to a testnet e2e suite — they require deploying both contracts in the
// same test env and orchestrating the marketplace_transfer auth chain
// (marketplace_transfer requires the marketplace contract address to sign,
// which is a sub-invocation auth that mock_all_auths covers, but the NFT
// storage state also needs bootstrapping). The unit tests above cover
// storage/error paths; the cross-contract fee math is exercised via testnet
// deployment + manual verification before audit.
