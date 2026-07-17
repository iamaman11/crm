# Architecture Readiness Gate

Status: **Blocked until PR #135 completes issue #134**.

The CRM may continue maintenance and integrity work, but new product-module expansion must not merge while production composition still depends on fixed process worker fields, central bootstrap visibility mapping, incomplete route parity or incomplete module lifecycle authority.

## Verified progress in PR #135

The production mutation/query route layer now assembles exact module-owned contributions through `crm-application-composition`. Central mutation planner, capability executor and query routers have been removed from the production path; tenant activation gates wrap business routes; and Accounts, Contact Points, Party Relationships, Consents and Identity Resolution perform cross-owner reference validation before final live authorization.

This milestone does **not** make the architecture ready by itself. Background-worker composition, declarative bootstrap visibility, mechanical manifest/binding/production parity and golden scaffolding remain mandatory.

Architecture readiness is proven only when all of the following hold together on one exact commit:

1. Every in-process module contributes its exact versioned mutation/query routes and background workers through the generic composition boundary.
2. Duplicate coordinates, owner mismatches, missing handlers and route-kind mismatches fail assembly deterministically.
3. Tenant module activation is checked before final live authorization for both mutations and queries.
4. Cross-owner reference reads occur in pre-authorization semantic validation, never as unrelated awaited work inside the authoritative executor.
5. Production route coverage is mechanically equal to the governed manifest/contract surface, except for explicit non-runtime classifications.
6. Background workers are discovered from deterministic module contributions rather than fixed `ApplicationComponents` fields.
7. Published module runtime identity is immutable at the same module version; semantic change requires a version bump.
8. Golden scaffolding creates the production contribution boundary and its acceptance checklist.
9. `python scripts/check_native_module_composition.py` reports no violations.
10. All applicable workflows pass together on one unchanged exact SHA.

No marker may be suppressed or allowlisted merely to make the gate green. The corresponding legacy wiring must be removed through a real module-owned replacement.
