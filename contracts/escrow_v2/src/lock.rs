use soroban_sdk::{token, Address, Env, String, Vec};
use crate::state::{Beneficiary, DataKey, EscrowDataV2, EscrowState, MAX_BENEFICIARIES, TOTAL_BPS};
use crate::errors::EscrowErrorV2;

pub fn lock(
    env: Env,
    merchant: Address,
    arbiter: Address,
    usdc_token: Address,
    amount: i128,
    platform_fee: i128,
    beneficiaries: Vec<Beneficiary>,
    order_id: String,
    expires_at: u64,
) -> Result<(), EscrowErrorV2> {
    let key = DataKey::Escrow(order_id.clone());
    if env.storage().persistent().has(&key) {
        return Err(EscrowErrorV2::AlreadyExists);
    }

    merchant.require_auth();

    if amount <= 0 {
        return Err(EscrowErrorV2::InvalidAmount);
    }
    if platform_fee < 0 || platform_fee >= amount {
        return Err(EscrowErrorV2::InvalidFee);
    }
    if expires_at <= env.ledger().timestamp() {
        return Err(EscrowErrorV2::AlreadyExpired);
    }

    if beneficiaries.is_empty() {
        return Err(EscrowErrorV2::EmptyBeneficiaries);
    }
    if beneficiaries.len() > MAX_BENEFICIARIES {
        return Err(EscrowErrorV2::TooManyBeneficiaries);
    }

    let mut bps_sum: u32 = 0;
    for b in beneficiaries.iter() {
        if b.percent_bps == 0 || b.percent_bps > TOTAL_BPS {
            return Err(EscrowErrorV2::InvalidPercent);
        }
        bps_sum = bps_sum
            .checked_add(b.percent_bps)
            .ok_or(EscrowErrorV2::Overflow)?;
    }
    if bps_sum != TOTAL_BPS {
        return Err(EscrowErrorV2::BpsSumMismatch);
    }

    let net_amount = amount
        .checked_sub(platform_fee)
        .ok_or(EscrowErrorV2::Overflow)?;

    // Pull USDC from merchant
    let usdc = token::Client::new(&env, &usdc_token);
    usdc.transfer(&merchant, &env.current_contract_address(), &amount);

    let escrow = EscrowDataV2 {
        merchant,
        arbiter,
        usdc_token,
        amount,
        platform_fee,
        net_amount,
        beneficiaries,
        state: EscrowState::Locked,
        order_id,
        expires_at,
        created_at: env.ledger().timestamp(),
    };
    env.storage().persistent().set(&key, &escrow);
    env.storage().persistent().extend_ttl(&key, 2_592_000, 7_776_000);
    Ok(())
}
