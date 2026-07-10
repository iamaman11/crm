# Ultimate CRM — System Invariants v1

Status: **Normative**  
Applies to: platform core, first-party modules, marketplace modules, workflows, AI actors, integrations, UI extensions, infrastructure and operational tooling.

The keywords **MUST**, **MUST NOT**, **SHOULD** and **MAY** are normative. A component that violates a MUST or MUST NOT is not conformant with the platform.

## 1. Authority and sources of truth

1. Every concept MUST have exactly one authoritative representation.
2. Protobuf is authoritative for RPC, command and event contracts.
3. SQL migrations are authoritative for PostgreSQL schema evolution.
4. A published typed Module Manifest IR is authoritative for an installed module. `module.yaml` is only its human-authored source.
5. CEL is authoritative only for deterministic expressions explicitly owned by a versioned policy or workflow.
6. PostgreSQL transactional state and the canonical event/audit records are authoritative runtime state.
7. Search, analytics, timelines, caches and projections MUST be rebuildable and MUST NOT become sources of truth.
8. OpenAPI, JSON Schema, SDKs and documentation MUST be generated from, or mechanically checked against, their authoritative contracts.

## 2. Governed execution

9. Every state-changing operation MUST execute as an identified Actor invoking a versioned Capability with a complete Execution Context.
10. Users, services, modules, workflows, integrations and AI actors MUST NOT mutate business state through an alternate internal path.
11. Live authorization MUST run immediately before every side effect. A previous policy snapshot or approval MUST NOT replace the live check.
12. An approval MUST bind actor, tenant, capability version, semantic input hash, data classes and expiry.
13. Each externally observable side effect MUST be authorized, idempotent, traced and auditable.

## 3. Module ownership and independence

14. Every mutable aggregate type MUST have exactly one owning module.
15. A module MUST NOT modify another module's aggregate directly.
16. Business modules MUST communicate only through versioned capabilities, versioned events and stable resource references.
17. Business modules MUST NOT import another business module's source code, internal Rust types, tables or repositories.
18. Business modules MUST NOT receive raw PostgreSQL, NATS, object storage, secret-store, arbitrary HTTP or LLM-provider clients.
19. Infrastructure access MUST be exposed through governed, tenant-aware SDK ports.
20. Cross-domain behavior SHOULD be implemented in a link module rather than by introducing bidirectional dependencies.
21. Module dependency graphs MUST be acyclic.
22. Every module MUST declare ownership, provided and consumed contracts, storage, permissions, lifecycle and uninstall behavior.
23. Every module MUST be independently buildable and testable after platform contracts are available.

## 4. Versioning and publication

24. Capabilities, events, modules, metadata packages, workflows, policies, UI extensions and canonicalization profiles MUST be versioned.
25. Published versions MUST be immutable.
26. A semantic change MUST create a new version; changing meaning under an existing version is forbidden.
27. Removed protobuf field numbers and names MUST be reserved.
28. Breaking contract changes MUST be detected in CI before merge.
29. Configuration changes MUST follow draft → validate → impact report → approval → publish → activate, with rollback where supported.

## 5. Transaction and event correctness

30. A logical mutation MUST atomically persist business state, aggregate version, idempotency result, outbox events and audit reference in one database transaction.
31. A committed business mutation MUST NOT exist without its corresponding durable event and audit evidence.
32. Event consumers MUST be idempotent.
33. Event envelopes MUST carry tenant, actor, capability, schema, correlation, causation and trace identity.
34. Ordering guarantees MUST be explicit and scoped; global ordering MUST NOT be assumed.
35. Replay MUST use the historical schema and policy context required to interpret the record, without silently applying current semantics.

## 6. Tenant and data isolation

36. Every persisted business row, cache key, object path, event, projection and search document MUST be tenant-scoped unless explicitly platform-global.
37. PostgreSQL tenant isolation MUST be enforced with RLS and tenant-bound transaction context, not only application predicates.
38. Cross-tenant negative tests MUST exist for every storage and query boundary.
39. Encryption keys, retention, export and deletion policies MUST be tenant-aware and data-class-aware.
40. Sensitive payloads MUST be minimized and referenced rather than copied into events, logs or audit records.
41. Secrets, credentials, biometric payloads and raw protected documents MUST NOT appear in logs or audit envelopes.

