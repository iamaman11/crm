# Production contribution boundary for crm.customer-enrichment

Current production inventory: 6 public mutations, 6 permission-aware queries, 1 activation-gated application worker, and 4 individually non-runtime coordinates.

The exact staged contract remains contracts/customer-enrichment-production-promotion.json. Production code must preserve module-owned contribution, durable activation gating, governed Party and Consent reads, exact owner capability invocation, deterministic replay and one unchanged 17-workflow acceptance head.
