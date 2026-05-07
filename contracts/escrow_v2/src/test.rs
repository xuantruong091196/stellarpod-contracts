#![cfg(test)]

use super::*;
use crate::errors::EscrowErrorV2;
use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    token, vec, Address, Env, String, Symbol, Vec,
};

fn setup_test() -> (
    Env,
    Address, // contract
    Address, // merchant
    Address, // arbiter
    Address, // usdc token
    token::StellarAssetClient<'static>,
) {
    let env = Env::default();
    env.mock_all_auths();

    let contract = env.register_contract(None, EscrowV2);

    let merchant = Address::generate(&env);
    let arbiter = Address::generate(&env);

    let usdc_admin = Address::generate(&env);
    let usdc_token = env.register_stellar_asset_contract_v2(usdc_admin.clone());
    let usdc_asset = token::StellarAssetClient::new(&env, &usdc_token.address());

    usdc_asset.mint(&merchant, &10_000_0000000);

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

    (env, contract, merchant, arbiter, usdc_token.address(), usdc_asset)
}

fn oid(env: &Env, s: &str) -> String {
    String::from_str(env, s)
}

fn ben(env: &Env, address: &Address, percent_bps: u32, role: &str) -> Beneficiary {
    Beneficiary {
        address: address.clone(),
        percent_bps,
        role_tag: Symbol::new(env, role),
    }
}

// ─── LOCK INVARIANT TESTS ─────────────────────────────────────────

#[test]
fn lock_succeeds_with_valid_3way_split() {
    let (env, contract, merchant, arbiter, usdc_token, _) = setup_test();
    let client = EscrowV2Client::new(&env, &contract);

    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let c = Address::generate(&env);
    let bs = vec![&env,
        ben(&env, &a, 7000, "merchant"),
        ben(&env, &b, 2000, "designer"),
        ben(&env, &c, 1000, "influencer"),
    ];

    client.lock(
        &merchant, &arbiter, &usdc_token,
        &100_0000000i128, &5_0000000i128, &bs,
        &oid(&env, "order_001"), &2_000_000u64,
    );

    let usdc = token::Client::new(&env, &usdc_token);
    assert_eq!(usdc.balance(&contract), 100_0000000);
}

#[test]
fn lock_rejects_bps_not_summing_to_10000() {
    let (env, contract, merchant, arbiter, usdc_token, _) = setup_test();
    let client = EscrowV2Client::new(&env, &contract);
    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let bs = vec![&env,
        ben(&env, &a, 5000, "merchant"),
        ben(&env, &b, 4000, "designer"), // sum = 9000, missing 1000
    ];
    let result = client.try_lock(
        &merchant, &arbiter, &usdc_token,
        &100_0000000i128, &5_0000000i128, &bs,
        &oid(&env, "order_001"), &2_000_000u64,
    );
    assert_eq!(result, Err(Ok(EscrowErrorV2::BpsSumMismatch)));
}

#[test]
fn lock_rejects_too_many_beneficiaries() {
    let (env, contract, merchant, arbiter, usdc_token, _) = setup_test();
    let client = EscrowV2Client::new(&env, &contract);
    let mut bs = vec![&env];
    for _ in 0..9 {
        let a = Address::generate(&env);
        bs.push_back(ben(&env, &a, 1111, "x")); // 9 entries > 8 max
    }
    let result = client.try_lock(
        &merchant, &arbiter, &usdc_token,
        &100_0000000i128, &5_0000000i128, &bs,
        &oid(&env, "order_001"), &2_000_000u64,
    );
    assert_eq!(result, Err(Ok(EscrowErrorV2::TooManyBeneficiaries)));
}

#[test]
fn lock_rejects_empty_beneficiaries() {
    let (env, contract, merchant, arbiter, usdc_token, _) = setup_test();
    let client = EscrowV2Client::new(&env, &contract);
    let bs: Vec<Beneficiary> = vec![&env];
    let result = client.try_lock(
        &merchant, &arbiter, &usdc_token,
        &100_0000000i128, &5_0000000i128, &bs,
        &oid(&env, "order_001"), &2_000_000u64,
    );
    assert_eq!(result, Err(Ok(EscrowErrorV2::EmptyBeneficiaries)));
}

