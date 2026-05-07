use soroban_sdk::{token, Address, Env, String};
use crate::state::{DataKey, EscrowDataV2, EscrowState, TOTAL_BPS};
use crate::errors::EscrowErrorV2;

pub fn release(env: Env, caller: Address, order_id: String) -> Result<(), EscrowErrorV2> {
    caller.require_auth();
    let key = DataKey::Escrow(order_id);
    let mut escrow: EscrowDataV2 = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(EscrowErrorV2::NotFound)?;

    if caller != escrow.arbiter && caller != escrow.merchant {
        return Err(EscrowErrorV2::NotAuthorized);
    }
    if escrow.state != EscrowState::Locked {
        return Err(EscrowErrorV2::NotLocked);
    }

    let usdc = token::Client::new(&env, &escrow.usdc_token);
    let contract = env.current_contract_address();

    // 1. Platform fee → arbiter
    if escrow.platform_fee > 0 {
        usdc.transfer(&contract, &escrow.arbiter, &escrow.platform_fee);
    }

    // 2. Net amount → beneficiaries (proportional, last absorbs rounding dust)
    let net = escrow.net_amount;
    let mut paid_so_far: i128 = 0;
    let count = escrow.beneficiaries.len();
    for (i, b) in escrow.beneficiaries.iter().enumerate() {
        let amount = if i as u32 == count - 1 {
            net - paid_so_far // dust absorbed by last
        } else {
            net.checked_mul(b.percent_bps as i128)
                .ok_or(EscrowErrorV2::Overflow)?
                / TOTAL_BPS as i128
        };
        if amount > 0 {
            usdc.transfer(&contract, &b.address, &amount);
        }
        paid_so_far = paid_so_far
            .checked_add(amount)
            .ok_or(EscrowErrorV2::Overflow)?;
    }

    escrow.state = EscrowState::Released;
    env.storage().persistent().set(&key, &escrow);
    Ok(())
}
