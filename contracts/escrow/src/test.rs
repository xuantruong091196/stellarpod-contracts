#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    token, Address, Env, String,
};

fn setup_test() -> (
    Env,
    Address,               // contract
    Address,               // merchant
    Address,               // provider
    Address,               // arbiter
    Address,               // usdc token
    token::StellarAssetClient, // usdc admin client
) {
    let env = Env::default();
    env.mock_all_auths();

    // Deploy escrow contract
    let contract = env.register_contract(None, StellarPodEscrow);

    // Create test addresses
    let merchant = Address::generate(&env);
    let provider = Address::generate(&env);
    let arbiter = Address::generate(&env);

    // Deploy USDC token
    let usdc_admin = Address::generate(&env);
    let usdc_token = env.register_stellar_asset_contract_v2(usdc_admin.clone());
    let usdc_asset = token::StellarAssetClient::new(&env, &usdc_token.address());

    // Mint USDC to merchant (10,000 USDC = 10_000_0000000 stroops)
    usdc_asset.mint(&merchant, &10_000_0000000);

    // Set ledger timestamp
    env.ledger().set(LedgerInfo {
        timestamp: 1_000_000,
        protocol_version: 21,
        sequence_number: 100,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 100,
        max_entry_ttl: 10_000_000,
    });

    (env, contract, merchant, provider, arbiter, usdc_token.address(), usdc_asset)
}

#[test]
fn test_lock_escrow() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _usdc_asset) = setup_test();

    let client = StellarPodEscrowClient::new(&env, &contract);

    let amount: i128 = 100_0000000; // 100 USDC
    let platform_fee: i128 = 5_0000000; // 5 USDC (5%)
    let order_id = String::from_str(&env, "order_001");
    let expires_at: u64 = 2_000_000; // In the future

    let result = client.lock(
        &merchant,
        &provider,
        &arbiter,
        &usdc_token,
        &amount,
        &platform_fee,
        &order_id,
        &expires_at,
    );

    assert_eq!(result, Ok(()));

    // Verify escrow state
    let escrow = client.get_state().unwrap();
    assert_eq!(escrow.state, EscrowState::Locked);
    assert_eq!(escrow.amount, amount);
    assert_eq!(escrow.platform_fee, platform_fee);
    assert_eq!(escrow.provider_amount, amount - platform_fee);

    // Verify USDC transferred to contract
    let usdc_client = token::Client::new(&env, &usdc_token);
    let contract_balance = usdc_client.balance(&contract);
    assert_eq!(contract_balance, amount);
}

#[test]
fn test_release_escrow() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _usdc_asset) = setup_test();
    let client = StellarPodEscrowClient::new(&env, &contract);

    let amount: i128 = 100_0000000;
    let platform_fee: i128 = 5_0000000;

    // Lock first
    client.lock(
        &merchant, &provider, &arbiter, &usdc_token,
        &amount, &platform_fee,
        &String::from_str(&env, "order_002"),
        &2_000_000u64,
    ).unwrap();

    // Release by arbiter
    let result = client.release(&arbiter);
    assert_eq!(result, Ok(()));

    // Verify state
    let escrow = client.get_state().unwrap();
    assert_eq!(escrow.state, EscrowState::Released);

    // Verify balances
    let usdc_client = token::Client::new(&env, &usdc_token);
    assert_eq!(usdc_client.balance(&provider), 95_0000000); // 100 - 5 fee
    assert_eq!(usdc_client.balance(&arbiter), 5_0000000);   // platform fee
    assert_eq!(usdc_client.balance(&contract), 0);           // contract empty
}

