# Security model

## Trust assumptions

- Destination execution currently trusts one configured relayer signer. This is a permissioned attestation model, not a trustless bridge.
- RPC providers may be stale, unavailable, or inconsistent. Future workers must retain independent observations and reconcile signatures, slots, blockhashes, and commitment.
- The authority can configure remote programs, relayers, and pause state. Production authority rotation and emergency control need a reviewed governance model.
- PostgreSQL and Redis are operational components, not cryptographic evidence.
- Local validator faucet keys and generated operator keys are development-only.

## Controls implemented

- PDA-scoped configuration, remote allowlist, sender sequence, message, and processed receipt.
- Bounded payload and expiry checks; canonical domain-separated message IDs.
- Relayer signer gate, trusted source domain/program checks, pause controls, and replay prevention.
- API key comparison uses constant-time equality; secrets are generated locally and ignored by Git.
- Non-root API and Nginx containers, health checks, restrictive browser policy, and bounded API request limits.

The Anchor destination instruction currently records a receipt only. It does not move SOL/SPL tokens or invoke arbitrary receiver programs. No production scanner, transaction worker, key-management system, or independent audit is included. Do not use this project to bridge real assets.

## Before broader deployment

Complete Anchor adversarial and invariant coverage, review authority and signer rotation, add a durable scanner/worker with reorg and blockhash-expiry reconciliation, harden operator identity and secret management, verify migrations and backup restoration, and commission an independent program and infrastructure review. Keep development, staging, and production cluster identities and keys isolated.
