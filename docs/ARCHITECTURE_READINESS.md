# Architecture Readiness Gate

Status: **Verification pending on PR #135 exact-head CI**.

The CRM may continue maintenance and integrity work, but new product-module expansion must not merge until the completed integrity mechanisms pass together on one unchanged exact SHA.

## Verified progress in PR #135

The production mutation/query route layer now assembles exact module-owned contributions through `crm-application-composition`. Central mutation planner, capability executor and query routers have been removed from the production path; tenant activation gates wrap business routes; and Accounts, Contact Points, Party Relationships, Consents and Identity Resolution perform cross-owner reference validation before final live authorization.

Background processing now assembles through one deterministic, phase-ordered registry. Import execution, export selection, Sales-to-Activities linking, Sales and Activities projections, Customer 360 projection and global search are module-owned contributions guarded by tenant module activation. `ApplicationComponents` no longer owns fixed worker fields or invokes a hard-coded worker sequence.

Bootstrap query visibility now resolves through declarative module contributions rather than a central `owner_module_id` switch. Data Quality remediation dispatches nested Party mutation through the exact Party executor and verifies Party activation before the nested authoritative operation.

These mechanisms now include authoritative persisted installation lifecycle, exact manifest/binding/production-route parity and golden scaffolding. They do **not** make the architecture ready until one unchanged exact SHA passes every applicable workflow.

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


## Implemented integrity mechanisms

- Production mutations and queries are assembled from exact module-owned contributions.
- Tenant route and worker activation reads `crm.module_installations`; bootstrap provisions durable active installations instead of bypassing lifecycle state.
- Background workers and bootstrap visibility are deterministic contribution registries.
- `production_route_parity` compares compiled production coordinates with governed bindings and explicit exact-route classifications.
- The module scaffold emits `production/CONTRIBUTION.md` so new modules cannot omit the production boundary.

Readiness remains blocked until every applicable workflow is green on one unchanged human-authored SHA.
