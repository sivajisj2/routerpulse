# API reference

The API currently exposes a small authenticated scaffold. It does not yet read chain logs or persist messages.

## Authentication

Operator requests must send `x-api-key`. `/livez` and `/readyz` are public health endpoints. Configure the local key in `.env`; never commit `.env`.

## Routes

| Method | Path | Auth | Result |
|---|---|---|---|
| GET | `/livez` | No | `ok` |
| GET | `/readyz` | No | `{"status":"ready"}` |
| GET | `/api/v1/chains` | Yes | Two local chain descriptors |
| GET | `/api/v1/messages` | Yes | `{ "items": [], "total": 0 }` |
| GET | `/api/v1/messages/{id}` | Yes | Message record or 404 |
| POST | `/api/v1/messages` | Yes | Idempotently register a message in process memory |

### Register a message record

```http
POST /api/v1/messages
x-api-key: local-demo-key-change-me
Content-Type: application/json

{
  "message_id": "0xexample",
  "source_chain_id": 31337,
  "destination_chain_id": 31338,
  "sender": "0x...",
  "receiver": "0x..."
}
```

The response contains the message and `replayed`. Repeating the same `message_id` returns the existing record. This is only an API idempotency demonstration; it does not create an on-chain message or submit a transaction.

