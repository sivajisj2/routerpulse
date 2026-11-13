# Architecture

## Current runtime

Docker Compose starts two Foundry Anvil nodes (31337 and 31338), PostgreSQL, Redis, the Rust/Axum API, and a Vite-built React console served by Nginx. The API currently holds message registrations in process memory. PostgreSQL schema and Redis are provisioned but not consumed by the API yet.

The source contract assigns a per-sender nonce and emits a message ID binding the source chain, source router, sender, nonce, destination chain, receiver, value, payload hash, and deadline. The destination contract accepts the message only from a configured relayer signature and trusted source router, then marks it processed before calling the receiver.

## Target relayer boundary

The next backend step should isolate chain reads/writes behind an Alloy adapter. Scanner observations should be immutable and include source block and transaction hashes. A worker should re-check the source block hash after its confirmation threshold, simulate destination execution, and persist the destination transaction hash before receipt polling. Unknown receipt status must be reconciled; it must never cause an automatic duplicate send.

## State transitions

```text
created -> source_submitted -> source_confirming -> source_confirmed -> queued
queued -> verifying -> ready_to_submit -> destination_submitted -> destination_confirming -> completed
source_confirming -> source_reorged -> queued
destination_submitted -> retry_scheduled (only after classified safe failure)
verifying -> rejected -> dead_lettered
```

Every future transition should be validated in domain code and committed with an audit row in one SQL transaction, using a version column for optimistic concurrency.

