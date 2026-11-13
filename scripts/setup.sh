#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

for tool in docker solana solana-keygen anchor node npm python3; do
  command -v "$tool" >/dev/null || { echo "Missing required tool: $tool" >&2; exit 1; }
done
docker compose version >/dev/null
anchor --version
solana --version
node --version

if [[ ! -f .env ]]; then cp .env.example .env; fi
python3 - <<'PY'
from pathlib import Path
import secrets
p = Path('.env')
s = p.read_text()
s = s.replace('POSTGRES_PASSWORD=generate-local-only', f'POSTGRES_PASSWORD={secrets.token_hex(32)}')
s = s.replace('API_KEY=generate-local-only', f'API_KEY={secrets.token_hex(32)}')
p.write_text(s)
p.chmod(0o600)
PY

mkdir -p .local target/deploy
for name in operator relayer; do
  key=".local/${name}.json"
  if [[ ! -f "$key" ]]; then
    solana-keygen new --no-bip39-passphrase --silent --outfile "$key" >/dev/null
    chmod 600 "$key"
  fi
done
program_key="target/deploy/routerpulse-keypair.json"
if [[ ! -f "$program_key" ]]; then
  solana-keygen new --no-bip39-passphrase --silent --outfile "$program_key" >/dev/null
  chmod 600 "$program_key"
fi

# Anchor stores the public program ID in source and the private deploy key in target/.
anchor keys sync
echo "Local credentials are stored under .local/ and target/; both paths are ignored by Git."
echo "Review the generated public program ID changes in Anchor.toml and programs/routerpulse/src/lib.rs."
