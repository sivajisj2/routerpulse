#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
(cd contracts && forge fmt --check && forge build)
(cd backend && cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo build)
npm install --prefix frontend
npm run build --prefix frontend

