-- Isolated workerd CI D1 schema only. Never applied to crm-dev.
CREATE TABLE crm_party_records(
  tenant TEXT NOT NULL, id TEXT NOT NULL, state TEXT NOT NULL,
  version INTEGER NOT NULL CHECK(version > 0),
  idem_key TEXT NOT NULL, idem_digest TEXT NOT NULL,
  PRIMARY KEY(tenant,id)
);
CREATE TABLE crm_party_receipts(
  tenant TEXT NOT NULL, key TEXT NOT NULL, digest TEXT NOT NULL,
  state TEXT NOT NULL, PRIMARY KEY(tenant,key)
);
CREATE TABLE crm_party_audit(
  seq INTEGER PRIMARY KEY AUTOINCREMENT,
  tenant TEXT NOT NULL, id TEXT NOT NULL,
  action TEXT NOT NULL, version INTEGER NOT NULL
);
CREATE TRIGGER crm_party_create_audit AFTER INSERT ON crm_party_records BEGIN
  INSERT INTO crm_party_receipts VALUES (NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
  INSERT INTO crm_party_audit(tenant,id,action,version)
    VALUES(NEW.tenant,NEW.id,'create',NEW.version);
END;
CREATE TRIGGER crm_party_update_audit AFTER UPDATE ON crm_party_records BEGIN
  INSERT INTO crm_party_receipts VALUES (NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
  INSERT INTO crm_party_audit(tenant,id,action,version)
    VALUES(NEW.tenant,NEW.id,'update',NEW.version);
END;
