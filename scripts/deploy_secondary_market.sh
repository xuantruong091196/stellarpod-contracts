#!/usr/bin/env bash
#
# Deploy stelo_nft + stelo_marketplace contracts to a Stellar network.
#
# Usage:
#   DEPLOYER_SECRET=S... PLATFORM_ADDRESS=G... USDC_CONTRACT=C... \
#     bash scripts/deploy_secondary_market.sh
#
# Required env vars:
#   DEPLOYER_SECRET   — Stellar secret (S...) of deploying account; needs ~50 XLM
#   PLATFORM_ADDRESS  — Stellar address (G...) that receives platform fees
#   USDC_CONTRACT     — Stellar Asset Contract (C...) for USDC on the target network
#
# Optional:
#   NETWORK             default: testnet
#   PLATFORM_FEE_BPS    default: 250 (2.5%)
#
# Output:
#   Prints both contract IDs. Save into the API config:
#     STELO_NFT_WASM_HASH=...   (used to deploy per-store contracts)
#     STELLAR_MARKETPLACE_CONTRACT_ID=...

set -euo pipefail

NETWORK="${NETWORK:-testnet}"
PLATFORM_FEE_BPS="${PLATFORM_FEE_BPS:-250}"

for required in DEPLOYER_SECRET PLATFORM_ADDRESS USDC_CONTRACT; do
  if [[ -z "${!required:-}" ]]; then
    echo "ERROR: $required env var required" >&2
    exit 1
  fi
done

cd "$(dirname "$0")/.."

if command -v stellar &> /dev/null; then
  CLI="stellar"
elif command -v soroban &> /dev/null; then
  CLI="soroban"
else
  echo "ERROR: neither 'stellar' nor 'soroban' CLI found in PATH" >&2
  echo "       install with: cargo install --locked stellar-cli" >&2
  exit 1
fi

echo "[1/4] Building stelo_nft + stelo_marketplace wasms…"
cargo build \
  -p stellarpod-stelo-nft \
  -p stellarpod-stelo-marketplace \
  --target wasm32-unknown-unknown --release

NFT_WASM="target/wasm32-unknown-unknown/release/stellarpod_stelo_nft.wasm"
MARKET_WASM="target/wasm32-unknown-unknown/release/stellarpod_stelo_marketplace.wasm"

for wasm in "$NFT_WASM" "$MARKET_WASM"; do
  if [[ ! -f "$wasm" ]]; then
    echo "ERROR: built wasm not found at $wasm" >&2
    exit 1
  fi
done

echo "[2/4] Installing stelo_nft wasm (gets a hash for per-store deploys)…"
NFT_WASM_HASH=$("$CLI" contract install \
  --wasm "$NFT_WASM" \
  --source-account "$DEPLOYER_SECRET" \
  --network "$NETWORK")

echo "      stelo_nft wasm hash: $NFT_WASM_HASH"

echo "[3/4] Deploying singleton stelo_marketplace contract…"
MARKETPLACE_ID=$("$CLI" contract deploy \
  --wasm "$MARKET_WASM" \
  --source-account "$DEPLOYER_SECRET" \
  --network "$NETWORK")

echo "      stelo_marketplace contract ID: $MARKETPLACE_ID"

echo "[4/4] Initializing marketplace (admin=deployer, USDC=$USDC_CONTRACT, platform=$PLATFORM_ADDRESS, fee=${PLATFORM_FEE_BPS} bps)…"

# Derive deployer public key from secret
DEPLOYER_PUBKEY=$(
  "$CLI" keys public-key --name "$DEPLOYER_SECRET" 2>/dev/null ||
  "$CLI" address public --secret-key "$DEPLOYER_SECRET" 2>/dev/null ||
  true
)

if [[ -z "${DEPLOYER_PUBKEY:-}" ]]; then
  echo "WARNING: could not derive public key automatically." >&2
  echo "         Set ADMIN_ADDRESS env var and re-run the init step manually:" >&2
  echo "         $CLI contract invoke --id $MARKETPLACE_ID \\" >&2
  echo "           --source-account \$DEPLOYER_SECRET --network $NETWORK \\" >&2
  echo "           -- init --admin <G...> --usdc_token $USDC_CONTRACT \\" >&2
  echo "           --platform_address $PLATFORM_ADDRESS --platform_fee_bps $PLATFORM_FEE_BPS" >&2
else
  "$CLI" contract invoke \
    --id "$MARKETPLACE_ID" \
    --source-account "$DEPLOYER_SECRET" \
    --network "$NETWORK" \
    -- init \
    --admin "$DEPLOYER_PUBKEY" \
    --usdc_token "$USDC_CONTRACT" \
    --platform_address "$PLATFORM_ADDRESS" \
    --platform_fee_bps "$PLATFORM_FEE_BPS"
  echo "      Marketplace initialized."
fi

echo ""
echo "Add to your API .env:"
echo "  STELO_NFT_WASM_HASH=$NFT_WASM_HASH"
echo "  STELLAR_MARKETPLACE_CONTRACT_ID=$MARKETPLACE_ID"
