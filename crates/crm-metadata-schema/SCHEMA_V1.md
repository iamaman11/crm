# Metadata definition schema v1

Canonical schema profile: `crm.metadata.definition/v1`.

## Canonicalization rules

- Definition structs use a fixed typed field layout.
- Set-like members are validated for duplicates and sorted before serialization.
- Meaningful authoring order, such as layout sections, fields within a section, pipeline stages and workflow actions, remains part of canonical identity.
- Canonical bytes are UTF-8 JSON emitted from the normalized typed representation.
- Those bytes are passed to `crm-metadata-runtime`, which computes the immutable bundle revision identity under its own versioned SHA-256 profile.

## Dependency rules

- fields depend on their owning object and referenced target object where applicable;
- relationships depend on source and target objects;
- layouts depend on their object and referenced fields;
- saved views depend on their object and referenced fields;
- pipelines depend on their object and stage field;
- permission templates depend on their object and referenced fields;
- workflows reference governed capability contracts rather than metadata definitions.

Bundle publication remains responsible for rejecting missing metadata dependencies.

## Execution safety

Schema v1 deliberately exposes no raw script, SQL, shell command or arbitrary HTTP action. Workflow actions are exact versioned governed capability references and remain subject to the normal runtime authorization and execution path.
