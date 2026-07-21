# Customer Enrichment Suggestion Query Adapter

Production permission-aware query adapter for exact suggestion lookup and list-by-Party. The list surface binds tenant, actor, capability version, Party/profile/status filters, page size and stable sort into a signed cursor, scans visibility in bounded batches and returns an empty page when the target Party is hidden.
