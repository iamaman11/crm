-- Original Customer Privacy intake in the existing one D1.
-- No implicit identity verification, restriction, export or erasure.
CREATE TABLE IF NOT EXISTS crm_privacy_case_records (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 id TEXT NOT NULL,
 state TEXT NOT NULL,
 version INTEGER NOT NULL CHECK(version > 0),
 idem_key TEXT NOT NULL,
 idem_digest TEXT NOT NULL,
 request_id TEXT NOT NULL,
 created_by TEXT NOT NULL,
 PRIMARY KEY(tenant,id)
);
CREATE TABLE IF NOT EXISTS crm_privacy_case_receipts (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 key TEXT NOT NULL,
 digest TEXT NOT NULL,
 state TEXT NOT NULL,
 PRIMARY KEY(tenant,key)
);
CREATE TRIGGER IF NOT EXISTS crm_privacy_case_create_audit
AFTER INSERT ON crm_privacy_case_records BEGIN
 INSERT INTO crm_privacy_case_receipts(tenant,key,digest,state)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,
        'customer_privacy.create',NEW.id,datetime('now'),NEW.request_id);
END;
CREATE TRIGGER IF NOT EXISTS crm_privacy_case_transition_audit
AFTER UPDATE ON crm_privacy_case_records BEGIN
 INSERT INTO crm_privacy_case_receipts(tenant,key,digest,state)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,
        CASE WHEN json_extract(NEW.state,'$.status.code')='submitted'
             THEN 'customer_privacy.submit'
             ELSE 'customer_privacy.cancel' END,
        NEW.id,datetime('now'),NEW.request_id);
END;
