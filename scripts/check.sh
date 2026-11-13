#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
anchor build
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
(cd frontend && npm ci && npm run build)

