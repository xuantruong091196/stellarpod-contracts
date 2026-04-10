#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror,
    Address, Env, String, token,
};

// ═══════════════════════════════════════════════
// STELO ESCROW CONTRACT — SOROBAN (Multi-Escrow)
//
// A single contract instance handles N escrows, keyed by order_id.
//
// Flow per escrow:
// 1. Merchant calls `lock(order_id, ...)` → USDC transferred into contract
// 2. Provider fulfils → Arbiter calls `release(order_id)`
// 3. Dispute → `dispute(order_id)` → `resolve_dispute(order_id, %)`
// 4. Expired → `refund(order_id)` (anyone can call)
// ═══════════════════════════════════════════════

// ─── DATA TYPES ───────────────────────────────

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum EscrowState {
    Locked,
    Released,
    Refunded,
    Disputed,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct EscrowData {
    pub merchant: Address,
    pub provider: Address,
    pub arbiter: Address,
    pub usdc_token: Address,
    pub amount: i128,
    pub platform_fee: i128,
    pub provider_amount: i128,
    pub state: EscrowState,
    pub order_id: String,
    pub expires_at: u64,
    pub created_at: u64,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum EscrowError {
    AlreadyExists = 1,
    NotFound = 2,
    InvalidAmount = 3,
    InvalidFee = 4,
    AlreadyExpired = 5,
    NotLocked = 6,
    NotDisputed = 7,
    NotAuthorized = 8,
    NotExpired = 9,
    InvalidPercent = 10,
    Overflow = 11,
}

// ─── STORAGE ─────────────────────────────────

/// Each escrow is keyed by its order_id via this enum.
#[contracttype]
enum DataKey {
    Escrow(String),
}

// ─── CONTRACT ─────────────────────────────────

#[contract]
pub struct SteloEscrow;

#[contractimpl]
impl SteloEscrow {
    /// Create a new escrow and lock USDC from merchant into the contract.
    /// Each order_id can only be used once.
    pub fn lock(
        env: Env,
        merchant: Address,
        provider: Address,
        arbiter: Address,
        usdc_token: Address,
        amount: i128,
        platform_fee: i128,
        order_id: String,
        expires_at: u64,
    ) -> Result<(), EscrowError> {
        let key = DataKey::Escrow(order_id.clone());

        // Prevent duplicate escrow for same order
        if env.storage().persistent().has(&key) {
            return Err(EscrowError::AlreadyExists);
        }

        merchant.require_auth();

        if amount <= 0 {
            return Err(EscrowError::InvalidAmount);
        }
        if platform_fee < 0 || platform_fee >= amount {
            return Err(EscrowError::InvalidFee);
        }
        if expires_at <= env.ledger().timestamp() {
            return Err(EscrowError::AlreadyExpired);
        }

        let provider_amount = amount
            .checked_sub(platform_fee)
            .ok_or(EscrowError::Overflow)?;

        // Transfer USDC: merchant → contract
        let usdc = token::Client::new(&env, &usdc_token);
        usdc.transfer(&merchant, &env.current_contract_address(), &amount);

        let escrow = EscrowData {
            merchant,
            provider,
            arbiter,
            usdc_token,
            amount,
            platform_fee,
            provider_amount,
            state: EscrowState::Locked,
            order_id,
            expires_at,
            created_at: env.ledger().timestamp(),
        };

        env.storage().persistent().set(&key, &escrow);
        env.storage().persistent().extend_ttl(&key, 2_592_000, 7_776_000);

        Ok(())
    }

    /// Release escrow funds to provider + platform fee to arbiter.
    /// Only arbiter or merchant can call.
    pub fn release(env: Env, caller: Address, order_id: String) -> Result<(), EscrowError> {
        caller.require_auth();
        let key = DataKey::Escrow(order_id);
        let mut escrow = Self::get_escrow(&env, &key)?;

        if caller != escrow.arbiter && caller != escrow.merchant {
            return Err(EscrowError::NotAuthorized);
        }
        if escrow.state != EscrowState::Locked {
            return Err(EscrowError::NotLocked);
        }

        let usdc = token::Client::new(&env, &escrow.usdc_token);
        let contract = env.current_contract_address();

        usdc.transfer(&contract, &escrow.provider, &escrow.provider_amount);
        if escrow.platform_fee > 0 {
            usdc.transfer(&contract, &escrow.arbiter, &escrow.platform_fee);
        }

        escrow.state = EscrowState::Released;
        env.storage().persistent().set(&key, &escrow);
        Ok(())
    }

    /// Refund full amount to merchant.
    /// Anyone can call after expiry. Only arbiter before expiry.
    pub fn refund(env: Env, caller: Address, order_id: String) -> Result<(), EscrowError> {
        caller.require_auth();
        let key = DataKey::Escrow(order_id);
        let mut escrow = Self::get_escrow(&env, &key)?;

        let is_expired = env.ledger().timestamp() > escrow.expires_at;
        if !is_expired && caller != escrow.arbiter {
            return Err(EscrowError::NotAuthorized);
        }
        if escrow.state != EscrowState::Locked && escrow.state != EscrowState::Disputed {
            return Err(EscrowError::NotLocked);
        }

        let usdc = token::Client::new(&env, &escrow.usdc_token);
        usdc.transfer(&env.current_contract_address(), &escrow.merchant, &escrow.amount);

        escrow.state = EscrowState::Refunded;
        env.storage().persistent().set(&key, &escrow);
        Ok(())
    }

    /// Merchant or provider raises a dispute — freezes the escrow.
    pub fn dispute(env: Env, caller: Address, order_id: String) -> Result<(), EscrowError> {
        caller.require_auth();
        let key = DataKey::Escrow(order_id);
        let mut escrow = Self::get_escrow(&env, &key)?;

        if caller != escrow.merchant && caller != escrow.provider {
            return Err(EscrowError::NotAuthorized);
        }
        if escrow.state != EscrowState::Locked {
            return Err(EscrowError::NotLocked);
        }

        escrow.state = EscrowState::Disputed;
        env.storage().persistent().set(&key, &escrow);
        Ok(())
    }

    /// Arbiter resolves dispute by splitting funds between provider and merchant.
    /// provider_percent: 0..=100 — percentage of net (amount - fee) going to provider.
    pub fn resolve_dispute(
        env: Env,
        caller: Address,
        order_id: String,
        provider_percent: u32,
    ) -> Result<(), EscrowError> {
        caller.require_auth();
        let key = DataKey::Escrow(order_id);
        let mut escrow = Self::get_escrow(&env, &key)?;

        if caller != escrow.arbiter {
            return Err(EscrowError::NotAuthorized);
        }
        if escrow.state != EscrowState::Disputed {
            return Err(EscrowError::NotDisputed);
        }
        if provider_percent > 100 {
            return Err(EscrowError::InvalidPercent);
        }

        let usdc = token::Client::new(&env, &escrow.usdc_token);
        let contract = env.current_contract_address();

        let net = escrow.amount
            .checked_sub(escrow.platform_fee)
            .ok_or(EscrowError::Overflow)?;
        let to_provider = net
            .checked_mul(provider_percent as i128)
            .ok_or(EscrowError::Overflow)?
            / 100;
        let to_merchant = net
            .checked_sub(to_provider)
            .ok_or(EscrowError::Overflow)?;

        if to_provider > 0 {
            usdc.transfer(&contract, &escrow.provider, &to_provider);
        }
        if to_merchant > 0 {
            usdc.transfer(&contract, &escrow.merchant, &to_merchant);
        }
        if escrow.platform_fee > 0 {
            usdc.transfer(&contract, &escrow.arbiter, &escrow.platform_fee);
        }

        escrow.state = EscrowState::Released;
        env.storage().persistent().set(&key, &escrow);
        Ok(())
    }

    // ─── READ-ONLY QUERIES ────────────────────

    pub fn get_state(env: Env, order_id: String) -> Result<EscrowData, EscrowError> {
        Self::get_escrow(&env, &DataKey::Escrow(order_id))
    }

    pub fn is_expired(env: Env, order_id: String) -> Result<bool, EscrowError> {
        let escrow = Self::get_escrow(&env, &DataKey::Escrow(order_id))?;
        Ok(env.ledger().timestamp() > escrow.expires_at)
    }

    pub fn time_remaining(env: Env, order_id: String) -> Result<u64, EscrowError> {
        let escrow = Self::get_escrow(&env, &DataKey::Escrow(order_id))?;
        let now = env.ledger().timestamp();
        if now >= escrow.expires_at { Ok(0) } else { Ok(escrow.expires_at - now) }
    }

    // ─── INTERNAL ─────────────────────────────

    fn get_escrow(env: &Env, key: &DataKey) -> Result<EscrowData, EscrowError> {
        env.storage()
            .persistent()
            .get(key)
            .ok_or(EscrowError::NotFound)
    }
}

mod test;
