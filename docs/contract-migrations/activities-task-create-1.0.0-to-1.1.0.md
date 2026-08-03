# Migrate `activities.task.create` from `1.0.0` to `1.1.0`

## Scope

`activities.task.create@1.0.0` was deprecated and retired on **2026-08-03**. Its
replacement is `activities.task.create@1.1.0`.

The replacement is wire-compatible with the retired coordinate:

- provider remains `crm.activities`;
- RPC remains `crm.activities.v1.TaskService.CreateTask`;
- request remains `crm.activities.v1.CreateTaskRequest`;
- response remains `crm.activities.v1.CreateTaskResponse`;
- request and response payload schema versions remain `1.0.0`.

Only the exact capability version used for routing changes from `1.0.0` to `1.1.0`.
Callers must not rewrite the protobuf payload schema version to `1.1.0`.

## Governed internal migration

The repository-owned consumer `crm.sales-activities-link` declares and invokes
`activities.task.create@1.1.0`. Its idempotency key, actor, tenant, authorization,
business transaction, payload encoding and delivery-state behavior are unchanged.

`crm.activities@0.1.2` removes the retired `1.0.0` coordinate from the current module
manifest. The PostgreSQL acceptance registry keeps the historical `0.1.0`, `0.1.1`
and `activities.task.create@1.0.0` publication rows while registering the successor
module version. Historical registry evidence is not a live provider declaration.

## Never-externally-released retirement

The normal lifecycle boundary remains **2026-09-02** and the normal telemetry
lookback remains 30 days. It was not shortened or backdated. Instead, the narrower
`never_externally_released` path was used because the coordinate had no external
consumer record and the project had no GitHub release, Git tag or deployment at the
retirement checkpoint.

The immutable evidence record is
`activities-task-create-1.0.0-never-released-2026-08-03`. It is bound by SHA-256 to
a repository artifact that records:

- empty GitHub Releases, tags and deployments snapshots;
- exact source commit `996d634a33945e618be5ff81c297f0f617ce19d5`;
- `publish = false` for the Activities owner and capability-adapter packages;
- the repository owner attestation and tracking issue;
- the exact retired capability coordinate.

Contract CI verifies newly introduced never-released evidence against the live GitHub
API before merge. The lifecycle tombstone and evidence then become append-only and
immutable. `telemetry.zero_since` remains `null`; no production zero-usage history was
fabricated.

## Result

`activities.task.create@1.0.0` is no longer published by the current module manifest,
is absent from the production capability catalog and is rejected before payload
decoding. `activities.task.create@1.1.0` remains the sole live create coordinate.
