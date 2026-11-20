# CRM Customer Enrichment Review Adapter

Non-runtime deterministic planning for `customer_enrichment.suggestion.accept@1.0.0` and `customer_enrichment.suggestion.reject@1.0.0`.

The crate is not registered in production composition. The production Customer Enrichment inventory remains exactly four mutations and four queries.

The planner receives one strictly rehydrated immutable suggestion plus the resolved acceptance-approval requirement. Before persistence it verifies the exact suggestion reference, Party resource version, proposed-value digest, decision timestamp, expiry and required approval evidence. A successful plan atomically creates one immutable review decision, one reviewed event, one audit and capability-idempotency evidence.

Policy lookup, Party reads, live authorization and production registration remain outside this crate.