#[test]
fn lock_rejects_duplicate_order_id() {
    let (env, contract, merchant, arbiter, usdc_token, _) = setup_test();
    let client = EscrowV2Client::new(&env, &contract);
    let a = Address::generate(&env);
    let bs = vec![&env, ben(&env, &a, 10000, "all")];

    client.lock(
        &merchant, &arbiter, &usdc_token,
        &50_0000000i128, &2_5000000i128, &bs,
        &oid(&env, "order_dup"), &2_000_000u64,
    );

    let result = client.try_lock(
        &merchant, &arbiter, &usdc_token,
        &50_0000000i128, &2_5000000i128, &bs,
        &oid(&env, "order_dup"), &2_000_000u64,
    );
    assert_eq!(result, Err(Ok(EscrowErrorV2::AlreadyExists)));
}

// ─── RELEASE TESTS ─────────────────────────────────────────────────

#[test]
fn release_pays_each_beneficiary_proportionally() {
    let (env, contract, merchant, arbiter, usdc_token, _) = setup_test();
    let client = EscrowV2Client::new(&env, &contract);

    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let c = Address::generate(&env);
    // Splits 70/20/10. Net = 100 - 10 fee = 90. Splits: A=63, B=18, C=9.
    let bs = vec![&env,
        ben(&env, &a, 7000, "a"),
        ben(&env, &b, 2000, "b"),
        ben(&env, &c, 1000, "c"),
    ];

    client.lock(
        &merchant, &arbiter, &usdc_token,
        &100_0000000i128, &10_0000000i128, &bs,
        &oid(&env, "rls"), &2_000_000u64,
    );

    client.release(&arbiter, &oid(&env, "rls"));

    let usdc = token::Client::new(&env, &usdc_token);
    assert_eq!(usdc.balance(&arbiter), 10_0000000);
    assert_eq!(usdc.balance(&a), 63_0000000); // 90 * 70/100
    assert_eq!(usdc.balance(&b), 18_0000000); // 90 * 20/100
    assert_eq!(usdc.balance(&c), 9_0000000);  // last absorbs dust; 90 - 63 - 18 = 9
    assert_eq!(usdc.balance(&contract), 0);
}

#[test]
fn release_dust_absorbed_by_last_beneficiary() {
    let (env, contract, merchant, arbiter, usdc_token, _) = setup_test();
    let client = EscrowV2Client::new(&env, &contract);

    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let c = Address::generate(&env);
    // Net = 7 stroops. Splits 33/33/34 of 7:
    //   A: 7 * 3300 / 10000 = 2 (truncated)
    //   B: 7 * 3300 / 10000 = 2 (truncated)
    //   C: net - paid_so_far = 7 - 4 = 3 (absorbs dust)
    let bs = vec![&env,
        ben(&env, &a, 3300, "a"),
        ben(&env, &b, 3300, "b"),
        ben(&env, &c, 3400, "c"),
    ];

    client.lock(
        &merchant, &arbiter, &usdc_token,
        &7i128, &0i128, &bs,
        &oid(&env, "dust"), &2_000_000u64,
    );

    client.release(&arbiter, &oid(&env, "dust"));

    let usdc = token::Client::new(&env, &usdc_token);
    assert_eq!(usdc.balance(&a), 2);
    assert_eq!(usdc.balance(&b), 2);
    assert_eq!(usdc.balance(&c), 3); // dust absorbed
    assert_eq!(usdc.balance(&contract), 0);
}

#[test]
fn release_only_arbiter_or_merchant_can_call() {
    let (env, contract, merchant, arbiter, usdc_token, _) = setup_test();
    let client = EscrowV2Client::new(&env, &contract);
    let a = Address::generate(&env);
    let bs = vec![&env, ben(&env, &a, 10000, "all")];

    client.lock(
        &merchant, &arbiter, &usdc_token,
        &50_0000000i128, &2_5000000i128, &bs,
        &oid(&env, "auth"), &2_000_000u64,
    );

    let stranger = Address::generate(&env);
    let result = client.try_release(&stranger, &oid(&env, "auth"));
    assert_eq!(result, Err(Ok(EscrowErrorV2::NotAuthorized)));
}

