# CRM Data Quality migrations

The module declares its owned record types through `module.yaml`, while production PostgreSQL schema changes remain centralized in the repository `database/migrations` ledger so clean apply, reverse rollback and reapply are verified consistently.

Phase 8A.9 migrations must preserve tenant isolation, FORCE RLS, strict record ownership and rollback/reapply evidence. This directory is the module-local lifecycle pointer required by the module manifest; it must not become an alternate migration ledger.
