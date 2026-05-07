#!/usr/bin/env bash
#
# Deploy escrow_v2 Soroban contract to a Stellar network.
#
# Usage:
#   DEPLOYER_SECRET=S... NETWORK=testnet bash scripts/deploy_escrow_v2.sh
#
# Required env vars:
#   DEPLOYER_SECRET — Stellar secret key (S...) of the deploying account
#                     (must be funded with at least ~10 XLM)
#   NETWORK         — testnet | public (default: testnet)
#
# Output:
#   Prints the deployed contract ID to stdout. Save this into the API config
#   as STELLAR_ESCROW_V2_CONTRACT_ID.
#
# Prerequisites:
#   - rustup target add wasm32-unknown-unknown
#   - soroban CLI: `cargo install --locked stellar-cli` (or `soroban-cli`)

set -euo pipefail

NETWORK="${NETWORK:-testnet}"
if [[ -z "${DEPLOYER_SECRET:-}" ]]; then
  echo "ERROR: DEPLOYER_SECRET env var required (Stellar secret key starting with S)" >&2
  exit 1
fi

cd "$(dirname "$0")/.."

echo "[1/3] Building escrow_v2 wasm…"
cargo build -p stellarpod-escrow-v2 --target wasm32-unknown-unknown --release

WASM_PATH="target/wasm32-unknown-unknown/release/stellarpod_escrow_v2.wasm"
if [[ ! -f "$WASM_PATH" ]]; then
  echo "ERROR: built wasm not found at $WASM_PATH" >&2
  exit 1
fi

# Try `stellar` first (newer CLI), fall back to `soroban`
if command -v stellar &> /dev/null; then
  CLI="stellar"
elif command -v soroban &> /dev/null; then
  CLI="soroban"
else
  echo "ERROR: neither 'stellar' nor 'soroban' CLI found in PATH" >&2
  echo "       install with: cargo install --locked stellar-cli" >&2
  exit 1
fi

echo "[2/3] Deploying with $CLI to $NETWORK…"
CONTRACT_ID=$(
  "$CLI" contract deploy \
    --wasm "$WASM_PATH" \
    --source-account "$DEPLOYER_SECRET" \
    --network "$NETWORK"
)

echo "[3/3] Deployed."
echo ""
echo "Contract ID: $CONTRACT_ID"
echo ""
echo "Add this to your API .env:"
echo "  STELLAR_ESCROW_V2_CONTRACT_ID=$CONTRACT_ID"
echo "  STELLAR_ESCROW_V2_NETWORK=$NETWORK"
