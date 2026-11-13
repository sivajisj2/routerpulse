#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
command -v forge >/dev/null || { echo "Foundry (forge) is required for local contract deployment." >&2; exit 1; }
command -v cast >/dev/null || { echo "Foundry (cast) is required for local contract deployment." >&2; exit 1; }
SOURCE_RPC_URL="${SOURCE_RPC_URL:-http://127.0.0.1:8545}"
DESTINATION_RPC_URL="${DESTINATION_RPC_URL:-http://127.0.0.1:8546}"
DEPLOYER_PRIVATE_KEY="${ANVIL_DEPLOYER_KEY:-0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80}"
RELAYER_PRIVATE_KEY="${ANVIL_RELAYER_KEY:-0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d}"
OWNER_ADDRESS="$(cast wallet address --private-key "$DEPLOYER_PRIVATE_KEY")"
RELAYER_ADDRESS="$(cast wallet address --private-key "$RELAYER_PRIVATE_KEY")"
mkdir -p contracts/deployments
src_output="$(cd contracts && forge create src/SourceRouter.sol:SourceRouter --rpc-url "$SOURCE_RPC_URL" --private-key "$DEPLOYER_PRIVATE_KEY" --broadcast --constructor-args "$OWNER_ADDRESS")"
source_router="$(printf '%s\n' "$src_output" | sed -n 's/^Deployed to: //p' | tail -n1)"
[[ -n "$source_router" ]] || { printf '%s\n' "$src_output"; echo "Could not parse SourceRouter deployment address." >&2; exit 1; }
cast send "$source_router" "setDestination(uint256,bool)" 31338 true --rpc-url "$SOURCE_RPC_URL" --private-key "$DEPLOYER_PRIVATE_KEY" >/dev/null
dst_output="$(cd contracts && forge create src/DestinationRouter.sol:DestinationRouter --rpc-url "$DESTINATION_RPC_URL" --private-key "$DEPLOYER_PRIVATE_KEY" --broadcast --constructor-args "$OWNER_ADDRESS" "$RELAYER_ADDRESS")"
destination_router="$(printf '%s\n' "$dst_output" | sed -n 's/^Deployed to: //p' | tail -n1)"
[[ -n "$destination_router" ]] || { printf '%s\n' "$dst_output"; echo "Could not parse DestinationRouter deployment address." >&2; exit 1; }
cast send "$destination_router" "setTrustedSource(uint256,address,bool)" 31337 "$source_router" true --rpc-url "$DESTINATION_RPC_URL" --private-key "$DEPLOYER_PRIVATE_KEY" >/dev/null
cat > contracts/deployments/local.json <<EOF
{
  "environment": "local-anvil",
  "source_chain_id": 31337,
  "destination_chain_id": 31338,
  "source_router": "$source_router",
  "destination_router": "$destination_router",
  "owner": "$OWNER_ADDRESS",
  "relayer": "$RELAYER_ADDRESS"
}
EOF
echo "Deployed local routers. Manifest: contracts/deployments/local.json"
echo "Local Anvil development keys are public and must never be used on real networks."

