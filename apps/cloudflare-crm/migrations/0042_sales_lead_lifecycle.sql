CREATE TABLE crm_sales_lead_records (
 tenant TEXT NOT NULL,
 id TEXT NOT NULL,
 original_party_id TEXT NOT NULL,
 name TEXT NOT NULL,
 status TEXT NOT NULL,
 reason TEXT NOT NULL DEFAULT '',
 deal_id TEXT,
 version INTEGER NOT NULL,
 idem_key TEXT NOT NULL,
 idem_digest TEXT NOT NULL,
 request_id TEXT NOT NULL,
 actor TEXT NOT NULL,
 PRIMARY KEY(tenant,id),
 UNIQUE(tenant,deal_id),
 FOREIGN KEY(tenant,original_party_id) REFERENCES crm_party_records(tenant,id),
 FOREIGN KEY(tenant,deal_id) REFERENCES crm_sales_deal_records(tenant,id),
 CHECK (
   (status='new' AND version=1 AND reason='' AND deal_id IS NULL)
   OR (status IN ('qualified','rejected') AND version=2 AND length(reason)>0 AND deal_id IS NULL)
   OR (status='converted' AND version=3 AND length(reason)>0 AND deal_id IS NOT NULL)
 )
);

-- Immutable post-images serve as both history and replay receipts.
CREATE TABLE crm_sales_lead_events (
 tenant TEXT NOT NULL,
 id TEXT NOT NULL,
 original_party_id TEXT NOT NULL,
 name TEXT NOT NULL,
 status TEXT NOT NULL,
 reason TEXT NOT NULL,
 deal_id TEXT,
 version INTEGER NOT NULL,
 idem_key TEXT NOT NULL,
 idem_digest TEXT NOT NULL,
 request_id TEXT NOT NULL,
 occurred_at INTEGER NOT NULL DEFAULT (unixepoch()),
 PRIMARY KEY(tenant,id,version),
 UNIQUE(tenant,idem_key),
 FOREIGN KEY(tenant,id) REFERENCES crm_sales_lead_records(tenant,id)
);

CREATE TRIGGER crm_sales_lead_created
AFTER INSERT ON crm_sales_lead_records BEGIN
 INSERT INTO crm_sales_lead_events(tenant,id,original_party_id,name,status,reason,deal_id,version,idem_key,idem_digest,request_id)
 VALUES(NEW.tenant,NEW.id,NEW.original_party_id,NEW.name,NEW.status,NEW.reason,NEW.deal_id,NEW.version,NEW.idem_key,NEW.idem_digest,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,'sales_leads.create',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

CREATE TRIGGER crm_sales_lead_changed
AFTER UPDATE ON crm_sales_lead_records BEGIN
 INSERT INTO crm_sales_lead_events(tenant,id,original_party_id,name,status,reason,deal_id,version,idem_key,idem_digest,request_id)
 VALUES(NEW.tenant,NEW.id,NEW.original_party_id,NEW.name,NEW.status,NEW.reason,NEW.deal_id,NEW.version,NEW.idem_key,NEW.idem_digest,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,
    CASE NEW.status WHEN 'qualified' THEN 'sales_leads.qualify' WHEN 'rejected' THEN 'sales_leads.reject' ELSE 'sales_leads.convert' END,
    NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

ALTER TABLE crm_sales_deal_records ADD COLUMN source_lead_id TEXT;
CREATE UNIQUE INDEX crm_sales_deal_source_lead
 ON crm_sales_deal_records(tenant,source_lead_id) WHERE source_lead_id IS NOT NULL;

-- Standard Deal creation keeps its original triggers. A source Lead also
-- moves to converted in the same statement, including both audits/receipts.
CREATE TRIGGER crm_sales_lead_converted
AFTER INSERT ON crm_sales_deal_records WHEN NEW.source_lead_id IS NOT NULL BEGIN
 UPDATE crm_sales_lead_records
 SET status='converted',version=version+1,deal_id=NEW.id,
     idem_key=NEW.idem_key,idem_digest=NEW.idem_digest,request_id=NEW.request_id,actor=NEW.actor
 WHERE tenant=NEW.tenant AND id=NEW.source_lead_id AND status='qualified';
 SELECT CASE WHEN changes()<>1 THEN RAISE(ABORT,'SALES_LEAD_CONVERSION_CONFLICT') END;
END;