## 7. Typed data and deterministic behavior

42. Schemaless runtime data is forbidden. Every JSON, JSONB, `bytes`, map or extension payload MUST identify owner, schema, version, data class, size limit and retention policy.
43. Money and other exact decimal values MUST NOT use binary floating-point.
44. Time, randomness and external responses used by domain decisions MUST enter through controlled ports and be traceable inputs.
45. Error handling MUST use stable typed error codes; external behavior MUST NOT depend on parsing error text.
46. Null, absence, default and deletion semantics MUST be explicit in contracts.
47. Limits for payload size, nesting, collection length and execution duration MUST be explicit and enforced.

## 8. Format policy

### Protobuf

Protobuf MUST be used for typed machine contracts, RPC, commands, events and storage envelopes. Raw protobuf serialization MUST NOT be treated as a permanent canonical representation for signatures, audit hashes or semantic idempotency hashes.

### JSON

JSON MAY be used for REST, webhooks, UI descriptors, structured logs, JSON Schema, imports/exports and normalized intermediate representations. It MUST be schema-bound. Duplicate keys, non-finite numbers and unknown properties MUST be rejected unless a schema explicitly permits extension properties.

### YAML

YAML MAY be used only as a human authoring format. Accepted YAML is restricted to a JSON-compatible YAML 1.2 subset. The following are forbidden:

- anchors and aliases;
- merge keys;
- custom tags;
- duplicate keys;
- non-string map keys;
- implicit dates or timestamps;
- non-finite or floating-point numbers in governance manifests;
- executable expressions or environment interpolation embedded in domain configuration.

YAML MUST be parsed strictly, converted to a plain JSON-compatible tree, validated with JSON Schema, semantically validated, normalized to typed IR and published as an immutable version. Runtime components MUST NOT execute raw YAML.

### Other approved formats

- SQL: migrations and platform-owned data access only.
- TOML: Rust and developer tooling only.
- CEL: deterministic, side-effect-free expressions only.
- Markdown: documentation and ADRs only; never runtime truth.
- WASM: signed sandboxed extension execution only.
- HCL: infrastructure as code only.
- CSV and Parquet: import, export and analytics boundaries only.

Arbitrary JavaScript, Python, Lua, shell or dynamic SQL execution inside business configuration is forbidden.

## 9. Canonicalization and hashing

48. Stable signatures, audit-chain material, package identities and semantic idempotency hashes MUST use an explicitly versioned canonicalization profile.
49. Canonicalization Profile `crm.cjson/v1` accepts only JSON-compatible objects with string keys, arrays, strings, booleans, null and bounded integers.
50. Object keys in canonical material MUST be ASCII identifiers and sorted lexicographically; insignificant whitespace is removed; UTF-8 is used; non-finite and floating-point values are forbidden.
51. The canonicalization profile identifier MUST be stored beside every digest or signature.
52. Changing canonicalization rules requires a new profile version and MUST NOT reinterpret existing hashes.

## 10. Transparency and operations

53. Every request and side effect MUST be traceable through request, correlation, causation and trace identifiers.
54. Audit records MUST explain who did what, through which capability, against which tenant and records, under which policy decision.
55. Audit integrity MUST use a canonical audit envelope and an append-only chain separate from business-event encoding.
56. Operational overrides and break-glass actions MUST be time-limited, explicitly authorized and more strongly audited than ordinary actions.
57. Backups, restores, tenant exports and tenant moves MUST preserve referential, event and audit consistency.
58. Production readiness requires measured performance, security testing, restore drills and SLO evidence; documentation alone is insufficient.

## 11. Conformance gates

A change MUST NOT merge unless applicable gates pass:

- protobuf build, lint and breaking-change checks;
- strict module-manifest parsing and schema validation;
- module dependency cycle and duplicate-provider checks;
- architecture dependency-boundary checks;
- formatting, linting and automated tests;
- migration forward and rollback/compensation review;
- security and data-class impact review for privileged capabilities;
- reproducible artifact digest generation.

Exceptions require an ADR with an owner, expiry date, migration plan and explicit risk acceptance. Permanent undocumented exceptions are forbidden.
