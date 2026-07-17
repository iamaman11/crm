# Adapter boundary for `crm.customer-enrichment`

Production persistence, query, capability, provider, secret-resolution and owner-capability adapters remain outside the pure business-module crate and depend on it through narrow typed contracts.

Required separated responsibilities:

- PostgreSQL persistence and FORCE RLS;
- exact mutation/query planning and decoding;
- Party snapshot/version validation and exact owner-capability invocation;
- policy/Consent evidence reads;
- provider adapter registry and sanitized canonical provider results;
- credential handle resolution without exposing secret material to the core;
- governed protected-payload file/evidence storage when retention policy permits;
- deterministic worker and lifecycle composition.

Do not add SQLx, PostgreSQL clients, brokers, arbitrary HTTP clients, provider SDKs, secret stores or another business module's internals to this module crate.
