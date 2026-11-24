-- SK-06: tenant-scoped read indexes for due-ordered Task and latest
-- Interaction facts. No new table, owner, projection or write path.
CREATE INDEX idx_crm_tasks_due_workday
 ON crm_activity_task_records(
   tenant,
   CAST(json_extract(state,'$.due_at_unix_nanos') AS INTEGER),
   id
 ) WHERE json_extract(state,'$.due_at_unix_nanos') IS NOT NULL;
CREATE INDEX idx_crm_interactions_tenant_time
 ON crm_activity_interactions(tenant,occurred_at_key DESC,id DESC);
