-- Test CRM D1: one small schema, no legacy data.
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS crm_tenants (
 id TEXT PRIMARY KEY, name TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS crm_users (
 email TEXT PRIMARY KEY, tenant_id TEXT NOT NULL REFERENCES crm_tenants(id),
 role TEXT NOT NULL DEFAULT 'admin', active INTEGER NOT NULL DEFAULT 1
);
CREATE TABLE IF NOT EXISTS crm_customers (
 id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL REFERENCES crm_tenants(id),
 name TEXT NOT NULL, kind TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS crm_contacts (
 id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL REFERENCES crm_tenants(id),
 customer_id TEXT NOT NULL REFERENCES crm_customers(id) ON DELETE CASCADE,
 name TEXT NOT NULL, email TEXT NOT NULL DEFAULT '', phone TEXT NOT NULL DEFAULT '',
 updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS crm_deals (
 id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL REFERENCES crm_tenants(id),
 customer_id TEXT NOT NULL REFERENCES crm_customers(id) ON DELETE CASCADE,
 title TEXT NOT NULL, stage TEXT NOT NULL DEFAULT 'new',
 amount REAL NOT NULL DEFAULT 0, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS crm_tasks (
 id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL REFERENCES crm_tenants(id),
 customer_id TEXT NOT NULL REFERENCES crm_customers(id) ON DELETE CASCADE,
 title TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'open',
 due_date TEXT NOT NULL DEFAULT '', updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS crm_audit (
 id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL REFERENCES crm_tenants(id),
 action TEXT NOT NULL, subject_id TEXT NOT NULL, created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_customers_tenant ON crm_customers(tenant_id, updated_at);
CREATE INDEX IF NOT EXISTS idx_contacts_tenant ON crm_contacts(tenant_id, updated_at);
CREATE INDEX IF NOT EXISTS idx_deals_tenant ON crm_deals(tenant_id, updated_at);
CREATE INDEX IF NOT EXISTS idx_tasks_tenant ON crm_tasks(tenant_id, updated_at);
CREATE INDEX IF NOT EXISTS idx_audit_tenant ON crm_audit(tenant_id, created_at);