#[test]
fn test_refund_expired() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _usdc_asset) = setup_test();
    let client = StellarPodEscrowClient::new(&env, &contract);

    let amount: i128 = 100_0000000;
    let platform_fee: i128 = 5_0000000;
    let expires_at: u64 = 1_500_000; // Expires before we advance time

    // Lock
    client.lock(
        &merchant, &provider, &arbiter, &usdc_token,
        &amount, &platform_fee,
        &String::from_str(&env, "order_003"),
        &expires_at,
    ).unwrap();

    // Advance time past expiry
    env.ledger().set(LedgerInfo {
        timestamp: 2_000_000, // Past expires_at
        protocol_version: 21,
        sequence_number: 200,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 100,
        max_entry_ttl: 10_000_000,
    });

    // Anyone can refund after expiry (using provider as caller)
    let result = client.refund(&provider);
    assert_eq!(result, Ok(()));

    // Verify merchant got full refund
    let usdc_client = token::Client::new(&env, &usdc_token);
    assert_eq!(usdc_client.balance(&merchant), 10_000_0000000); // Back to original
    assert_eq!(usdc_client.balance(&contract), 0);
}

#[test]
fn test_dispute_and_resolve() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _usdc_asset) = setup_test();
    let client = StellarPodEscrowClient::new(&env, &contract);

    let amount: i128 = 100_0000000;
    let platform_fee: i128 = 5_0000000;

    // Lock
    client.lock(
        &merchant, &provider, &arbiter, &usdc_token,
        &amount, &platform_fee,
        &String::from_str(&env, "order_004"),
        &2_000_000u64,
    ).unwrap();

    // Merchant raises dispute
    let result = client.dispute(&merchant);
    assert_eq!(result, Ok(()));

    let escrow = client.get_state().unwrap();
    assert_eq!(escrow.state, EscrowState::Disputed);

    // Arbiter resolves: 70% to provider, 30% to merchant
    let result = client.resolve_dispute(&arbiter, &70u32);
    assert_eq!(result, Ok(()));

    // Verify split
    let usdc_client = token::Client::new(&env, &usdc_token);
    let net = amount - platform_fee; // 95 USDC
    let to_provider = (net * 70) / 100; // 66.5 USDC
    let to_merchant_refund = net - to_provider; // 28.5 USDC

    assert_eq!(usdc_client.balance(&provider), to_provider);
    assert_eq!(usdc_client.balance(&arbiter), platform_fee);
    // Merchant: original 10000 - locked 100 + refund portion
    assert_eq!(usdc_client.balance(&merchant), 10_000_0000000 - amount + to_merchant_refund);
}

#[test]
fn test_cannot_release_after_release() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _usdc_asset) = setup_test();
    let client = StellarPodEscrowClient::new(&env, &contract);

    client.lock(
        &merchant, &provider, &arbiter, &usdc_token,
        &100_0000000i128, &5_0000000i128,
        &String::from_str(&env, "order_005"),
        &2_000_000u64,
    ).unwrap();

    // Release once — OK
    client.release(&arbiter).unwrap();

    // Try release again — should fail
    let result = client.release(&arbiter);
    assert_eq!(result, Err(Ok(EscrowError::NotLocked)));
}

#[test]
fn test_unauthorized_release() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _usdc_asset) = setup_test();
    let client = StellarPodEscrowClient::new(&env, &contract);

    client.lock(
        &merchant, &provider, &arbiter, &usdc_token,
        &100_0000000i128, &5_0000000i128,
        &String::from_str(&env, "order_006"),
        &2_000_000u64,
    ).unwrap();

    // Provider tries to release — not authorized
    let random = Address::generate(&env);
    let result = client.release(&random);
    assert_eq!(result, Err(Ok(EscrowError::NotAuthorized)));
}

#[test]
fn test_time_remaining() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _usdc_asset) = setup_test();
    let client = StellarPodEscrowClient::new(&env, &contract);

    client.lock(
        &merchant, &provider, &arbiter, &usdc_token,
        &100_0000000i128, &5_0000000i128,
        &String::from_str(&env, "order_007"),
        &2_000_000u64,
    ).unwrap();

    // Current time: 1_000_000, expires: 2_000_000
    let remaining = client.time_remaining().unwrap();
    assert_eq!(remaining, 1_000_000);

    assert!(!client.is_expired().unwrap());
}
