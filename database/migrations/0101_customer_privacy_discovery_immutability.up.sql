CREATE FUNCTION crm.reject_customer_privacy_discovery_evidence_mutation()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
  RAISE EXCEPTION 'customer privacy discovery evidence is immutable'
    USING ERRCODE = '55000';
END;
$$;

CREATE TRIGGER customer_privacy_discovery_attempts_immutable
BEFORE UPDATE OR DELETE ON crm.customer_privacy_discovery_attempts
FOR EACH ROW EXECUTE FUNCTION crm.reject_customer_privacy_discovery_evidence_mutation();

CREATE TRIGGER customer_privacy_discovery_owner_pages_immutable
BEFORE UPDATE OR DELETE ON crm.customer_privacy_discovery_owner_pages
FOR EACH ROW EXECUTE FUNCTION crm.reject_customer_privacy_discovery_evidence_mutation();

CREATE TRIGGER customer_privacy_discovery_snapshots_immutable
BEFORE UPDATE OR DELETE ON crm.customer_privacy_discovery_snapshots
FOR EACH ROW EXECUTE FUNCTION crm.reject_customer_privacy_discovery_evidence_mutation();

CREATE TRIGGER customer_privacy_discovery_audit_immutable
BEFORE UPDATE OR DELETE ON crm.customer_privacy_discovery_audit
FOR EACH ROW EXECUTE FUNCTION crm.reject_customer_privacy_discovery_evidence_mutation();

CREATE TRIGGER customer_privacy_discovery_snapshot_records_immutable
BEFORE UPDATE OR DELETE ON crm.records
FOR EACH ROW
WHEN (OLD.record_type = 'customer-privacy.scope-snapshot')
EXECUTE FUNCTION crm.reject_customer_privacy_discovery_evidence_mutation();
