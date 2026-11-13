#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
./scripts/setup.sh
docker compose up --build --wait -d
./scripts/deploy-local.sh
echo "RouterPulse console: http://localhost:3000"
echo "API readiness:       http://localhost:8080/readyz"
echo "Source RPC:          http://localhost:8899"
echo "Destination RPC:     http://localhost:8900"

