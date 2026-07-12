# crm-metadata-schema

Strict typed v1 schemas for Admin Studio metadata authoring.

## Boundary

This crate converts validated typed definitions into canonical `crm-metadata-runtime::MetadataDocument` values. It owns authoring-time schema rules and deterministic dependency extraction for:

- objects;
- fields;
- relationships;
- layouts;
- saved views;
- pipelines;
- permission templates;
- governed-capability workflows.

It does not own persistence, live authorization, transport, frontend state, business owner-domain invariants or arbitrary code execution.

## Safety model

- unknown fields are rejected by definition/config structs;
- identifiers, labels, collection sizes and numeric bounds are validated;
- duplicate members are rejected rather than silently normalized away;
- set-like members are sorted before canonical serialization;
- meaningful authoring order remains part of canonical identity;
- dependency extraction feeds bundle-level missing-dependency validation;
- workflow actions reference exact SemVer governed capabilities only;
- no script, raw SQL or arbitrary HTTP action exists in the schema surface.

Future persistence and Admin Studio APIs must consume these typed schemas instead of accepting opaque unvalidated metadata payloads.
