# API reference

The API provides authenticated operator views over PostgreSQL and read-only Solana RPC health checks. It does not yet scan Anchor logs or submit destination transactions.

## Authentication

Operator endpoints require `x-api-key`. `/livez` and `/readyz` are public. Local credentials are generated into `.env`; do not commit them. API keys must have at least 24 bytes unless `ALLOW_WEAK_LOCAL_API_KEY=true` is explicitly used for local development.

## Endpoints

| Method | Path | Auth | Purpose |
|---|---|---|---|
| GET | `/livez` | No | Process liveness |
| GET | `/readyz` | No | Database readiness |
| GET | `/api/v1/clusters` | Yes | Genesis hash, domain hash, current slot, RPC health |
| GET | `/api/v1/routes` | Yes | Configured route records |
| GET | `/api/v1/messages` | Yes | Persisted messages; supports `status`, `limit` (max 200), and `before` cursor |
| GET | `/api/v1/messages/{id}` | Yes | One 32-byte message ID, hex encoded |
| GET | `/api/v1/relayer/health` | Yes | Relayer heartbeat/readiness; worker is not implemented yet |

Example:

```bash
curl -H "x-api-key: $(sed -n 's/^API_KEY=//p' .env)" http://localhost:8080/api/v1/clusters
```

Message fields use Solana signatures and slots. A sequence is an application-level sender sequence stored by the Anchor program; it is not a Solana transaction nonce. The API returns empty message lists until an event scanner writes records.

Errors use JSON `{ "code": "...", "message": "..." }`. Responses include `x-request-id` for log correlation.