// ─── DISPUTE TESTS ─────────────────────────────────────────────────

#[test]
fn dispute_then_resolve_partial_splits_correctly() {
    let (env, contract, merchant, arbiter, usdc_token, _) = setup_test();
    let client = EscrowV2Client::new(&env, &contract);

    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let c = Address::generate(&env);
    let bs = vec![&env,
        ben(&env, &a, 7000, "a"),
        ben(&env, &b, 2000, "b"),
        ben(&env, &c, 1000, "c"),
    ];

    // Lock 100 USDC, fee 10, splits 70/20/10
    client.lock(
        &merchant, &arbiter, &usdc_token,
        &100_0000000i128, &10_0000000i128, &bs,
        &oid(&env, "dis"), &2_000_000u64,
    );
    client.dispute(&merchant, &oid(&env, "dis"));
    // Arbiter resolves: 60% of net to beneficiary group; 40% refund to merchant
    client.resolve_dispute(&arbiter, &oid(&env, "dis"), &6000u32);

    let usdc = token::Client::new(&env, &usdc_token);
    // Fee 10 → arbiter
    assert_eq!(usdc.balance(&arbiter), 10_0000000);
    // Net 90 * 60% = 54 for beneficiaries; 90 - 54 = 36 refund
    // A = 54 * 7000/10000 = 37.8 → 37_8000000 (i128 floor on stroops)
    // B = 54 * 2000/10000 = 10.8 → 10_8000000
    // C = 54 - 37.8 - 10.8 = 5.4 (absorbs dust as last) — 54_0000000 - 37_8000000 - 10_8000000 = 5_4000000
    assert_eq!(usdc.balance(&a), 37_8000000);
    assert_eq!(usdc.balance(&b), 10_8000000);
    assert_eq!(usdc.balance(&c), 5_4000000);
    assert_eq!(usdc.balance(&merchant), 10_000_0000000 - 100_0000000 + 36_0000000); // started with 10000 USDC, spent 100, got 36 refund
    assert_eq!(usdc.balance(&contract), 0);
}

#[test]
fn dispute_callable_by_beneficiary() {
    let (env, contract, merchant, arbiter, usdc_token, _) = setup_test();
    let client = EscrowV2Client::new(&env, &contract);
    let a = Address::generate(&env);
    let bs = vec![&env, ben(&env, &a, 10000, "all")];

    client.lock(
        &merchant, &arbiter, &usdc_token,
        &50_0000000i128, &2_5000000i128, &bs,
        &oid(&env, "ben_disp"), &2_000_000u64,
    );

    // Beneficiary A can dispute
    client.dispute(&a, &oid(&env, "ben_disp"));
}

#[test]
fn dispute_rejects_unauthorized() {
    let (env, contract, merchant, arbiter, usdc_token, _) = setup_test();
    let client = EscrowV2Client::new(&env, &contract);
    let a = Address::generate(&env);
    let bs = vec![&env, ben(&env, &a, 10000, "all")];

    client.lock(
        &merchant, &arbiter, &usdc_token,
        &50_0000000i128, &2_5000000i128, &bs,
        &oid(&env, "stranger_disp"), &2_000_000u64,
    );

    let stranger = Address::generate(&env);
    let result = client.try_dispute(&stranger, &oid(&env, "stranger_disp"));
    assert_eq!(result, Err(Ok(EscrowErrorV2::NotAuthorized)));
}

#[test]
fn resolve_dispute_rejects_bps_over_10000() {
    let (env, contract, merchant, arbiter, usdc_token, _) = setup_test();
    let client = EscrowV2Client::new(&env, &contract);
    let a = Address::generate(&env);
    let bs = vec![&env, ben(&env, &a, 10000, "all")];

    client.lock(
        &merchant, &arbiter, &usdc_token,
        &50_0000000i128, &2_5000000i128, &bs,
        &oid(&env, "bps_high"), &2_000_000u64,
    );
    client.dispute(&merchant, &oid(&env, "bps_high"));
    let result = client.try_resolve_dispute(&arbiter, &oid(&env, "bps_high"), &10_001u32);
    assert_eq!(result, Err(Ok(EscrowErrorV2::InvalidPercent)));
}
