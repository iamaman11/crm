-- SK-07.C: Team belongs to the existing Access owner.
CREATE TABLE crm_access_teams (
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
CREATE INDEX idx_crm_team_name ON crm_access_teams(tenant,lower(json_extract(state,'$.name')));
CREATE TABLE crm_team_receipts (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 key TEXT NOT NULL,
 digest TEXT NOT NULL,
 state TEXT NOT NULL,
 original_request_id TEXT NOT NULL,
 PRIMARY KEY(tenant,key)
);
CREATE TRIGGER crm_team_create_audit AFTER INSERT ON crm_access_teams BEGIN
 INSERT INTO crm_team_receipts(tenant,key,digest,state,original_request_id)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,'access.team.create',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;
CREATE TRIGGER crm_team_update_audit AFTER UPDATE ON crm_access_teams BEGIN
 INSERT INTO crm_team_receipts(tenant,key,digest,state,original_request_id)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,'access.team.update',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;
CREATE INDEX idx_crm_task_owner_team ON crm_activity_task_records(tenant,json_extract(state,'$.owner.team_id'));
CREATE INDEX idx_crm_deal_owner_team ON crm_sales_deal_records(tenant,json_extract(state,'$.owner.team_id'));
