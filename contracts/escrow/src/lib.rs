#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, symbol_short,
    Address, Env, String, token,
};

// ═══════════════════════════════════════════════
// STELLARPOD ESCROW CONTRACT — SOROBAN
//
// Luồng hoạt động:
// 1. Merchant gọi `lock()` → USDC chuyển vào contract
// 2. Provider hoàn thành đơn → Arbiter gọi `release()`
// 3. Nếu tranh chấp → `dispute()` → `resolve_dispute()`
// 4. Nếu quá hạn → `refund()` (ai cũng có thể gọi)
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
#[derive(Clone, Debug)]
pub struct EscrowData {
    pub merchant: Address,
    pub provider: Address,
    pub arbiter: Address,        // StellarPOD platform address
    pub usdc_token: Address,     // USDC token contract on Stellar
    pub amount: i128,            // Tổng USDC (bao gồm platform fee)
    pub platform_fee: i128,      // Phí StellarPOD (2-5%)
    pub provider_amount: i128,   // = amount - platform_fee
    pub state: EscrowState,
    pub order_id: String,        // Off-chain order reference
    pub expires_at: u64,         // Unix timestamp — auto-refund deadline
    pub created_at: u64,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum EscrowError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    InvalidAmount = 3,
    InvalidFee = 4,
    AlreadyExpired = 5,
    NotLocked = 6,
    NotDisputed = 7,
    NotAuthorized = 8,
    NotExpired = 9,
    InvalidPercent = 10,
    TransferFailed = 11,
}

// ─── STORAGE KEYS ─────────────────────────────

const ESCROW_KEY: symbol_short!("ESCROW") = symbol_short!("ESCROW");
const INIT_KEY: symbol_short!("INIT") = symbol_short!("INIT");

// ─── CONTRACT ─────────────────────────────────

#[contract]
pub struct StellarPodEscrow;

#[contractimpl]
impl StellarPodEscrow {
    /// Tạo escrow mới và khóa USDC từ merchant wallet vào contract.
    ///
    /// Merchant phải approve USDC transfer trước khi gọi hàm này.
    /// Arbiter là StellarPOD platform address — có quyền release/refund/resolve.
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
        // Prevent double initialization
        if env.storage().persistent().has(&INIT_KEY) {
            return Err(EscrowError::AlreadyInitialized);
        }

        // Merchant phải authorize transaction này
        merchant.require_auth();

        // Validate inputs
        if amount <= 0 {
            return Err(EscrowError::InvalidAmount);
        }
        if platform_fee < 0 || platform_fee >= amount {
            return Err(EscrowError::InvalidFee);
        }
        if expires_at <= env.ledger().timestamp() {
            return Err(EscrowError::AlreadyExpired);
        }

        let provider_amount = amount - platform_fee;

        // Transfer USDC: merchant → contract
        let usdc = token::Client::new(&env, &usdc_token);
        usdc.transfer(&merchant, &env.current_contract_address(), &amount);

        // Lưu escrow data
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

        env.storage().persistent().set(&ESCROW_KEY, &escrow);
        env.storage().persistent().set(&INIT_KEY, &true);

        // Extend TTL để storage không bị xóa (30 ngày min, 90 ngày max)
        env.storage().persistent().extend_ttl(&ESCROW_KEY, 2_592_000, 7_776_000);
        env.storage().persistent().extend_ttl(&INIT_KEY, 2_592_000, 7_776_000);

