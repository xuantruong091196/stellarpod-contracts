use soroban_sdk::{token, Env, String};
use crate::state::{DataKey, EscrowDataV2, EscrowState};
use crate::errors::EscrowErrorV2;

pub fn refund(env: Env, order_id: String) -> Result<(), EscrowErrorV2> {
    let key = DataKey::Escrow(order_id);
    let mut escrow: EscrowDataV2 = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(EscrowErrorV2::NotFound)?;
    if escrow.state != EscrowState::Locked {
        return Err(EscrowErrorV2::NotLocked);
    }
    if env.ledger().timestamp() < escrow.expires_at {
        return Err(EscrowErrorV2::NotExpired);
    }

    let usdc = token::Client::new(&env, &escrow.usdc_token);
    usdc.transfer(
        &env.current_contract_address(),
        &escrow.merchant,
        &escrow.amount,
    );

    escrow.state = EscrowState::Refunded;
    env.storage().persistent().set(&key, &escrow);
    Ok(())
}
