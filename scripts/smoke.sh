#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ -f .env ]]; then set -a; source .env; set +a; fi
API_KEY="${API_KEY:-local-demo-key-change-me}"
curl --fail --silent http://localhost:8080/livez >/dev/null
code="$(curl --silent --output /dev/null --write-out '%{http_code}' http://localhost:8080/api/v1/chains)"
[[ "$code" == "401" ]] || { echo "Expected unauthenticated operator call to return 401; got $code"; exit 1; }
curl --fail --silent -H "x-api-key: $API_KEY" http://localhost:8080/api/v1/chains >/dev/null
echo "Smoke checks passed: health and API auth boundary."

