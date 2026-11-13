CREATE TYPE message_status AS ENUM (
  'created', 'source_submitted', 'source_confirming', 'source_confirmed',
  'source_reorged', 'queued', 'verifying', 'ready_to_submit',
  'destination_submitted', 'destination_confirming', 'completed',
  'retry_scheduled', 'rejected', 'dead_lettered'
);

CREATE TABLE chains (
  id BIGINT PRIMARY KEY, name TEXT NOT NULL, rpc_url TEXT NOT NULL,
  confirmation_depth INTEGER NOT NULL CHECK (confirmation_depth > 0),
  enabled BOOLEAN NOT NULL DEFAULT TRUE, created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE routes (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(), source_chain_id BIGINT NOT NULL REFERENCES chains(id),
  destination_chain_id BIGINT NOT NULL REFERENCES chains(id), source_router TEXT NOT NULL,
  destination_router TEXT NOT NULL, enabled BOOLEAN NOT NULL DEFAULT TRUE,
  UNIQUE(source_chain_id, destination_chain_id)
);
CREATE TABLE messages (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(), message_id TEXT NOT NULL UNIQUE,
  source_chain_id BIGINT NOT NULL REFERENCES chains(id), destination_chain_id BIGINT NOT NULL REFERENCES chains(id),
  source_router TEXT NOT NULL, destination_router TEXT NOT NULL, sender TEXT NOT NULL, receiver TEXT NOT NULL,
  nonce NUMERIC(78,0) NOT NULL, payload_hash TEXT NOT NULL, payload BYTEA NOT NULL,
  source_tx_hash TEXT, source_block_number BIGINT, source_block_hash TEXT,
  status message_status NOT NULL DEFAULT 'created', confirmation_depth INTEGER NOT NULL,
  deadline TIMESTAMPTZ NOT NULL, version BIGINT NOT NULL DEFAULT 0,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(), updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX messages_pending_idx ON messages(status, updated_at) WHERE status NOT IN ('completed','rejected','dead_lettered');
CREATE TABLE message_observations (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(), message_id UUID NOT NULL REFERENCES messages(id),
  block_number BIGINT NOT NULL, block_hash TEXT NOT NULL, transaction_hash TEXT NOT NULL,
  observed_at TIMESTAMPTZ NOT NULL DEFAULT now(), UNIQUE(message_id, block_hash)
);
CREATE TABLE message_attempts (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(), message_id UUID NOT NULL REFERENCES messages(id),
  attempt_number INTEGER NOT NULL, destination_tx_hash TEXT UNIQUE, signer_address TEXT, nonce NUMERIC(78,0),
  gas_limit NUMERIC(78,0), status TEXT NOT NULL, error_code TEXT, error_detail TEXT,
  submitted_at TIMESTAMPTZ, confirmed_at TIMESTAMPTZ, UNIQUE(message_id, attempt_number)
);
CREATE TABLE audit_events (
  id BIGSERIAL PRIMARY KEY, message_id UUID REFERENCES messages(id), actor TEXT NOT NULL,
  event_type TEXT NOT NULL, state_from message_status, state_to message_status,
  details JSONB NOT NULL DEFAULT '{}', created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

