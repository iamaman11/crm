# CRM Search Runtime

`crm-search-runtime` provides tenant-scoped, rebuildable search mechanics for the Ultimate CRM platform.

## Security model

The search index is **candidate-only**. Indexed documents never contain an authoritative ACL snapshot. Every candidate is checked through the live `QueryVisibilityAuthorizer` before the runtime returns:

- resource identity;
- field values;
- matched-field metadata.

A candidate is suppressed when the current actor cannot see the resource or when the query matched only fields that are currently hidden.

## Reindex model

Search documents are stored as generalized projection documents. Each search generation has its own projection identity and therefore reuses the shared projection checkpoint/replay runtime.

The reindex lifecycle is:

```text
register building generation
→ rebuild through ProjectionRunner
→ activate generation only after successful replay
→ retire the previous active generation
```

The previous generation remains queryable while a replacement is building. No second search-specific replay/checkpoint mechanism exists.

## Backend boundary

`SearchCandidateStore` is replaceable. The first adapter uses PostgreSQL full-text search over rebuildable projection documents, while the runtime contract keeps ranking, cursor binding and live permission filtering independent from PostgreSQL implementation details.
