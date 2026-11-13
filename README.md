# RouterPulse

**Solana cross-cluster message routing and relayer operations** using Rust, Anchor 0.30, Solana RPC, PostgreSQL, Redis infrastructure, and a React operator console.

> Portfolio and local-development software. The relayer is permissioned. RouterPulse is not a trustless bridge, has not been independently audited, does not custody tokens, and is not approved for mainnet use.

## Design

RouterPulse sends an application message from one Solana cluster, waits for source finality, and lets a configured relayer attest to the source observation on a second cluster. The destination program validates the cluster/program domain, recomputes the canonical message ID, and creates a replay-protected processed-message PDA.

Two local Solana test validators stand in for independent clusters. Each cluster domain is SHA-256 over `ROUTERPULSE_CLUSTER_V1 || genesis_hash_bytes || ":" || configured_label`. The label keeps local source and destination identities distinct even when validator genesis parameters happen to match.

### Anchor program

The Anchor program exposes `initialize`, `register_remote`, `update_remote`, `set_relayer`, `set_paused`, `send_message`, and `execute_attested_message`.

| PDA | Seeds | Purpose |
|---|---|---|
| `RouterConfig` | `config` | Authority, relayer, local cluster domain, pause state |
| `RemoteConfig` | `remote`, domain hash, remote program | Trusted remote cluster/program and enable flag |
| `SenderSequence` | `sender`, sender pubkey | Application-level per-sender sequence |
| `MessageAccount` | `message`, sender pubkey, sequence LE | Source message and bounded payload |
| `ProcessedMessage` | `processed`, canonical message ID | Destination replay protection |

The sequence PDA is application-level idempotency and ordering for a sender. Solana transaction identity and reconciliation use signatures, recent blockhashes, slots, and commitment levels.

The destination instruction records an execution receipt and emits an event. It does not invoke arbitrary receiver programs or transfer tokens. Add application-specific CPI and asset custody only after a separate threat model and review.

## Current implementation status

- Anchor program: PDA configuration, remote allowlists, sender sequence, bounded message payload, expiry validation, canonical domain-separated IDs, relayer-gated destination receipt, and duplicate-execution prevention.
- Rust/Axum API: API-key boundary with constant-time comparison, PostgreSQL migrations, message/route queries, request IDs, graceful shutdown, and Solana RPC health checks.
- Database: clusters, routes, messages, source observations, attempts, audit transitions, idempotency keys, outbox, and relayer heartbeats.
- Console: Solana cluster slots, signatures, message status, source finality, route health, and review timeline. Demo rows are illustrative until a scanner feeds the database.
- Local infrastructure: source/destination `solana-test-validator`, PostgreSQL, Redis, API, and Nginx-served UI.

The event scanner, durable worker lease loop, automatic source-reorg reconciliation, destination transaction submission, and live database-to-console feed are still roadmap work. The database schema is ready for those services, but a provisioned Redis container is not yet used by the API.

## Local quick start

Prerequisites: Docker Compose v2, Solana CLI 1.18.26, Anchor 0.30.1, Rust, Node.js 24, npm, and Python 3.

```bash
./scripts/setup.sh
./scripts/up.sh
./scripts/smoke.sh
```

`setup.sh` creates a private `.env`, random local operator and relayer keypairs under `.local/`, and a local Anchor program keypair under `target/`. Those paths are ignored by Git. Anchor sync updates the public program ID in `Anchor.toml` and `programs/routerpulse/src/lib.rs`; review and commit your own program ID only when you intend to use it in your fork.

`up.sh` starts both test validators and supporting services, builds and deploys the Anchor program to localhost RPC endpoints only, then initializes each cluster config and registers the trusted remote program. The deployment script refuses non-local RPC URLs.

Open:

- Console: <http://localhost:3000>
- API readiness: <http://localhost:8080/readyz>
- Source validator RPC: <http://localhost:8899>
- Destination validator RPC: <http://localhost:8900>
- PostgreSQL: `localhost:5433`
- Redis: `localhost:6379`

The API key is generated into `.env`; enter it in the console's key control to connect. Stop the stack with `./scripts/down.sh`. Delete local database state with `docker compose down -v`.

## API

Operator endpoints require `x-api-key`. Health endpoints are public.

| Method | Endpoint | Purpose |
|---|---|---|
| GET | `/livez` | API process liveness |
| GET | `/readyz` | Database readiness |
| GET | `/api/v1/clusters` | Genesis/domain identity, current slot, and RPC health for both local clusters |
| GET | `/api/v1/routes` | Configured off-chain route records |
| GET | `/api/v1/messages` | Cursor-filterable message records (`status`, `limit`, `before`) |
| GET | `/api/v1/messages/{id}` | One message by 32-byte hex ID |
| GET | `/api/v1/relayer/health` | Worker readiness; scanner is not enabled yet |

Example:

```bash
curl -H "x-api-key: $(sed -n 's/^API_KEY=//p' .env)" http://localhost:8080/api/v1/clusters
```

## Development

```bash
./scripts/check.sh
```

This formats/lints/builds the Anchor/Rust workspace and creates a production frontend bundle. Local Solana validators and end-to-end relayer behavior require separate integration coverage before they can be claimed as verified.

## Security model and boundaries

- A destination execution is authorized by a configured relayer signer. This is an operator attestation, not a cryptographic proof of source-chain state.
- Relayers must independently reconcile source signatures at the configured commitment and re-check slot/blockhash evidence before signing.
- Messages expire; destination program IDs and cluster-domain hashes are bound into canonical message IDs.
- A processed PDA prevents a second destination receipt for the same message ID.
- Local keypairs and validator faucets are development-only. Never point the deployment script at a shared or production cluster.
- The program does not handle SOL or SPL token custody.
- No third-party audit has been performed.

See [docs/SECURITY.md](docs/SECURITY.md) and [SECURITY.md](SECURITY.md).

## Roadmap

1. **Solana foundation (current):** Anchor PDAs, local dual-validator deployment, PostgreSQL schema, authenticated API, and Solana operations UI.
2. **Contract hardening:** Anchor integration/property coverage, signer rotation review, replay/expiry/domain cases, adversarial accounts, and compute-budget review.
3. **Durable scanner:** parse `MessageSent` logs, persist observations transactionally, and gate work on configured commitment.
4. **Safe relayer:** Redis leases plus PostgreSQL source of truth, outbox dispatch, reorg reconciliation, simulation, blockhash-expiry tracking, and signature reconciliation without blind retry.
5. **Operations:** populated routes, real-time dashboard data, Prometheus metrics, dashboards, backup/restore workflow, and release automation.
6. **Portfolio polish:** capture real localnet screenshots and publish a validation report after the full stack is verified.

## Repository layout

```text
programs/routerpulse/   Anchor program, accounts, instructions, events, errors
backend/                Rust/Axum API, SQLx startup migrations, Dockerfile
frontend/               React/TypeScript console, Nginx reverse proxy
scripts/                local setup, deployment, smoke, and build workflows
docs/                   architecture, API, operations, security, roadmap
compose.yaml            two Solana validators, PostgreSQL, Redis, API, UI
Anchor.toml             Anchor 0.30.1 and Solana 1.18.26 project configuration
```

## License

MIT. See [LICENSE](LICENSE).
