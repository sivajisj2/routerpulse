#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
./scripts/setup.sh
docker compose up --build --wait -d
echo "RouterPulse UI: http://localhost:3000"
echo "API health:    http://localhost:8080/readyz"
echo "Source RPC:    http://localhost:8545"
echo "Destination:   http://localhost:8546"
./scripts/deploy-local.sh

