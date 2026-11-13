#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ ! -f .env ]]; then cp .env.example .env; echo "Created .env with local demo settings."; fi
echo "Prerequisites: Docker Compose v2. Source checks require Foundry, Rust, and Node.js 22+."

