-- Tenant-scoped named CSV sources and external-ID -> original Party identity.
-- Only SHA-256 external-ID digests are persisted; raw source IDs and CSV bytes are transient.
ALTER TABLE crm_party_import_receipts ADD COLUMN source_id TEXT;
ALTER TABLE crm_party_import_receipts ADD COLUMN preview_fingerprint TEXT;

CREATE TABLE crm_party_import_sources (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 id TEXT NOT NULL,
 display_name TEXT NOT NULL CHECK(length(display_name) BETWEEN 1 AND 240),
 normalized_name TEXT NOT NULL CHECK(length(normalized_name) BETWEEN 1 AND 960),
 created_by TEXT NOT NULL,
 request_id TEXT NOT NULL,
 PRIMARY KEY(tenant,id),
 UNIQUE(tenant,normalized_name)
);

CREATE TABLE crm_party_import_source_receipts (
 tenant TEXT NOT NULL,
 key TEXT NOT NULL CHECK(length(key) BETWEEN 8 AND 120),
 source_id TEXT NOT NULL,
 request_id TEXT NOT NULL CHECK(length(request_id) > 0),
 PRIMARY KEY(tenant,key),
 FOREIGN KEY(tenant,source_id) REFERENCES crm_party_import_sources(tenant,id)
);

CREATE INDEX crm_party_import_source_receipt_source
 ON crm_party_import_source_receipts(tenant,source_id);

CREATE TABLE crm_party_source_identities (
 tenant TEXT NOT NULL,
 source_id TEXT NOT NULL,
 external_id_sha256 TEXT NOT NULL
   CHECK(length(external_id_sha256)=64 AND external_id_sha256 NOT GLOB '*[^0-9a-f]*'),
 party_id TEXT NOT NULL,
 party_version INTEGER NOT NULL CHECK(party_version > 0),
 request_id TEXT NOT NULL,
 actor TEXT NOT NULL,
 PRIMARY KEY(tenant,source_id,external_id_sha256),
 FOREIGN KEY(tenant,source_id) REFERENCES crm_party_import_sources(tenant,id),
 FOREIGN KEY(tenant,party_id) REFERENCES crm_party_records(tenant,id)
);

CREATE INDEX crm_party_source_identity_party
 ON crm_party_source_identities(tenant,party_id);

CREATE TRIGGER crm_party_import_source_registered
AFTER INSERT ON crm_party_import_sources BEGIN
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,
        'customer_data.csv_import_source.register',NEW.id,datetime('now'),NEW.request_id,NEW.created_by);
END;

CREATE TRIGGER crm_party_source_identity_bound
AFTER INSERT ON crm_party_source_identities BEGIN
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,
        'customer_data.csv_import_identity.bind',NEW.party_id,datetime('now'),NEW.request_id,NEW.actor);
END;

-- A matched-row no-op update is the transactional CAS guard for the Party version
-- observed by Preview. If the Party changed after Preview, the whole D1 batch aborts.
CREATE TRIGGER crm_party_source_identity_version_guard
BEFORE UPDATE OF party_version ON crm_party_source_identities
WHEN NOT EXISTS (
 SELECT 1 FROM crm_party_records
 WHERE tenant=NEW.tenant AND id=NEW.party_id AND version=NEW.party_version
)
BEGIN
 SELECT RAISE(ABORT,'PARTY_SOURCE_IDENTITY_VERSION_CONFLICT');
END;
