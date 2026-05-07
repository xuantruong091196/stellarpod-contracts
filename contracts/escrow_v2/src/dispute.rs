use soroban_sdk::{token, Address, Env, String};
use crate::state::{DataKey, EscrowDataV2, EscrowState, TOTAL_BPS};
use crate::errors::EscrowErrorV2;

pub fn dispute(env: Env, caller: Address, order_id: String) -> Result<(), EscrowErrorV2> {
    caller.require_auth();
    let key = DataKey::Escrow(order_id);
    let mut escrow: EscrowDataV2 = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(EscrowErrorV2::NotFound)?;

    let is_beneficiary = escrow.beneficiaries.iter().any(|b| b.address == caller);
    if caller != escrow.merchant && caller != escrow.arbiter && !is_beneficiary {
        return Err(EscrowErrorV2::NotAuthorized);
    }
    if escrow.state != EscrowState::Locked {
        return Err(EscrowErrorV2::NotLocked);
    }

    escrow.state = EscrowState::Disputed;
    env.storage().persistent().set(&key, &escrow);
    Ok(())
}

/// Arbiter resolves dispute by allocating `provider_bps` of net_amount to the
/// beneficiary group; the remainder refunds to merchant. provider_bps is in
/// basis points (0..=10000) for consistency with the rest of the contract —
/// 100 bps = 1%, 5000 bps = 50%.
pub fn resolve_dispute(
    env: Env,
    arbiter: Address,
    order_id: String,
    provider_bps: u32,
) -> Result<(), EscrowErrorV2> {
    arbiter.require_auth();
    let key = DataKey::Escrow(order_id);
    let mut escrow: EscrowDataV2 = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(EscrowErrorV2::NotFound)?;
    if arbiter != escrow.arbiter {
        return Err(EscrowErrorV2::NotAuthorized);
    }
    if escrow.state != EscrowState::Disputed {
        return Err(EscrowErrorV2::NotDisputed);
    }
    if provider_bps > TOTAL_BPS {
        return Err(EscrowErrorV2::InvalidPercent);
    }

    let usdc = token::Client::new(&env, &escrow.usdc_token);
    let contract = env.current_contract_address();

    if escrow.platform_fee > 0 {
        usdc.transfer(&contract, &escrow.arbiter, &escrow.platform_fee);
    }

    let to_split = escrow
        .net_amount
        .checked_mul(provider_bps as i128)
        .ok_or(EscrowErrorV2::Overflow)?
        / TOTAL_BPS as i128;
    let to_refund = escrow.net_amount - to_split;

    if to_refund > 0 {
        usdc.transfer(&contract, &escrow.merchant, &to_refund);
    }

    let mut paid: i128 = 0;
    let count = escrow.beneficiaries.len();
    for (i, b) in escrow.beneficiaries.iter().enumerate() {
        let amount = if i as u32 == count - 1 {
            to_split - paid
        } else {
            to_split
                .checked_mul(b.percent_bps as i128)
                .ok_or(EscrowErrorV2::Overflow)?
                / TOTAL_BPS as i128
        };
        if amount > 0 {
            usdc.transfer(&contract, &b.address, &amount);
        }
        paid += amount;
    }

    escrow.state = EscrowState::Released;
    env.storage().persistent().set(&key, &escrow);
    Ok(())
}
