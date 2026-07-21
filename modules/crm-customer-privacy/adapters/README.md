# Adapter boundary for `crm.customer-privacy`

This directory records an explicit **TODO architecture boundary**.

Production capability, query, visibility, persistence, owner-contribution, enforcement and worker adapters must remain outside the pure business-module core and depend on it through narrow typed contracts.

Required rules:

- PostgreSQL and transaction-scoped subject locks belong in infrastructure/composition crates;
- cross-owner semantic reads occur before final authorization;
- the authoritative restriction decision is reloaded while the shared subject lock is held;
- no adapter writes another module's storage or invokes another module's private implementation;
- public and worker routes enter production only through module-owned exact-coordinate contributions and durable activation;
- privacy module inactivity or an unavailable/stale decision source must fail protected processing closed;
- projections, search, caches and export artifacts are never treated as authority.

Do not add SQLx, arbitrary HTTP, brokers, secret stores, schedulers or another business module's internals to the module crate.
