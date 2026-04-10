#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    token, Address, Env, String,
};

fn setup_test() -> (
    Env,
    Address,                    // contract
    Address,                    // merchant
    Address,                    // provider
    Address,                    // arbiter
    Address,                    // usdc token
    token::StellarAssetClient<'static>,
) {
    let env = Env::default();
    env.mock_all_auths();

    let contract = env.register_contract(None, SteloEscrow);

    let merchant = Address::generate(&env);
    let provider = Address::generate(&env);
    let arbiter  = Address::generate(&env);

    let usdc_admin = Address::generate(&env);
    let usdc_token = env.register_stellar_asset_contract_v2(usdc_admin.clone());
    let usdc_asset = token::StellarAssetClient::new(&env, &usdc_token.address());

    usdc_asset.mint(&merchant, &10_000_0000000);

    env.ledger().set(LedgerInfo {
        timestamp:             1_000_000,
        protocol_version:      21,
        sequence_number:       100,
        network_id:            Default::default(),
        base_reserve:          10,
        min_temp_entry_ttl:    100,
        min_persistent_entry_ttl: 100,
        max_entry_ttl:         10_000_000,
    });

    (env, contract, merchant, provider, arbiter, usdc_token.address(), usdc_asset)
}

fn oid(env: &Env, s: &str) -> String {
    String::from_str(env, s)
}

// ─── LOCK TESTS ───────────────────────────────────────────────────

#[test]
fn test_lock_escrow() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _) = setup_test();
    let client = SteloEscrowClient::new(&env, &contract);
    let order = oid(&env, "order_001");

    client.lock(&merchant, &provider, &arbiter, &usdc_token,
                &100_0000000i128, &5_0000000i128, &order, &2_000_000u64);

    let escrow = client.get_state(&order);
    assert_eq!(escrow.state, EscrowState::Locked);
    assert_eq!(escrow.amount, 100_0000000);
    assert_eq!(escrow.platform_fee, 5_0000000);
    assert_eq!(escrow.provider_amount, 95_0000000);

    let usdc = token::Client::new(&env, &usdc_token);
    assert_eq!(usdc.balance(&contract), 100_0000000);
}

#[test]
fn test_lock_multiple_escrows() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _) = setup_test();
    let client = SteloEscrowClient::new(&env, &contract);

    client.lock(&merchant, &provider, &arbiter, &usdc_token,
                &50_0000000i128, &2_5000000i128, &oid(&env, "order_A"), &2_000_000u64);
    client.lock(&merchant, &provider, &arbiter, &usdc_token,
                &30_0000000i128, &1_5000000i128, &oid(&env, "order_B"), &2_000_000u64);

    let a = client.get_state(&oid(&env, "order_A"));
    let b = client.get_state(&oid(&env, "order_B"));
    assert_eq!(a.amount, 50_0000000);
    assert_eq!(b.amount, 30_0000000);

    let usdc = token::Client::new(&env, &usdc_token);
    assert_eq!(usdc.balance(&contract), 80_0000000);
}

#[test]
fn test_lock_duplicate_order_fails() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _) = setup_test();
    let client = SteloEscrowClient::new(&env, &contract);
    let order = oid(&env, "dup_order");

    client.lock(&merchant, &provider, &arbiter, &usdc_token,
                &100_0000000i128, &5_0000000i128, &order, &2_000_000u64);

    let result = client.try_lock(&merchant, &provider, &arbiter, &usdc_token,
                &50_0000000i128, &2_0000000i128, &order, &2_000_000u64);
    assert_eq!(result, Err(Ok(EscrowError::AlreadyExists)));
}

#[test]
fn test_lock_zero_amount_fails() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _) = setup_test();
    let client = SteloEscrowClient::new(&env, &contract);

    let result = client.try_lock(&merchant, &provider, &arbiter, &usdc_token,
                &0i128, &0i128, &oid(&env, "zero"), &2_000_000u64);
    assert_eq!(result, Err(Ok(EscrowError::InvalidAmount)));
}

#[test]
fn test_lock_fee_equal_amount_fails() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _) = setup_test();
    let client = SteloEscrowClient::new(&env, &contract);

    let result = client.try_lock(&merchant, &provider, &arbiter, &usdc_token,
                &100i128, &100i128, &oid(&env, "bad_fee"), &2_000_000u64);
    assert_eq!(result, Err(Ok(EscrowError::InvalidFee)));
}

#[test]
fn test_lock_expired_time_fails() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _) = setup_test();
    let client = SteloEscrowClient::new(&env, &contract);

    // Current time is 1_000_000; expires_at is in the past
    let result = client.try_lock(&merchant, &provider, &arbiter, &usdc_token,
                &100_0000000i128, &5_0000000i128, &oid(&env, "expired"), &500_000u64);
    assert_eq!(result, Err(Ok(EscrowError::AlreadyExpired)));
}

// ─── RELEASE TESTS ────────────────────────────────────────────────

