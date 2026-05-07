#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, vec, Address, BytesN, Env};

fn empty_policy(env: &Env) -> RoyaltyPolicy {
    RoyaltyPolicy {
        splits: vec![&env],
        total_bps: 0,
    }
}

fn setup() -> (Env, SteloNftClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract = env.register_contract(None, SteloNft);
    let client = SteloNftClient::new(&env, &contract);
    let admin = Address::generate(&env);
    let marketplace = Address::generate(&env);
    client.init(&admin, &marketplace);
    (env, client, admin, marketplace)
}

#[test]
fn mint_with_no_royalty() {
    let (env, client, _, _) = setup();
    let owner = Address::generate(&env);
    let policy = empty_policy(&env);
    let id = client.mint(&owner, &BytesN::from_array(&env, &[0u8; 32]), &policy);
    assert_eq!(id, 1);
    assert_eq!(client.owner_of(&id), owner);
}

#[test]
fn mint_with_valid_royalty() {
    let (env, client, _, _) = setup();
    let designer = Address::generate(&env);
    let owner = Address::generate(&env);
    let splits = vec![
        &env,
        RoyaltySplitOnChain {
            address: designer.clone(),
            percent_bps: 1000,
        },
    ]; // 10%
    let policy = RoyaltyPolicy {
        splits,
        total_bps: 1000,
    };
    let id = client.mint(&owner, &BytesN::from_array(&env, &[1u8; 32]), &policy);
    let p = client.royalty_policy(&id);
    assert_eq!(p.total_bps, 1000);
    assert_eq!(p.splits.len(), 1);
}

#[test]
fn mint_serial_increments() {
    let (env, client, _, _) = setup();
    let owner = Address::generate(&env);
    let id1 = client.mint(
        &owner,
        &BytesN::from_array(&env, &[1u8; 32]),
        &empty_policy(&env),
    );
    let id2 = client.mint(
        &owner,
        &BytesN::from_array(&env, &[2u8; 32]),
        &empty_policy(&env),
    );
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
}

#[test]
fn mint_rejects_royalty_over_25_percent() {
    let (env, client, _, _) = setup();
    let owner = Address::generate(&env);
    let splits = vec![
        &env,
        RoyaltySplitOnChain {
            address: Address::generate(&env),
            percent_bps: 3000,
        },
    ];
    let policy = RoyaltyPolicy {
        splits,
        total_bps: 3000,
    }; // 30% > 25% cap
    let result =
        client.try_mint(&owner, &BytesN::from_array(&env, &[0u8; 32]), &policy);
    assert_eq!(result, Err(Ok(NftError::InvalidRoyalty)));
}

#[test]
fn mint_rejects_split_sum_mismatch() {
    let (env, client, _, _) = setup();
    let splits = vec![
        &env,
        RoyaltySplitOnChain {
            address: Address::generate(&env),
            percent_bps: 500,
        },
        RoyaltySplitOnChain {
            address: Address::generate(&env),
            percent_bps: 400,
        },
    ];
    let policy = RoyaltyPolicy {
        splits,
        total_bps: 1000,
    }; // sum=900 != 1000
    let result = client.try_mint(
        &Address::generate(&env),
        &BytesN::from_array(&env, &[0u8; 32]),
        &policy,
    );
    assert_eq!(result, Err(Ok(NftError::InvalidSplit)));
}

#[test]
fn p2p_transfer_changes_owner() {
    let (env, client, _, _) = setup();
    let owner_a = Address::generate(&env);
    let owner_b = Address::generate(&env);
    let id = client.mint(
        &owner_a,
        &BytesN::from_array(&env, &[0u8; 32]),
        &empty_policy(&env),
    );
    client.transfer(&owner_a, &owner_b, &id);
    assert_eq!(client.owner_of(&id), owner_b);
}

#[test]
fn p2p_transfer_rejects_non_owner() {
    let (env, client, _, _) = setup();
    let owner_a = Address::generate(&env);
    let stranger = Address::generate(&env);
    let owner_b = Address::generate(&env);
    let id = client.mint(
        &owner_a,
        &BytesN::from_array(&env, &[0u8; 32]),
        &empty_policy(&env),
    );
    let result = client.try_transfer(&stranger, &owner_b, &id);
    assert_eq!(result, Err(Ok(NftError::NotAuthorized)));
}

#[test]
fn marketplace_transfer_changes_owner() {
    let (env, client, _, _) = setup();
    let owner = Address::generate(&env);
    let buyer = Address::generate(&env);
    let id = client.mint(
        &owner,
        &BytesN::from_array(&env, &[0u8; 32]),
        &empty_policy(&env),
    );
    // mock_all_auths_allowing_non_root_auth() lets the stored marketplace address sign
    env.mock_all_auths_allowing_non_root_auth();
    client.marketplace_transfer(&owner, &buyer, &id);
    assert_eq!(client.owner_of(&id), buyer);
}

#[test]
fn metadata_hash_persists() {
    let (env, client, _, _) = setup();
    let owner = Address::generate(&env);
    let hash = BytesN::from_array(&env, &[7u8; 32]);
    let id = client.mint(&owner, &hash, &empty_policy(&env));
    assert_eq!(client.metadata_hash(&id), hash);
}

#[test]
fn set_royalty_policy_updates_token() {
    let (env, client, _, _) = setup();
    let owner = Address::generate(&env);
    let id = client.mint(
        &owner,
        &BytesN::from_array(&env, &[0u8; 32]),
        &empty_policy(&env),
    );
    let new_policy = RoyaltyPolicy {
        splits: vec![
            &env,
            RoyaltySplitOnChain {
                address: Address::generate(&env),
                percent_bps: 1500,
            },
        ],
        total_bps: 1500,
    };
    client.set_royalty_policy(&id, &new_policy);
    assert_eq!(client.royalty_policy(&id).total_bps, 1500);
}

#[test]
fn views_reject_unknown_token() {
    let (_, client, _, _) = setup();
    let result = client.try_owner_of(&999u32);
    assert_eq!(result, Err(Ok(NftError::TokenNotFound)));
}

#[test]
fn double_init_rejected() {
    let (env, client, _, _) = setup();
    let result =
        client.try_init(&Address::generate(&env), &Address::generate(&env));
    assert_eq!(result, Err(Ok(NftError::AlreadyInitialized)));
}
