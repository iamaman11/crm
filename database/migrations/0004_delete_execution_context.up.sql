BEGIN;

CREATE OR REPLACE FUNCTION crm.require_write_context()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
  row_value jsonb;
  bound_tenant text := crm.current_tenant_id();
  bound_actor text := crm.current_actor_id();
  bound_request text := crm.current_request_id();
  bound_capability text := crm.current_capability_id();
  bound_capability_version text := crm.current_capability_version();
  bound_transaction text := crm.current_business_transaction_id();
  row_tenant text;
  row_transaction text;
BEGIN
  IF TG_OP = 'DELETE' THEN
    row_value := to_jsonb(OLD);
  ELSE
    row_value := to_jsonb(NEW);
  END IF;

  IF bound_tenant IS NULL OR bound_actor IS NULL OR bound_request IS NULL
     OR bound_capability IS NULL OR bound_capability_version IS NULL
     OR bound_transaction IS NULL THEN
    RAISE EXCEPTION USING
      ERRCODE = '28000',
      MESSAGE = 'complete transaction-local execution context is required';
  END IF;

  row_tenant := NULLIF(row_value ->> 'tenant_id', '');
  IF row_tenant IS NULL OR row_tenant <> bound_tenant THEN
    RAISE EXCEPTION USING
      ERRCODE = '42501',
      MESSAGE = 'row tenant does not match the bound tenant context';
  END IF;

  IF TG_OP <> 'DELETE' THEN
    IF row_value ? 'business_transaction_id' THEN
      row_transaction := NULLIF(row_value ->> 'business_transaction_id', '');
    ELSIF row_value ? 'last_business_transaction_id' THEN
      row_transaction := NULLIF(row_value ->> 'last_business_transaction_id', '');
    END IF;

    IF row_transaction IS NOT NULL AND row_transaction <> bound_transaction THEN
      RAISE EXCEPTION USING
        ERRCODE = '42501',
        MESSAGE = 'row business transaction does not match the bound transaction context';
    END IF;
  END IF;

  IF TG_OP = 'DELETE' THEN
    RETURN OLD;
  END IF;
  RETURN NEW;
END;
$$;

COMMIT;
