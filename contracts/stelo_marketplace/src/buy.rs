use soroban_sdk::{contracttype, token, Address, Env, IntoVal, Symbol, Vec};
use crate::state::{DataKey, Listing};
use crate::errors::MarketError;

const TOTAL_BPS: i128 = 10_000;

// Mirror types from stelo_nft for cross-contract policy retrieval.
// These must match the on-chain encoding of stelo_nft's RoyaltySplitOnChain
// and RoyaltyPolicy exactly (same field order, same contracttype attribute).
#[contracttype]
#[derive(Clone, Debug)]
pub struct RoyaltySplitOnChain {
    pub address: Address,
    pub percent_bps: u32,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct RoyaltyPolicy {
    pub splits: Vec<RoyaltySplitOnChain>,
    pub total_bps: u32,
}

pub fn buy(
    env: Env,
    buyer: Address,
    nft_contract: Address,
    token_id: u32,
) -> Result<(), MarketError> {
    buyer.require_auth();
    let key = DataKey::Listing(nft_contract.clone(), token_id);
    let listing: Listing = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(MarketError::NotListed)?;

    let usdc_token: Address = env
        .storage()
        .instance()
        .get(&DataKey::UsdcToken)
        .ok_or(MarketError::NotInitialized)?;
    let platform_fee_bps: u32 = env
        .storage()
        .instance()
        .get(&DataKey::PlatformFeeBps)
        .unwrap_or(250);
    let platform_addr: Address = env
        .storage()
        .instance()
        .get(&DataKey::PlatformAddress)
        .ok_or(MarketError::NotInitialized)?;

    // Query royalty policy from NFT contract (cross-contract call).
    // royalty_policy(token_id) returns RoyaltyPolicy (same XDR encoding).
    let policy: RoyaltyPolicy = env.invoke_contract(
        &nft_contract,
        &Symbol::new(&env, "royalty_policy"),
        soroban_sdk::vec![&env, token_id.into_val(&env)],
    );

    // Compute fees
    let price = listing.price;
    let platform_fee = price
        .checked_mul(platform_fee_bps as i128)
        .ok_or(MarketError::Overflow)?
        / TOTAL_BPS;
    let royalty_total = price
        .checked_mul(policy.total_bps as i128)
        .ok_or(MarketError::Overflow)?
        / TOTAL_BPS;
    let seller_amount = price
        .checked_sub(platform_fee)
        .ok_or(MarketError::Overflow)?
        .checked_sub(royalty_total)
        .ok_or(MarketError::Overflow)?;
    if seller_amount < 0 {
        return Err(MarketError::InvalidSplit);
    }

    let usdc = token::Client::new(&env, &usdc_token);
    let market_addr = env.current_contract_address();

    // 1. Buyer pays full price to marketplace escrow
    usdc.transfer(&buyer, &market_addr, &price);

    // 2. Platform fee
    if platform_fee > 0 {
        usdc.transfer(&market_addr, &platform_addr, &platform_fee);
    }

    // 3. Royalties — atomic split with last-recipient absorbs dust
    let mut paid: i128 = 0;
    let count = policy.splits.len();
    for (i, s) in policy.splits.iter().enumerate() {
        let amt = if i as u32 == count - 1 {
            // Last split absorbs any rounding dust
            royalty_total - paid
        } else {
            royalty_total
                .checked_mul(s.percent_bps as i128)
                .ok_or(MarketError::Overflow)?
                / policy.total_bps as i128
        };
        if amt > 0 {
            usdc.transfer(&market_addr, &s.address, &amt);
        }
        paid = paid.checked_add(amt).ok_or(MarketError::Overflow)?;
    }

    // 4. Seller receives remaining proceeds
    if seller_amount > 0 {
        usdc.transfer(&market_addr, &listing.seller, &seller_amount);
    }

    // 5. Transfer NFT from marketplace escrow to buyer
    env.invoke_contract::<()>(
        &nft_contract,
        &Symbol::new(&env, "marketplace_transfer"),
        soroban_sdk::vec![
            &env,
            market_addr.into_val(&env),
            buyer.clone().into_val(&env),
            token_id.into_val(&env),
        ],
    );

    // 6. Remove listing + emit sale event
    env.storage().persistent().remove(&key);
    env.events().publish(
        (Symbol::new(&env, "sale"),),
        (
            buyer,
            listing.seller,
            token_id,
            price,
            royalty_total,
            platform_fee,
        ),
    );
    Ok(())
}
