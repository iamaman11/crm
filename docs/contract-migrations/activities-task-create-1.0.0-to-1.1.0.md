# Migrate `activities.task.create` from `1.0.0` to `1.1.0`

## Scope

`activities.task.create@1.0.0` is deprecated on **2026-08-03**. Its replacement is
`activities.task.create@1.1.0`. The earliest permitted removal date is **2026-09-02**,
but reaching that date does not authorize removal by itself.

The replacement is wire-compatible with the deprecated coordinate:

- provider remains `crm.activities`;
- RPC remains `crm.activities.v1.TaskService.CreateTask`;
- request remains `crm.activities.v1.CreateTaskRequest`;
- response remains `crm.activities.v1.CreateTaskResponse`;
- request and response payload schema versions remain `1.0.0`.

Only the exact capability version used for routing changes from `1.0.0` to `1.1.0`.
Callers must not rewrite the protobuf payload schema version to `1.1.0`.

## Governed internal migration

The repository-owned consumer `crm.sales-activities-link` now declares and invokes
`activities.task.create@1.1.0`. Its idempotency key, actor, tenant, authorization,
business transaction, payload encoding and delivery-state behavior are unchanged.

The deprecated `1.0.0` coordinate remains published and executable during the
observation window. Any exact `1.0.0` resolution is exposed through the existing
zero-seeded `crm_deprecated_capability_usage_total` telemetry series.

## Retirement conditions

This migration does **not** retire `1.0.0` and does not establish a zero-usage start
date. A later retirement packet must independently prove all of the following:

1. no live internal or declared external consumers remain;
2. the removal date is on or after `2026-09-02`;
3. production telemetry establishes at least 30 consecutive days of zero exact
   `activities.task.create@1.0.0` usage;
4. the lifecycle registry records immutable migration and zero-usage evidence before
   the old coordinate is removed.
