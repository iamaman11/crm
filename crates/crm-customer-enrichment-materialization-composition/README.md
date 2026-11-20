# Customer Enrichment materialization composition

Non-runtime PostgreSQL coordinator for deterministic suggestion materialization.

The coordinator reads and strictly validates the immutable provider response receipt, provider profile and mapping in tenant context, then executes one atomic request-and-suggestions mutation plan. It is intentionally not registered in the public production capability inventory.
