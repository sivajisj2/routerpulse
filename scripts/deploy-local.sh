#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
[[ -f .env ]] || { echo "Run ./scripts/setup.sh first." >&2; exit 1; }
set -a; source .env; set +a
OPERATOR_KEYPAIR="$PWD/.local/operator.json"
RELAYER_KEYPAIR="$PWD/.local/relayer.json"
[[ -f "$OPERATOR_KEYPAIR" && -f "$RELAYER_KEYPAIR" ]] || { echo "Local operator keypairs are missing; run scripts/setup.sh." >&2; exit 1; }
for rpc in "$SOURCE_RPC_URL" "$DESTINATION_RPC_URL"; do
  case "$rpc" in
    http://127.0.0.1:*|http://localhost:*) ;;
    *) echo "Refusing deployment outside localhost: $rpc" >&2; exit 1 ;;
  esac
  until curl --silent --fail -X POST "$rpc" -H 'content-type: application/json' \
    --data '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' >/dev/null; do
    sleep 1
  done
done
anchor build
PROGRAM_ID="$(anchor keys list | awk '/routerpulse:/{print $2; exit}')"
for rpc in "$SOURCE_RPC_URL" "$DESTINATION_RPC_URL"; do
  solana airdrop 10 "$(solana-keygen pubkey "$OPERATOR_KEYPAIR")" --url "$rpc" --keypair "$OPERATOR_KEYPAIR" >/dev/null
  ANCHOR_PROVIDER_URL="$rpc" ANCHOR_WALLET="$OPERATOR_KEYPAIR" anchor deploy --provider.cluster localnet
done
ROUTERPULSE_RELAYER_KEYPAIR="$RELAYER_KEYPAIR" ROUTERPULSE_CLUSTER_NAME=source ROUTERPULSE_REMOTE_CLUSTER_NAME=destination \
ROUTERPULSE_REMOTE_RPC_URL="$DESTINATION_RPC_URL" ANCHOR_PROVIDER_URL="$SOURCE_RPC_URL" ANCHOR_WALLET="$OPERATOR_KEYPAIR" \
  frontend/node_modules/.bin/tsx scripts/configure-local.ts
ROUTERPULSE_RELAYER_KEYPAIR="$RELAYER_KEYPAIR" ROUTERPULSE_CLUSTER_NAME=destination ROUTERPULSE_REMOTE_CLUSTER_NAME=source \
ROUTERPULSE_REMOTE_RPC_URL="$SOURCE_RPC_URL" ANCHOR_PROVIDER_URL="$DESTINATION_RPC_URL" ANCHOR_WALLET="$OPERATOR_KEYPAIR" \
  frontend/node_modules/.bin/tsx scripts/configure-local.ts
echo "Anchor program $PROGRAM_ID is deployed and configured on both local validators."

