# CRM Customer Data Operations migrations

Phase 8A.7 begins with a pure owner/coordinator domain and deterministic private-state contracts.

Authoritative PostgreSQL migrations are added only with the production persistence adapter. They must preserve tenant isolation, exact optimistic job versions, immutable source/mapping binding, deterministic row uniqueness, resumable checkpoint monotonicity and rollback/reapply coverage.