        Ok(())
    }

    /// Giải ngân cho provider sau khi đơn hàng delivered.
    ///
    /// Chỉ arbiter (StellarPOD) hoặc merchant có thể gọi.
    /// Flow: provider_amount → provider, platform_fee → arbiter (treasury)
    pub fn release(env: Env, caller: Address) -> Result<(), EscrowError> {
        caller.require_auth();

        let mut escrow = Self::get_escrow(&env)?;

        // Chỉ arbiter hoặc merchant được release
        if caller != escrow.arbiter && caller != escrow.merchant {
            return Err(EscrowError::NotAuthorized);
        }

        // Phải đang ở trạng thái Locked
        if escrow.state != EscrowState::Locked {
            return Err(EscrowError::NotLocked);
        }

        let usdc = token::Client::new(&env, &escrow.usdc_token);
        let contract = env.current_contract_address();

        // Chuyển tiền cho provider
        usdc.transfer(&contract, &escrow.provider, &escrow.provider_amount);

        // Chuyển phí cho arbiter (StellarPOD treasury)
        if escrow.platform_fee > 0 {
            usdc.transfer(&contract, &escrow.arbiter, &escrow.platform_fee);
        }

        escrow.state = EscrowState::Released;
        env.storage().persistent().set(&ESCROW_KEY, &escrow);

        Ok(())
    }

    /// Hoàn tiền toàn bộ cho merchant.
    ///
    /// Có thể gọi khi:
    /// - Escrow đã quá hạn (ai cũng gọi được)
    /// - Arbiter quyết định refund sau dispute
    pub fn refund(env: Env, caller: Address) -> Result<(), EscrowError> {
        caller.require_auth();

        let mut escrow = Self::get_escrow(&env)?;

        let is_expired = env.ledger().timestamp() > escrow.expires_at;

        // Nếu chưa expired, chỉ arbiter được refund
        if !is_expired && caller != escrow.arbiter {
            return Err(EscrowError::NotAuthorized);
        }

        // Phải đang Locked hoặc Disputed
        if escrow.state != EscrowState::Locked && escrow.state != EscrowState::Disputed {
            return Err(EscrowError::NotLocked);
        }

        let usdc = token::Client::new(&env, &escrow.usdc_token);
        let contract = env.current_contract_address();

        // Hoàn toàn bộ về merchant
        usdc.transfer(&contract, &escrow.merchant, &escrow.amount);

        escrow.state = EscrowState::Refunded;
        env.storage().persistent().set(&ESCROW_KEY, &escrow);

        Ok(())
    }

    /// Merchant hoặc provider raise dispute — freeze escrow.
    pub fn dispute(env: Env, caller: Address) -> Result<(), EscrowError> {
        caller.require_auth();

        let mut escrow = Self::get_escrow(&env)?;

        // Chỉ merchant hoặc provider được raise dispute
        if caller != escrow.merchant && caller != escrow.provider {
            return Err(EscrowError::NotAuthorized);
        }

        if escrow.state != EscrowState::Locked {
            return Err(EscrowError::NotLocked);
        }

        escrow.state = EscrowState::Disputed;
        env.storage().persistent().set(&ESCROW_KEY, &escrow);

        Ok(())
    }

    /// Arbiter giải quyết tranh chấp bằng cách chia tiền theo tỷ lệ.
    ///
    /// Ví dụ: provider làm được 70% → provider_percent = 70
    /// → 70% net amount → provider, 30% → merchant, platform_fee → arbiter
    pub fn resolve_dispute(
        env: Env,
        caller: Address,
        provider_percent: u32,
    ) -> Result<(), EscrowError> {
        caller.require_auth();

        let mut escrow = Self::get_escrow(&env)?;

        // Chỉ arbiter được resolve
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

        // Tính split (trừ platform fee luôn về arbiter)
        let net = escrow.amount - escrow.platform_fee;
        let to_provider = (net * provider_percent as i128) / 100;
        let to_merchant = net - to_provider;

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
        env.storage().persistent().set(&ESCROW_KEY, &escrow);

        Ok(())
    }

    // ─── READ-ONLY QUERIES ────────────────────

    /// Lấy trạng thái escrow hiện tại
    pub fn get_state(env: Env) -> Result<EscrowData, EscrowError> {
        Self::get_escrow(&env)
    }

    /// Kiểm tra escrow đã expired chưa
    pub fn is_expired(env: Env) -> Result<bool, EscrowError> {
        let escrow = Self::get_escrow(&env)?;
        Ok(env.ledger().timestamp() > escrow.expires_at)
    }

    /// Lấy thời gian còn lại trước khi expired (giây)
    pub fn time_remaining(env: Env) -> Result<u64, EscrowError> {
        let escrow = Self::get_escrow(&env)?;
        let now = env.ledger().timestamp();
        if now >= escrow.expires_at {
            Ok(0)
        } else {
            Ok(escrow.expires_at - now)
        }
    }

    // ─── INTERNAL HELPERS ─────────────────────

    fn get_escrow(env: &Env) -> Result<EscrowData, EscrowError> {
        env.storage()
            .persistent()
            .get(&ESCROW_KEY)
            .ok_or(EscrowError::NotInitialized)
    }
}

mod test;
