# Published contract boundary for `crm.customer-privacy`

This directory is an explicit **TODO boundary**, not a published privacy contract.

Before adding behavior that crosses the module boundary:

- define compatible versioned Protobuf commands, queries and events under `proto/crm/customer_privacy/v1`;
- bind every published coordinate to `crm.customer-privacy` through the generated module contract registry;
- preserve the exact architecture-freeze inventory unless a separately reviewed scope change updates all normative controls;
- keep public wire schemas independent from private persisted-state schemas;
- classify every coordinate as public runtime, trusted worker/internal runtime or reasoned non-runtime;
- preserve bounded errors without subject-verification evidence, legal-hold authority material or internal diagnostics.

Do not invent ad-hoc JSON or duplicate generated wire types inside the business module.
