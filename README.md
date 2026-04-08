# StellarPOD Smart Contracts

Soroban smart contracts cho StellarPOD escrow system.

## Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add Soroban target
rustup target add wasm32-unknown-unknown

# Install Soroban CLI
cargo install --locked stellar-cli
```

## Build

```bash
stellar contract build
```

Output: `target/wasm32-unknown-unknown/release/stellarpod_escrow.wasm`

## Test

```bash
cargo test
```

## Deploy (Testnet)

```bash
# Generate identity
stellar keys generate --global deployer --network testnet

# Fund account
stellar keys fund deployer --network testnet

# Deploy contract
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/stellarpod_escrow.wasm \
  --source deployer \
  --network testnet

# Returns: CONTRACT_ID
```

## Invoke (Testnet)

```bash
# Lock escrow
stellar contract invoke \
  --id CONTRACT_ID \
  --source merchant \
  --network testnet \
  -- lock \
  --merchant MERCHANT_ADDRESS \
  --provider PROVIDER_ADDRESS \
  --arbiter ARBITER_ADDRESS \
  --usdc_token USDC_TOKEN_ADDRESS \
  --amount 1000000000 \
  --platform_fee 50000000 \
  --order_id "order_001" \
  --expires_at 1735689600

# Check state
stellar contract invoke \
  --id CONTRACT_ID \
  --network testnet \
  -- get_state
```
