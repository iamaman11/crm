# Adapter boundary for `crm.customer-accounts`

Future Account capability/query/persistence adapters remain outside the pure owner-module core. Direct database, broker, arbitrary HTTP, secret-store, LLM-provider and cross-module internal access are forbidden.

Cross-domain validation and coordination must use governed contracts and capabilities rather than table reads or module imports.
