# Contributing

1. Create a focused branch for each change.
2. Preserve the Solana-first architecture and document changes to PDAs, account constraints, state transitions, or trust assumptions.
3. Add appropriate validation for changes; run `./scripts/check.sh` when the local Rust, Anchor, Solana, and Node toolchains are available.
4. Never commit credentials, local keypairs, or generated deployment state.
5. Include migration, operational, and security impacts in the pull request description.

Changes involving message verification, replay protection, signer access, retries, or source reorg handling require a concise threat-model review.
