-- L-03: original crm-identity-resolution reversible merge lineage.
-- This is NOT a destructive merge or reparent operation: every Party and its
-- Account/ContactPoint/Relationship/Deal/Task references remain unchanged.
-- Canonical identity is derived from active owner merge-operation edges.
CREATE TABLE crm_identity_merge_records (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 id TEXT NOT NULL,
 source_party TEXT NOT NULL,
 survivor_party TEXT NOT NULL,
 state TEXT NOT NULL,
 version INTEGER NOT NULL CHECK(version IN (1,2)),
 status TEXT NOT NULL CHECK(status IN ('active','unmerged')),
 idem_key TEXT NOT NULL,
 idem_digest TEXT NOT NULL,
 request_id TEXT NOT NULL,
 actor TEXT NOT NULL CHECK(length(trim(actor))>0),
 PRIMARY KEY(tenant,id),
 FOREIGN KEY(tenant,source_party) REFERENCES crm_party_records(tenant,id),
 FOREIGN KEY(tenant,survivor_party) REFERENCES crm_party_records(tenant,id),
 CHECK(source_party != survivor_party),
 CHECK((version=1 AND status='active') OR (version=2 AND status='unmerged'))
);
CREATE UNIQUE INDEX idx_crm_identity_merge_active_source
 ON crm_identity_merge_records(tenant,source_party) WHERE status='active';
CREATE INDEX idx_crm_identity_merge_survivor
 ON crm_identity_merge_records(tenant,survivor_party,id);
CREATE TABLE crm_identity_merge_receipts (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 key TEXT NOT NULL,
 digest TEXT NOT NULL,
 state TEXT NOT NULL,
 original_request_id TEXT NOT NULL,
 PRIMARY KEY(tenant,key)
);
-- The original Rust domain encodes strict canonical JSON and lifecycle.
-- Concurrency-proof CAS happens INSIDE SQLite's native BEFORE INSERT trigger,
-- in the SAME D1 statement as the state+receipt+audit write. The application
-- cannot race an intervening Party update or canonical redirect.
CREATE TRIGGER crm_identity_merge_preflight BEFORE INSERT ON crm_identity_merge_records BEGIN
 SELECT CASE WHEN
   NOT EXISTS(SELECT 1 FROM crm_party_records
     WHERE tenant=NEW.tenant AND id=NEW.source_party
       AND version=json_extract(NEW.state,'$.source_party_version'))
   OR NOT EXISTS(SELECT 1 FROM crm_party_records
     WHERE tenant=NEW.tenant AND id=NEW.survivor_party
       AND version=json_extract(NEW.state,'$.survivor_party_version'))
   OR EXISTS(SELECT 1 FROM crm_identity_merge_records
     WHERE tenant=NEW.tenant AND status='active'
       AND source_party IN (NEW.source_party, NEW.survivor_party))
 THEN RAISE(ABORT,'IDENTITY_MERGE_STALE_OR_REDIRECTED') END;
END;
-- SQLite guarantees atomic receipt + audit + unique source even on races.
CREATE TRIGGER crm_identity_merge_created AFTER INSERT ON crm_identity_merge_records BEGIN
 INSERT INTO crm_identity_merge_receipts(tenant,key,digest,state,original_request_id)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,'identity_resolution.merge',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;
CREATE TRIGGER crm_identity_merge_update_guard BEFORE UPDATE ON crm_identity_merge_records BEGIN
 SELECT CASE WHEN OLD.status!='active' OR OLD.version!=1 OR
   NEW.status!='unmerged' OR NEW.version!=2 OR
   NEW.tenant!=OLD.tenant OR NEW.id!=OLD.id OR
   NEW.source_party!=OLD.source_party OR NEW.survivor_party!=OLD.survivor_party
 THEN RAISE(ABORT,'IDENTITY_RESOLUTION_INVALID_UNMERGE') END;
END;
CREATE TRIGGER crm_identity_merge_unmerged AFTER UPDATE ON crm_identity_merge_records BEGIN
 INSERT INTO crm_identity_merge_receipts(tenant,key,digest,state,original_request_id)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,'identity_resolution.unmerge',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;
