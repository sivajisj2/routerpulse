# Architecture

## Runtime

Docker Compose runs two independent `solana-test-validator` processes, PostgreSQL, Redis, the Rust/Axum API, and a React operator console served by unprivileged Nginx. The API reads PostgreSQL and probes Solana RPC. The Redis service is provisioned for future worker leases but is not consumed yet.

## Message flow

1. A sender invokes `send_message` on the source Anchor program. The program validates the configured remote and bounded payload, advances that sender's application sequence, stores a message PDA, and emits `MessageSent`.
2. A future scanner observes the log and records transaction signature, slot, blockhash, and commitment observations.
3. A future relayer waits for configured source commitment, reconciles signature and slot evidence, and submits an attested message to the destination program.
4. The destination program checks the configured relayer, source domain and program, expiry, and canonical message ID. A processed-message PDA provides replay protection. The current instruction records a receipt event and performs no arbitrary CPI or asset transfer.

The relayer is permissioned. Its signature is an operator attestation, not a light-client proof of source state. Cluster identity is derived from the Solana genesis hash and configured domain label; a program ID is separately bound to the remote configuration.

## Durable backend boundary

PostgreSQL is the source of truth for future observations, attempts, state transitions, idempotency keys, audit events, and outbox records. Redis may coordinate worker leases, but must not become the only record of message state. Unknown submission outcomes must reconcile the original Solana signature and blockhash validity before retrying; never blindly resubmit after a timeout.

## State transitions

```text
source_observed -> source_confirming -> source_finalized -> queued
queued -> verifying -> ready_to_execute -> destination_submitted -> destination_confirming -> completed
source_confirming -> source_reorged -> queued
 destination_submitted -> retry_scheduled (only after classified safe failure)
verifying -> rejected -> dead_lettered
```

Every transition should be checked in domain logic and committed with its audit row in one SQL transaction, with optimistic concurrency control.
