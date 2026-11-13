# Threat model

## Trust assumptions

- The first verifier is a single permissioned relayer signer. This is a trusted bridge model.
- RPC endpoints are untrusted observations and may disagree or lag.
- Local Anvil keys are public test keys.
- The owner can configure trusted routers, relayers, and pause state.
- PostgreSQL/Redis are operational dependencies, not cryptographic evidence.

## Contract controls present

- Source sender and destination allowlists, monotonic per-sender nonce, bounded payload, and deadline.
- Destination trusted source mapping, EIP-712 destination domain, signer recovery, expiry, replay map, and pause switch.
- Effects-before-interactions for the replay flag before receiver callback.

## Review needed before broader use

- Independently verify canonical message IDs on destination; currently signer approval is the main authorization for the supplied `messageId`.
- Decide whether callback failure should revert the entire message, and whether native value forwarding needs an explicit source escrow model.
- Add signature malleability checks and a reviewed ECDSA implementation.
- Add delayed/two-step owner and signer rotation; separate emergency pause authority.
- Add fuzz, invariant, adversarial callback, wrong-domain, wrong-router, replay, expiry, and payload-bound tests.
- Protect the API key with a stronger operator session model and rate limiting before any shared deployment.

Do not use this code to bridge real funds. It is not audited or production-ready.

