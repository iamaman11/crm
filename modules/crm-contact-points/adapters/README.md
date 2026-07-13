# Adapter boundary for `crm.contact-points`

Future Contact Point capability/query/persistence/verification adapters remain outside the pure owner-module core. Direct database, broker, arbitrary HTTP, secret-store, LLM-provider and cross-module internal access are forbidden.

Provider-specific verification or enrichment must enter through governed integration adapters and preserve provenance rather than becoming authoritative infrastructure access inside this module.
