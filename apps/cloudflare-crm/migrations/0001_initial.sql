-- CRM Cloudflare D1 pilot only. No imported production records.
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS crm_tenants (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS crm_users (
  email TEXT PRIMARY KEY,
  tenant_id TEXT NOT NULL REFERENCES crm_tenants(id),
  role TEXT NOT NULL CHECK (role IN ('operator','admin')),
  active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0,1))
);
CREATE TABLE IF NOT EXISTS crm_customers (
  id TEXT PRIMARY KEY,
  tenant_id TEXT NOT NULL REFERENCES crm_tenants(id),
  name TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('person','organization')),
  updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_customers_tenant ON crm_customers(tenant_id, updated_at);
CREATE TABLE IF NOT EXISTS crm_audit (
  id TEXT PRIMARY KEY,
  tenant_id TEXT NOT NULL REFERENCES crm_tenants(id),
  action TEXT NOT NULL,
  subject_id TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_audit_tenant ON crm_audit(tenant_id, created_at);
