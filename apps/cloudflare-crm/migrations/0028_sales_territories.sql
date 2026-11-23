-- Sales-owned territory and commercial account assignments.
CREATE TABLE crm_sales_territory_records (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 id TEXT NOT NULL,
 state TEXT NOT NULL CHECK(json_valid(state)),
 version INTEGER NOT NULL CHECK(version>0),
 idem_key TEXT NOT NULL,
 idem_digest TEXT NOT NULL,
 request_id TEXT NOT NULL,
 actor TEXT NOT NULL,
 PRIMARY KEY(tenant,id)
);
CREATE TABLE crm_sales_territory_receipts (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 key TEXT NOT NULL,
 digest TEXT NOT NULL,
 state TEXT NOT NULL,
 original_request_id TEXT NOT NULL,
 PRIMARY KEY(tenant,key)
);
CREATE TRIGGER crm_sales_territory_create_audit AFTER INSERT ON crm_sales_territory_records BEGIN
 INSERT INTO crm_sales_territory_receipts(tenant,key,digest,state,original_request_id)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,'sales.territory.create',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;
CREATE TRIGGER crm_sales_territory_update_audit AFTER UPDATE ON crm_sales_territory_records BEGIN
 INSERT INTO crm_sales_territory_receipts(tenant,key,digest,state,original_request_id)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,'sales.territory.update',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;
CREATE TABLE crm_sales_account_assignment_records (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 id TEXT NOT NULL,
 state TEXT NOT NULL CHECK(json_valid(state)),
 version INTEGER NOT NULL CHECK(version>0),
 idem_key TEXT NOT NULL,
 idem_digest TEXT NOT NULL,
 request_id TEXT NOT NULL,
 actor TEXT NOT NULL,
 PRIMARY KEY(tenant,id),
 FOREIGN KEY(tenant,id) REFERENCES crm_account_records(tenant,id)
);
CREATE TABLE crm_sales_account_assignment_receipts (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 key TEXT NOT NULL,
 digest TEXT NOT NULL,
 state TEXT NOT NULL,
 original_request_id TEXT NOT NULL,
 PRIMARY KEY(tenant,key)
);
CREATE TRIGGER crm_sales_account_assignment_create_audit AFTER INSERT ON crm_sales_account_assignment_records BEGIN
 INSERT INTO crm_sales_account_assignment_receipts(tenant,key,digest,state,original_request_id)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,'sales.account_assignment.create',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;
CREATE TRIGGER crm_sales_account_assignment_update_audit AFTER UPDATE ON crm_sales_account_assignment_records BEGIN
 INSERT INTO crm_sales_account_assignment_receipts(tenant,key,digest,state,original_request_id)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,'sales.account_assignment.update',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;
CREATE INDEX idx_crm_sales_territory_parent ON crm_sales_territory_records(tenant,json_extract(state,'$.parent_id'));
CREATE INDEX idx_crm_sales_assignment_territory ON crm_sales_account_assignment_records(tenant,json_extract(state,'$.territory_id'));
CREATE INDEX idx_crm_sales_assignment_owner ON crm_sales_account_assignment_records(tenant,json_extract(state,'$.primary_owner'));
