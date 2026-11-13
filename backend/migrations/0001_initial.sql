CREATE TYPE message_status AS ENUM (
  'source_observed', 'source_confirming', 'source_finalized', 'queued',
  'verifying', 'ready_to_execute', 'destination_submitted',
  'destination_confirming', 'completed', 'source_reorged',
  'retry_scheduled', 'rejected', 'dead_lettered'
);

CREATE TABLE clusters (
  domain_hash BYTEA PRIMARY KEY CHECK (octet_length(domain_hash) = 32),
  genesis_hash TEXT NOT NULL,
  name TEXT NOT NULL UNIQUE,
  rpc_url TEXT NOT NULL,
  websocket_url TEXT NOT NULL,
  commitment TEXT NOT NULL CHECK (commitment IN ('confirmed','finalized')),
  enabled BOOLEAN NOT NULL DEFAULT TRUE,
  last_observed_slot BIGINT,
  last_healthy_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE routes (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  source_domain_hash BYTEA NOT NULL REFERENCES clusters(domain_hash),
  destination_domain_hash BYTEA NOT NULL REFERENCES clusters(domain_hash),
  source_program TEXT NOT NULL,
  destination_program TEXT NOT NULL,
  source_confirmations INTEGER NOT NULL CHECK (source_confirmations > 0),
  destination_confirmations INTEGER NOT NULL CHECK (destination_confirmations > 0),
  max_payload_bytes INTEGER NOT NULL DEFAULT 1024 CHECK (max_payload_bytes BETWEEN 1 AND 1024),
  enabled BOOLEAN NOT NULL DEFAULT FALSE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CHECK (source_domain_hash <> destination_domain_hash),
  UNIQUE(source_domain_hash, destination_domain_hash, source_program, destination_program)
);

CREATE TABLE messages (
  message_id BYTEA PRIMARY KEY CHECK (octet_length(message_id) = 32),
  source_domain_hash BYTEA NOT NULL REFERENCES clusters(domain_hash),
  destination_domain_hash BYTEA NOT NULL REFERENCES clusters(domain_hash),
  source_program TEXT NOT NULL,
  destination_program TEXT NOT NULL,
  sender TEXT NOT NULL,
  receiver TEXT NOT NULL,
  sequence BIGINT NOT NULL CHECK (sequence >= 0),
  payload_hash BYTEA NOT NULL CHECK (octet_length(payload_hash) = 32),
  payload BYTEA NOT NULL CHECK (octet_length(payload) <= 1024),
  source_signature TEXT NOT NULL,
  source_slot BIGINT NOT NULL CHECK (source_slot >= 0),
  source_blockhash TEXT NOT NULL,
  source_finalized_slot BIGINT,
  destination_signature TEXT UNIQUE,
  destination_slot BIGINT,
  status message_status NOT NULL DEFAULT 'source_observed',
  status_reason TEXT NOT NULL DEFAULT 'Source message observed',
  version BIGINT NOT NULL DEFAULT 0,
  expires_at TIMESTAMPTZ NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE(source_domain_hash, source_program, sender, sequence)
);
CREATE INDEX messages_pending_idx ON messages(status, updated_at)
  WHERE status NOT IN ('completed','rejected','dead_lettered');
CREATE INDEX messages_source_slot_idx ON messages(source_domain_hash, source_slot);
CREATE INDEX messages_route_created_idx ON messages(source_domain_hash, destination_domain_hash, created_at DESC);

CREATE TABLE message_observations (
  id BIGSERIAL PRIMARY KEY,
  message_id BYTEA NOT NULL REFERENCES messages(message_id),
  observed_slot BIGINT NOT NULL,
  observed_blockhash TEXT NOT NULL,
  observation_kind TEXT NOT NULL CHECK (observation_kind IN ('websocket','poll','reconciliation')),
  observed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE(message_id, observed_blockhash, observation_kind)
);

CREATE TABLE message_attempts (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  message_id BYTEA NOT NULL REFERENCES messages(message_id),
  attempt_number INTEGER NOT NULL CHECK (attempt_number > 0),
  destination_signature TEXT UNIQUE,
  signer_pubkey TEXT NOT NULL,
  recent_blockhash TEXT,
  last_valid_block_height BIGINT,
  status TEXT NOT NULL CHECK (status IN ('prepared','simulated','submitted','confirmed','failed','unknown','expired')),
  error_code TEXT,
  error_detail TEXT,
  submitted_at TIMESTAMPTZ,
  confirmed_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE(message_id, attempt_number)
);
CREATE INDEX attempts_pending_idx ON message_attempts(status, created_at)
  WHERE status IN ('submitted','unknown','prepared');

CREATE TABLE audit_events (
  id BIGSERIAL PRIMARY KEY,
  message_id BYTEA REFERENCES messages(message_id),
  actor TEXT NOT NULL,
  event_type TEXT NOT NULL,
  state_from message_status,
  state_to message_status,
  details JSONB NOT NULL DEFAULT '{}',
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX audit_message_time_idx ON audit_events(message_id, created_at);

CREATE TABLE outbox (
  id BIGSERIAL PRIMARY KEY,
  event_type TEXT NOT NULL,
  aggregate_id BYTEA NOT NULL,
  payload JSONB NOT NULL,
  attempts INTEGER NOT NULL DEFAULT 0,
  available_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  claimed_by TEXT,
  claimed_until TIMESTAMPTZ,
  published_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX outbox_claim_idx ON outbox(available_at, id) WHERE published_at IS NULL;

CREATE TABLE idempotency_keys (
  actor TEXT NOT NULL,
  idempotency_key TEXT NOT NULL,
  request_hash BYTEA NOT NULL CHECK (octet_length(request_hash) = 32),
  response JSONB,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  expires_at TIMESTAMPTZ NOT NULL,
  PRIMARY KEY(actor, idempotency_key)
);

CREATE TABLE relayer_heartbeats (
  worker_id TEXT PRIMARY KEY,
  version TEXT NOT NULL,
  source_slot BIGINT,
  destination_slot BIGINT,
  active_leases INTEGER NOT NULL DEFAULT 0,
  last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE FUNCTION routerpulse_touch_updated_at() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN NEW.updated_at = now(); RETURN NEW; END;
$$;
CREATE TRIGGER routes_touch_updated_at BEFORE UPDATE ON routes
  FOR EACH ROW EXECUTE FUNCTION routerpulse_touch_updated_at();
CREATE TRIGGER messages_touch_updated_at BEFORE UPDATE ON messages
  FOR EACH ROW EXECUTE FUNCTION routerpulse_touch_updated_at();

CREATE FUNCTION routerpulse_audit_message_transition() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF OLD.status IS DISTINCT FROM NEW.status THEN
    INSERT INTO audit_events(message_id, actor, event_type, state_from, state_to, details)
    VALUES (NEW.message_id, current_user, 'state_transition', OLD.status, NEW.status,
      jsonb_build_object('reason', NEW.status_reason, 'version', NEW.version));
  END IF;
  RETURN NEW;
END;
$$;
CREATE TRIGGER messages_audit_transition AFTER UPDATE ON messages
  FOR EACH ROW EXECUTE FUNCTION routerpulse_audit_message_transition();