#[test]
fn test_release_escrow() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _) = setup_test();
    let client = SteloEscrowClient::new(&env, &contract);
    let order = oid(&env, "rel_001");

    client.lock(&merchant, &provider, &arbiter, &usdc_token,
                &100_0000000i128, &5_0000000i128, &order, &2_000_000u64);
    client.release(&arbiter, &order);

    let escrow = client.get_state(&order);
    assert_eq!(escrow.state, EscrowState::Released);

    let usdc = token::Client::new(&env, &usdc_token);
    assert_eq!(usdc.balance(&provider), 95_0000000);
    assert_eq!(usdc.balance(&arbiter), 5_0000000);
    assert_eq!(usdc.balance(&contract), 0);
}

#[test]
fn test_merchant_can_release() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _) = setup_test();
    let client = SteloEscrowClient::new(&env, &contract);
    let order = oid(&env, "merch_rel");

    client.lock(&merchant, &provider, &arbiter, &usdc_token,
                &100_0000000i128, &5_0000000i128, &order, &2_000_000u64);
    client.release(&merchant, &order);

    assert_eq!(client.get_state(&order).state, EscrowState::Released);
}

#[test]
fn test_cannot_release_after_release() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _) = setup_test();
    let client = SteloEscrowClient::new(&env, &contract);
    let order = oid(&env, "dbl_rel");

    client.lock(&merchant, &provider, &arbiter, &usdc_token,
                &100_0000000i128, &5_0000000i128, &order, &2_000_000u64);
    client.release(&arbiter, &order);

    let result = client.try_release(&arbiter, &order);
    assert_eq!(result, Err(Ok(EscrowError::NotLocked)));
}

#[test]
fn test_unauthorized_release() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _) = setup_test();
    let client = SteloEscrowClient::new(&env, &contract);
    let order = oid(&env, "unauth_rel");

    client.lock(&merchant, &provider, &arbiter, &usdc_token,
                &100_0000000i128, &5_0000000i128, &order, &2_000_000u64);

    let random = Address::generate(&env);
    let result = client.try_release(&random, &order);
    assert_eq!(result, Err(Ok(EscrowError::NotAuthorized)));
}

#[test]
fn test_provider_cannot_release() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _) = setup_test();
    let client = SteloEscrowClient::new(&env, &contract);
    let order = oid(&env, "prov_rel");

    client.lock(&merchant, &provider, &arbiter, &usdc_token,
                &100_0000000i128, &5_0000000i128, &order, &2_000_000u64);

    let result = client.try_release(&provider, &order);
    assert_eq!(result, Err(Ok(EscrowError::NotAuthorized)));
}

// ─── REFUND TESTS ─────────────────────────────────────────────────

#[test]
fn test_refund_expired() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _) = setup_test();
    let client = SteloEscrowClient::new(&env, &contract);
    let order = oid(&env, "ref_exp");

    client.lock(&merchant, &provider, &arbiter, &usdc_token,
                &100_0000000i128, &5_0000000i128, &order, &1_500_000u64);

    env.ledger().set(LedgerInfo {
        timestamp: 2_000_000, protocol_version: 21, sequence_number: 200,
        network_id: Default::default(), base_reserve: 10,
        min_temp_entry_ttl: 100, min_persistent_entry_ttl: 100, max_entry_ttl: 10_000_000,
    });

    // Anyone can refund after expiry
    client.refund(&provider, &order);

    let usdc = token::Client::new(&env, &usdc_token);
    assert_eq!(usdc.balance(&merchant), 10_000_0000000);
    assert_eq!(usdc.balance(&contract), 0);
}

#[test]
fn test_refund_before_expiry_unauthorized() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _) = setup_test();
    let client = SteloEscrowClient::new(&env, &contract);
    let order = oid(&env, "ref_early");

    client.lock(&merchant, &provider, &arbiter, &usdc_token,
                &100_0000000i128, &5_0000000i128, &order, &2_000_000u64);

    // Provider tries to refund before expiry — not arbiter
    let result = client.try_refund(&provider, &order);
    assert_eq!(result, Err(Ok(EscrowError::NotAuthorized)));
}

#[test]
fn test_refund_from_disputed_state() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _) = setup_test();
    let client = SteloEscrowClient::new(&env, &contract);
    let order = oid(&env, "ref_disp");

    client.lock(&merchant, &provider, &arbiter, &usdc_token,
                &100_0000000i128, &5_0000000i128, &order, &2_000_000u64);
    client.dispute(&merchant, &order);

    // Arbiter refunds disputed escrow
    client.refund(&arbiter, &order);
    assert_eq!(client.get_state(&order).state, EscrowState::Refunded);
}

// ─── DISPUTE TESTS ────────────────────────────────────────────────

