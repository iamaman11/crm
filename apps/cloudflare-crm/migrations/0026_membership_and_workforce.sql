-- SK-07.B: one authoritative workforce identity and membership ledger.
-- crm_users remains the ONLY admission source for Cloudflare Access; NO second
-- employee/auth table, no service-token impersonation or tenant reparenting.
ALTER TABLE crm_users ADD COLUMN version INTEGER NOT NULL DEFAULT 1 CHECK(version > 0);
ALTER TABLE crm_users ADD COLUMN idem_key TEXT NOT NULL DEFAULT '';
ALTER TABLE crm_users ADD COLUMN idem_digest TEXT NOT NULL DEFAULT '';
ALTER TABLE crm_users ADD COLUMN request_id TEXT NOT NULL DEFAULT '';
ALTER TABLE crm_users ADD COLUMN actor TEXT NOT NULL DEFAULT '';

CREATE INDEX idx_crm_users_tenant_email ON crm_users(tenant_id, email);
CREATE TABLE crm_member_receipts (
  tenant TEXT NOT NULL REFERENCES crm_tenants(id),
  key TEXT NOT NULL,
  digest TEXT NOT NULL,
  email TEXT NOT NULL,
  state TEXT NOT NULL,
  original_request_id TEXT NOT NULL,
  PRIMARY KEY(tenant, key)
);
CREATE TRIGGER crm_member_provision_audit
AFTER INSERT ON crm_users WHEN NEW.idem_key <> '' BEGIN
  SELECT CASE WHEN length(trim(NEW.actor))=0
    THEN RAISE(ABORT,'ACCESS_MEMBER_SIGNED_ACTOR_REQUIRED') END;
  INSERT INTO crm_member_receipts(tenant,key,digest,email,state,original_request_id)
  VALUES(NEW.tenant_id,NEW.idem_key,NEW.idem_digest,NEW.email,
    json_object('email',NEW.email,'role',NEW.role,'active',NEW.active,'version',NEW.version),
    NEW.request_id);
  INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant_id,'access.member.provision',NEW.email,
    datetime('now'),NEW.request_id,NEW.actor);
END;
CREATE TRIGGER crm_member_update_audit
AFTER UPDATE ON crm_users WHEN NEW.idem_key <> '' AND
  (NEW.version<>OLD.version OR NEW.idem_key<>OLD.idem_key) BEGIN
  SELECT CASE WHEN length(trim(NEW.actor))=0
    THEN RAISE(ABORT,'ACCESS_MEMBER_SIGNED_ACTOR_REQUIRED') END;
  INSERT INTO crm_member_receipts(tenant,key,digest,email,state,original_request_id)
  VALUES(NEW.tenant_id,NEW.idem_key,NEW.idem_digest,NEW.email,
    json_object('email',NEW.email,'role',NEW.role,'active',NEW.active,'version',NEW.version),
    NEW.request_id);
  INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant_id,'access.member.update',NEW.email,
    datetime('now'),NEW.request_id,NEW.actor);
END;
-- Server-owner filtering for team queues and workload BEFORE page size.
CREATE INDEX idx_crm_task_owner_actor
 ON crm_activity_task_records(tenant,json_extract(state,'$.owner.actor_id'));
