CREATE TABLE customer_privacy.processing_restrictions (
  tenant_id text NOT NULL,
  restriction_id text NOT NULL,
  privacy_case_id text NOT NULL,
  canonical_party_id text NOT NULL,
  state text NOT NULL CHECK (state = 'active'),
  scopes text[] NOT NULL CHECK (
    cardinality(scopes) = 1
    AND scopes[1] = 'all_processing'
  ),
  channels text[] NOT NULL DEFAULT ARRAY[]::text[] CHECK (cardinality(channels) = 0),
  starts_at_unix_nanos bigint NOT NULL CHECK (starts_at_unix_nanos > 0),
  expires_at_unix_nanos bigint,
  reason text NOT NULL CHECK (btrim(reason) <> ''),
  legal_basis text NOT NULL CHECK (btrim(legal_basis) <> ''),
  policy_version text NOT NULL CHECK (btrim(policy_version) <> ''),
  placed_at_unix_nanos bigint NOT NULL CHECK (
    placed_at_unix_nanos > 0
    AND mod(placed_at_unix_nanos, 1000) = 0
    AND starts_at_unix_nanos <= placed_at_unix_nanos
  ),
  placed_by_actor_id text NOT NULL,
  request_id text NOT NULL,
  correlation_id text NOT NULL,
  trace_id text NOT NULL,
  idempotency_key text NOT NULL CHECK (btrim(idempotency_key) <> ''),
  restriction_version bigint NOT NULL DEFAULT 1 CHECK (restriction_version = 1),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_id, restriction_id),
  UNIQUE (tenant_id, idempotency_key),
  UNIQUE (tenant_id, canonical_party_id),
  CHECK (expires_at_unix_nanos IS NULL)
);

CREATE INDEX processing_restrictions_case_idx
  ON customer_privacy.processing_restrictions (tenant_id, privacy_case_id);

CREATE TABLE customer_privacy.processing_restriction_events (
  tenant_id text NOT NULL,
  restriction_id text NOT NULL,
  event_sequence bigint NOT NULL CHECK (event_sequence = 1),
  event_type text NOT NULL CHECK (event_type = 'customer_privacy.restriction.placed'),
  privacy_case_id text NOT NULL,
  canonical_party_id text NOT NULL,
  policy_version text NOT NULL,
  request_id text NOT NULL,
  actor_id text NOT NULL,
  recorded_at_unix_nanos bigint NOT NULL CHECK (
    recorded_at_unix_nanos > 0
    AND mod(recorded_at_unix_nanos, 1000) = 0
  ),
  PRIMARY KEY (tenant_id, restriction_id, event_sequence),
  FOREIGN KEY (tenant_id, restriction_id)
    REFERENCES customer_privacy.processing_restrictions (tenant_id, restriction_id)
    ON DELETE RESTRICT
);

CREATE TABLE customer_privacy.processing_restriction_idempotency (
  tenant_id text NOT NULL,
  idempotency_key text NOT NULL,
  restriction_id text NOT NULL,
  privacy_case_id text NOT NULL,
  canonical_party_id text NOT NULL,
  request_id text NOT NULL,
  policy_version text NOT NULL,
  committed_at_unix_nanos bigint NOT NULL CHECK (
    committed_at_unix_nanos > 0
    AND mod(committed_at_unix_nanos, 1000) = 0
  ),
  PRIMARY KEY (tenant_id, idempotency_key),
  UNIQUE (tenant_id, restriction_id),
  FOREIGN KEY (tenant_id, restriction_id)
    REFERENCES customer_privacy.processing_restrictions (tenant_id, restriction_id)
    ON DELETE RESTRICT
);

ALTER TABLE customer_privacy.processing_restrictions ENABLE ROW LEVEL SECURITY;
ALTER TABLE customer_privacy.processing_restrictions FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON customer_privacy.processing_restrictions
  USING (tenant_id = current_setting('app.tenant_id', true))
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true));

ALTER TABLE customer_privacy.processing_restriction_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE customer_privacy.processing_restriction_events FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON customer_privacy.processing_restriction_events
  USING (tenant_id = current_setting('app.tenant_id', true))
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true));

ALTER TABLE customer_privacy.processing_restriction_idempotency ENABLE ROW LEVEL SECURITY;
ALTER TABLE customer_privacy.processing_restriction_idempotency FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON customer_privacy.processing_restriction_idempotency
  USING (tenant_id = current_setting('app.tenant_id', true))
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true));
