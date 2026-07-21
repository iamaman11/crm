# Acceptance gates for `crm.customer-enrichment`

Phase 8A.10 state: **Complete**. Accepted source checkpoint `f92d101206886e3ceaf94d0e56e52580cec21093` passed all 17 permanent workflows unchanged and was squash-merged through PR #137 as `150e44b95d9dbdc08c1792563de03ec73f34aed1`.

## Frozen production inventory

The authoritative inventory in `contracts/customer-enrichment-production-promotion.json` is exactly:

- **6 public mutations**;
- **6 permission-aware queries**;
- **5 activation-gated worker-only coordinates** with no public HTTP/gRPC ingress:
  - provider process: `request.dispatch`, `response.record`;
  - materialization process: `suggestions.materialize`;
  - application process: `party.display_name.apply`, `application.outcome.record`.

All 17 manifest-bound coordinates are now accounted for as public runtime or worker runtime. No completed Customer Enrichment coordinate remains classified non-runtime.

Any inventory change requires a separately reviewed promotion contract and complete parity/process evidence.

## Ownership and architecture

- [x] Immutable module identity and retain-on-uninstall provenance are frozen.
- [x] The module owns enrichment requests and immutable provider/mapping, response, conflict, suggestion, review, usage and application evidence.
- [x] Authoritative Party, Account, Contact Point, Consent, Identity Resolution and Data Quality values remain with their owner modules.
- [x] Accepted Party display-name changes invoke only `parties.party.update@1.0.0` through the governed capability boundary.
- [x] The pure module core contains no PostgreSQL, arbitrary HTTP, provider SDK or secret-store dependency.
- [x] Concrete provider transport, tenant secret resolution and transaction-scoped reference guards are host-owned infrastructure.
- [x] Production composition is module-owned and contains no central business-route switch.

## Domain, persistence and contracts

- [x] Provider-profile and mapping versions are immutable, bounded and content-addressed.
- [x] Requests, response receipts/conflicts, suggestions, reviews, usage and application attempts have deterministic identities and strict canonical persistence.
- [x] All nine manifest-owned record types reject corrupt or non-canonical state.
- [x] Public Protobuf contracts, manifest bindings and descriptor hashes are synchronized.
- [x] Mutations persist atomic state, idempotency, outbox, audit and business-transaction evidence.
- [x] Customer Enrichment tenant tables use ENABLE + FORCE RLS with cross-tenant/no-context denial and rollback/reapply proof.

## Public mutation/query surface

- [x] `customer_enrichment.provider_profile.publish/get@1.0.0`.
- [x] `customer_enrichment.mapping.publish/get@1.0.0`.
- [x] `customer_enrichment.request.create/cancel/get/list@1.0.0`.
- [x] `customer_enrichment.suggestion.get/list_by_party/accept/reject@1.0.0`.
- [x] All five internal coordinates remain activation-gated workers with no public route.
- [x] Query visibility is module-owned, permission-aware and field-redaction capable.
- [x] Hidden Party/provider resources are concealed rather than disclosed through authorization differences.

## Provider, reconciliation and recovery

- [x] Exact adapter kind/version registry and explicit enabled/disabled configuration.
- [x] First concrete registry HTTP transport lives outside the pure module core.
- [x] Endpoint allowlisting, deadlines, bounded bodies, redirect rejection and sanitized network/status/response failures.
- [x] Tenant-bound secret handles without credential value leakage.
- [x] Quota and circuit behavior with fail-closed unknown coordinates and no version fallback.
- [x] Commit-before-provider-I/O and crash-safe replay with the same provider idempotency lineage.
- [x] Independent live dispatch and response authorization.
- [x] Deterministic `New`, `ExactDuplicate` and `SemanticDuplicate` reconciliation.
- [x] Changed canonical response class, digest, metering or evidence fails closed.
- [x] Immutable provider-response conflict evidence and exact replay without duplicates.
- [x] Retain-first and terminal-reject operator resolution evidence.
- [x] Unresolved conflicts stop checkpoint advancement and repeat provider I/O.

