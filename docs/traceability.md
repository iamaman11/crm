# Requirement Traceability Matrix

| Requirement | ADR | Contract | Implementation | Test | Metric | Runbook | Status |
|---|---|---|---|---|---|---|---|
| MOD-001 Modules cannot access DB directly | ADR-004 | capability.proto | scripts/check_architecture.py | Architecture CI | CI pass rate | — | Skeleton |
| EVT-001 Mutations use transactional outbox | ADR-005 | event.proto | crm-core-events | Pending | outbox lag | outbox-relay-stuck | Planned |
| AUD-001 Canonical audit hash chain | ADR-011, ADR-017 | audit.proto | crm-core-events | Pending | verification failures | audit-hash-mismatch | Planned |
| SEC-001 Default deny and field filtering | ADR-004 | policy.proto | crm-core-permissions | Pending | denied/masked calls | permission-incident | Planned |
| WF-001 Live authorization before action | ADR-016 | workflow.proto | crm-core-workflow | Pending | auth rejections | workflow-dlq | Planned |