#[test]
fn test_dispute_and_resolve_70() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _) = setup_test();
    let client = SteloEscrowClient::new(&env, &contract);
    let order = oid(&env, "disp_70");

    client.lock(&merchant, &provider, &arbiter, &usdc_token,
                &100_0000000i128, &5_0000000i128, &order, &2_000_000u64);
    client.dispute(&merchant, &order);
    assert_eq!(client.get_state(&order).state, EscrowState::Disputed);

    client.resolve_dispute(&arbiter, &order, &70u32);

    let usdc = token::Client::new(&env, &usdc_token);
    let net = 95_0000000i128;
    let to_provider = (net * 70) / 100;
    let to_merchant = net - to_provider;

    assert_eq!(usdc.balance(&provider), to_provider);
    assert_eq!(usdc.balance(&arbiter), 5_0000000);
    assert_eq!(usdc.balance(&merchant), 10_000_0000000 - 100_0000000 + to_merchant);
}

#[test]
fn test_resolve_with_0_percent() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _) = setup_test();
    let client = SteloEscrowClient::new(&env, &contract);
    let order = oid(&env, "disp_0");

    client.lock(&merchant, &provider, &arbiter, &usdc_token,
                &100_0000000i128, &5_0000000i128, &order, &2_000_000u64);
    client.dispute(&merchant, &order);
    client.resolve_dispute(&arbiter, &order, &0u32);

    let usdc = token::Client::new(&env, &usdc_token);
    assert_eq!(usdc.balance(&provider), 0);
    assert_eq!(usdc.balance(&merchant), 10_000_0000000 - 5_0000000); // got net back
}

#[test]
fn test_resolve_with_100_percent() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _) = setup_test();
    let client = SteloEscrowClient::new(&env, &contract);
    let order = oid(&env, "disp_100");

    client.lock(&merchant, &provider, &arbiter, &usdc_token,
                &100_0000000i128, &5_0000000i128, &order, &2_000_000u64);
    client.dispute(&provider, &order);
    client.resolve_dispute(&arbiter, &order, &100u32);

    let usdc = token::Client::new(&env, &usdc_token);
    assert_eq!(usdc.balance(&provider), 95_0000000); // all net
    assert_eq!(usdc.balance(&merchant), 10_000_0000000 - 100_0000000); // nothing back
}

#[test]
fn test_resolve_invalid_percent() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _) = setup_test();
    let client = SteloEscrowClient::new(&env, &contract);
    let order = oid(&env, "disp_bad");

    client.lock(&merchant, &provider, &arbiter, &usdc_token,
                &100_0000000i128, &5_0000000i128, &order, &2_000_000u64);
    client.dispute(&merchant, &order);

    let result = client.try_resolve_dispute(&arbiter, &order, &101u32);
    assert_eq!(result, Err(Ok(EscrowError::InvalidPercent)));
}

#[test]
fn test_provider_can_dispute() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _) = setup_test();
    let client = SteloEscrowClient::new(&env, &contract);
    let order = oid(&env, "prov_disp");

    client.lock(&merchant, &provider, &arbiter, &usdc_token,
                &100_0000000i128, &5_0000000i128, &order, &2_000_000u64);
    client.dispute(&provider, &order);

    assert_eq!(client.get_state(&order).state, EscrowState::Disputed);
}

#[test]
fn test_arbiter_cannot_dispute() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _) = setup_test();
    let client = SteloEscrowClient::new(&env, &contract);
    let order = oid(&env, "arb_disp");

    client.lock(&merchant, &provider, &arbiter, &usdc_token,
                &100_0000000i128, &5_0000000i128, &order, &2_000_000u64);

    let result = client.try_dispute(&arbiter, &order);
    assert_eq!(result, Err(Ok(EscrowError::NotAuthorized)));
}

// ─── QUERY TESTS ──────────────────────────────────────────────────

#[test]
fn test_time_remaining() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _) = setup_test();
    let client = SteloEscrowClient::new(&env, &contract);
    let order = oid(&env, "time_q");

    client.lock(&merchant, &provider, &arbiter, &usdc_token,
                &100_0000000i128, &5_0000000i128, &order, &2_000_000u64);

    assert_eq!(client.time_remaining(&order), 1_000_000);
    assert!(!client.is_expired(&order));
}

#[test]
fn test_not_found() {
    let (env, contract, _, _, _, _, _) = setup_test();
    let client = SteloEscrowClient::new(&env, &contract);

    let result = client.try_get_state(&oid(&env, "nonexistent"));
    assert_eq!(result, Err(Ok(EscrowError::NotFound)));
}

// ─── PLATFORM FEE ROUTING ─────────────────────────────────────────

#[test]
fn test_zero_platform_fee_release() {
    let (env, contract, merchant, provider, arbiter, usdc_token, _) = setup_test();
    let client = SteloEscrowClient::new(&env, &contract);
    let order = oid(&env, "no_fee");

    client.lock(&merchant, &provider, &arbiter, &usdc_token,
                &100_0000000i128, &0i128, &order, &2_000_000u64);
    client.release(&arbiter, &order);

    let usdc = token::Client::new(&env, &usdc_token);
    assert_eq!(usdc.balance(&provider), 100_0000000);
    assert_eq!(usdc.balance(&arbiter), 0);
}
