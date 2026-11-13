#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
[[ -f .env ]] || { echo "Run ./scripts/setup.sh first." >&2; exit 1; }
set -a; source .env; set +a
curl --fail --silent http://localhost:8080/livez >/dev/null
unauth="$(curl --silent --output /dev/null --write-out '%{http_code}' http://localhost:8080/api/v1/clusters)"
[[ "$unauth" == "401" ]] || { echo "Expected unauthenticated operator route to return 401; got $unauth" >&2; exit 1; }
curl --fail --silent -H "x-api-key: $API_KEY" http://localhost:8080/api/v1/clusters | python3 -c 'import json,sys; x=json.load(sys.stdin); assert len(x)==2 and all(c["healthy"] for c in x), x'
echo "Smoke checks passed: API liveness, API key boundary, and both Solana RPC endpoints."