## Materialization, review and owner application

- [x] Deterministic materialization over exact request/receipt/profile/mapping and finalized evidence lineage.
- [x] Raw provider payload is never interpreted by the module process.
- [x] Missing/malformed/future evidence stops execution without checkpoint advancement.
- [x] Suggestion supersession and expiry do not leak hidden successors.
- [x] Review uses exact Party version/value digest, approval evidence and immutable decision records.
- [x] Owner application commits a pending attempt before owner I/O and appends one exact outcome.
- [x] Target-success/outcome-missing recovery reuses the same target idempotency lineage.
- [x] Provider, materialization and application workers are activation-gated in deterministic phases 240 → 245 → 250.

## Atomic reference guards

- [x] Mapping publication uses the mapping as its primary aggregate and locks/revalidates the exact immutable provider-profile row inside the same PostgreSQL transaction.
- [x] The provider-profile guard verifies persisted canonical identity and target-field support before mapping persistence.
- [x] Request creation uses the request as its primary aggregate and locks the exact Party row/version inside the same PostgreSQL transaction.
- [x] Reference guards cannot commit, perform external I/O or mutate referenced owner records.

## Production acceptance topology

The permanent acceptance matrix deliberately follows the production boundary:

- the Application Runtime workflow starts the real `crm-api` binary on fresh PostgreSQL and exercises actual HTTP/gRPC ingress for public mutations, queries, visibility, Consent, activation and authorization behavior;
- dedicated fresh-PostgreSQL provider, materialization, review and application process workflows exercise worker-only coordinates directly through the production compositions that the binary registers;
- the production background registry test proves exact phase order 240 → 245 → 250 and durable disable/uninstall shutdown.

This avoids creating a forbidden public ingress path merely for testing worker-only coordinates while still proving their real production composition and persistence behavior.

### Real `crm-api` evidence

- [x] Unauthenticated HTTP returns bounded `401 {"error":"request_failed"}`.
- [x] Party creation, profile publication and mapping publication succeed through real gRPC ingress.
- [x] A legitimate-interest request commits one governed enrichment-request record through the exact Party guard.
- [x] Deployment field ceiling redacts confidential profile definition.
- [x] Cross-tenant profile lookup returns `CUSTOMER_ENRICHMENT_PROVIDER_PROFILE_NOT_FOUND`.
- [x] Tenant outside token grant returns `TENANT_FORBIDDEN`.
- [x] Missing Consent evidence returns `CUSTOMER_ENRICHMENT_REQUEST_CONSENT_DENIED`.
- [x] Live suspension returns `MODULE_NOT_ACTIVE` before semantic/persistence work.
- [x] Bootstrap-disabled live permission returns `CAPABILITY_PERMISSION_DENIED`.
- [x] gRPC returns typed safe code/message and `x-error-retryable=false`; HTTP hides governed details.
- [x] Credential, provider payload and internal diagnostic markers never reach the public surface.
- [x] Request/event/audit/idempotency/business-transaction counters remain unchanged after every pre-persistence denial.

## Governance and merge gate

- [x] `tests/acceptance.rs` is a non-ignored production contract that verifies the exact 6+6+5 inventory, merge evidence and permanent real-process evidence.
- [x] `production/CONTRIBUTION.md` matches production composition, visibility, worker and lifecycle boundaries.
- [x] Module README, catalog entry, roadmap, phase plan and project status are synchronized to the merged state.
- [x] Additional provider transports are explicitly future separately owned infrastructure work and are not hidden in this packet.
- [x] Final synchronized user-authored SHA `f92d101206886e3ceaf94d0e56e52580cec21093` passed all 17 permanent workflows unchanged.
- [x] PR #137 and issue #125 record the exact accepted SHA and squash merge `150e44b95d9dbdc08c1792563de03ec73f34aed1`.
- [x] No unresolved review thread or submitted change request remained at merge.

Phase 8A.10 is a completed production integration slice. This does not make Customer Enrichment or the universal CRM product complete; additional providers, target fields and product UX require separately governed packets.
