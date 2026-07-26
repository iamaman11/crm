# PostgreSQL Real-Process Isolation Pilot

Status: Phase E two-shard pilot

This pilot separates two low-coupling `crm-api` real-process suites from the shared sequential job shape so they can execute concurrently without sharing a database, port namespace, mutable fixture state or artifact path.

## Pilot suites

- Party lifecycle: `party_process_e2e`
- Customer Account lifecycle: `account_process_e2e`

Both suites are already accepted production scenarios. Party is the base owner; Account adds authoritative Party-reference integrity while remaining small enough to diagnose independently.

## Isolation model

`PostgreSQL Process Isolation Pilot` uses a two-entry GitHub Actions matrix. Every matrix entry receives:

- a separate hosted runner;
- a separate PostgreSQL 17 service container;
- a unique database name (`crm_process_party_test` or `crm_process_account_test`);
- a service-local mapped port on its own runner;
- independent dynamic HTTP/gRPC ports selected by the process test;
- a suite-specific artifact directory and artifact name;
- independent failure and PostgreSQL diagnostics;
- deterministic job-level cleanup when the ephemeral runner and service are removed.

The database preparation script refuses any database name outside `crm_process_*_test`, verifies `current_database()`, applies the complete ordered migration set and common platform fixtures, and configures only the isolated application role in that database.

## Measurements

Each shard publishes `telemetry.json` with:

- candidate SHA, suite, test target and isolated database;
- explicit PostgreSQL readiness-probe duration;
- database reset/migration/fixture duration;
- Rust process-target compile duration;
- warm process-test execution duration after `--no-run` compilation;
- step outcomes and measured total.

GitHub provisions service containers before normal job steps. The pilot therefore cannot directly measure the hidden container-provisioning interval from shell code; overall workflow/job telemetry remains the source for that outer duration, while the artifact records the explicit readiness probe.

## Safety and acceptance

- The existing sequential Application Runtime CI remains unchanged and continues to prove the complete ordered scenario set.
- The database migration lane remains sequential where migration ordering, role semantics, rollback/reapply or FORCE RLS are the behavior under test.
- The pilot changes no production code, route, contract, migration or process behavior.
- A shard failure does not cancel the other shard, preserving independent diagnostics.
- Final Gate review still requires all applicable permanent workflows on one unchanged candidate SHA.

## Expansion criterion

Expand only after both shards repeatedly prove:

- independent success and reproducible failure diagnostics;
- no shared mutable state or fixed-port collision;
- useful separation of setup, compile and process execution time;
- neutral or improved critical path relative to the sequential control lane;
- manageable additional compute cost.

The first accepted run is proof of correctness and observability, not yet proof of stable performance improvement.
