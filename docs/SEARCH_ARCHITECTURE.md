# Ultimate CRM — Search Architecture

Status: **Phase 7 foundation**  
Tracked by: issue #66 and parent Phase 7 issue #10.

Search is rebuildable, non-authoritative read state. It may accelerate discovery, ranking and filtering, but it never owns business truth and never becomes an authorization source.

## 1. Runtime boundary

```text
immutable owner-domain events
→ crm-projection-runtime
→ search generation projection documents
→ SearchCandidateStore
→ ranked tenant-scoped candidates + backend match evidence
→ live QueryVisibilityAuthorizer
→ visible search hits
```

The search runtime is intentionally split into two responsibilities:

- the index produces only candidate resource identities, rank, indexed field material and backend-derived match evidence;
- the live query visibility boundary decides whether the current actor may see each resource and field at request time.

No ACL snapshot in the index is authoritative.

## 2. Reindexing

A logical search index has tenant-scoped generations. Every generation uses its own projection identity, so rebuild and incremental catch-up reuse the generalized projection checkpoint/history runtime.

```text
register building generation
→ replay immutable event history through ProjectionRunner
→ write isolated generation documents
→ activate only after successful rebuild
→ retire previous active generation
```

The previously active generation stays queryable while a replacement is building. Search therefore does not need a second replay/checkpoint system and does not require an empty-index cutover during normal reindexing.

The currently active generation cannot be rebuilt in place. Reindexing must target a distinct building generation so a reset or partial replay can never erase or expose an incomplete live index. Generation coordinates are immutable once a generation leaves the building lifecycle state.

## 3. Permission and match-evidence safety

For every candidate, search repeats live visibility before disclosure.

A candidate is omitted when:

- the resource is no longer visible;
- the backend reports no field-local match evidence;
- the query matched only fields that are currently hidden.

`SearchCandidateStore` is responsible for evaluating query syntax under the backend's own matching semantics and returning the exact indexed fields that matched. The runtime must not reinterpret the query with a second matcher. It only intersects backend-provided `matched_fields` with live field visibility before constructing a response.

Match evidence is field-local: the complete backend query must match at least one individual searchable field. Terms split across different fields cannot be combined into a synthetic match. This keeps candidate selection, matched-field disclosure and permission filtering aligned for multi-term and structured query syntax.

The response contains only currently visible fields and only matched-field metadata for currently visible fields. Permission revocation must therefore take effect at query time without waiting for reindexing.

## 4. Tenant isolation

Tenant identity is explicit in:

- search generation registration;
- projection/checkpoint state;
- projection documents;
- candidate queries;
- live visibility checks;
- cursor binding.

PostgreSQL search tables use FORCE RLS where applicable. A tenant cannot activate, enumerate or query another tenant's generation.

## 5. Determinism

The first PostgreSQL adapter uses deterministic ordering:

```text
rank DESC
→ resource_type ASC
→ resource_id ASC
```

Opaque cursors are HMAC-bound to tenant, actor, capability, normalized filter, sort and page size using the existing query-runtime cursor machinery.

## 6. First indexed resources

The first production generation indexes only complete title-bearing snapshot events:

- `sales.deal.created` / `sales.deal.updated` → Deal `name`;
- `activities.task.created` / `activities.task.updated` → Task `subject`.

Partial events such as stage changes, completion and reminder scheduling are not used to reconstruct title state. Future search schemas may expand fields only with explicit visibility semantics and compatible generation/version handling.

## 7. Backend replacement

`SearchCandidateStore` is a stable platform boundary. PostgreSQL full-text search is the first adapter, not a permanent contract. A future external search engine must preserve:

- tenant isolation;
- deterministic cursor semantics;
- rebuildability;
- generation switching;
- backend-consistent field-local match evidence;
- live resource and field visibility before disclosure;
- non-authoritative ownership.
