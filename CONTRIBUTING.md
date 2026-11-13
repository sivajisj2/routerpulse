# Contributing

1. Create a focused branch for each change.
2. Keep contract, backend, and UI behavior covered by tests.
3. Run `./scripts/check.sh` before opening a pull request.
4. Include migration, threat-model, and operational impacts when relevant.
5. Never commit secrets or real private keys. Local Anvil keys are public test keys.

For message verification, replay protection, signer access, retries, or reorg logic,
include a short security review in the pull request description.

