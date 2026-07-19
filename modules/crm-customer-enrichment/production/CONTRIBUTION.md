# Production contribution boundary for `crm.customer-enrichment`

This file is a mandatory architecture boundary. The pure module core does not wire itself into the process host and never contains provider SDK, HTTP, credential, SQL or concrete owner-adapter logic.

The exact staged promotion contract is machine-readable at `contracts/customer-enrichment-production-promotion.json`. It is authoritative for the current 4-mutation + 5-query runtime inventory, all eight individually non-runtime coordinates, their route kinds, dependencies and mandatory acceptance evidence. Production coordinates must be promoted in that declared order unless the contract and its validator are changed together on one green exact head.

Before production readiness, separately owned adapter/composition crates must:

- contribute every exact versioned public and worker-only mutation coordinate owned by `crm.customer-enrichment`;
- contribute every exact versioned query coordinate and declarative field-visibility definition;
- perform Party and policy/Consent reads through governed pre-authorization semantic ports;
- resolve provider adapters through an infrastructure-owned exact adapter-kind/version registry without provider switches in generic routers;
- keep credential resolution, arbitrary HTTP, raw payload handling, rate limits and circuit isolation outside the pure module core;
- gate all routes and workers through durable `ModuleActivationPort` / `crm.module_installations` state;
- contribute deterministic bounded workers in explicit dispatch, response-reconciliation, suggestion-materialization/expiry, accepted-suggestion-application and outcome-recovery phases;
- invoke authoritative changes only through exact owner capabilities, initially `parties.party.update@1.0.0`;
- fail startup on duplicate coordinates, owner mismatches or route-kind mismatches;
- register exact manifest/binding/compiled-production-route parity, with individual reasons for any non-runtime coordinate;
- require no customer-enrichment, capability-ID, query-ID, module-ID or provider-specific branch in central routers or worker schedulers;
- prove provider replay, stale evidence, target idempotency, crash recovery, lifecycle disable/uninstall and real-process acceptance.

Every promotion entry must retain the global contract invariants: durable activation gating, module-owned contribution, no central business route switch, provenance retention on uninstall and one unchanged SHA green across all 17 applicable workflows.

Disable/uninstall must stop contributed routes and workers, retain enrichment provenance and leave all existing customer-master owner paths operational.
