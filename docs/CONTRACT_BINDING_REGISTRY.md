# Module Contract Binding Registry

Status: **Normative contract-generation architecture**

## Purpose

`contracts/module-contract-bindings.json` is a generated, machine-readable index that joins a module-owned versioned contract identity to its exact Protobuf wire surface.

For a capability it records:

- module owner;
- capability ID and semantic version;
- fully qualified Protobuf RPC;
- exact request message;
- exact response message.

For an event it records the module owner, event ID/version and exact Protobuf payload message.

The registry is not product configuration, UI configuration, runtime installation state or an independently authored source of truth.

## Sources of truth

Each authoring `modules/*/module.yaml` owns the stable capability/event identity and its build-time binding:

```yaml
provides:
  capabilities:
    - id: parties.party.update
      version: 1.0.0
      binding:
        kind: protobuf_rpc
        rpc: crm.parties.v1.PartyService.UpdateParty
        request: crm.parties.v1.UpdatePartyRequest
        response: crm.parties.v1.UpdatePartyResponse
  events:
    - id: parties.party.updated
      version: 1.0.0
      binding:
        kind: protobuf_message
        message: crm.parties.v1.PartyUpdatedEvent
```

The compiled Protobuf descriptor set is the source of truth for whether those services, methods and messages exist and for the actual RPC input/output types.

The JSON registry is deterministically generated from those two sources. It must never be edited manually.

## Layering

Binding coordinates are authoring/build metadata. `scripts/module_manifest_projection.py` removes them while compiling normalized runtime module IR. Runtime module identity therefore contains only the stable versioned capability/event IDs and remains independent from repository-level Protobuf file or service organization.

This separation provides both:

- strict repository contract integrity;
- a compact runtime manifest without build-system details.

## Generation and verification

Generate locally:

```bash
python scripts/generate_contract_bindings.py --write
```

Verify without changing files:

```bash
python scripts/generate_contract_bindings.py --check
```

A prebuilt descriptor may be supplied in CI:

```bash
python scripts/generate_contract_bindings.py \
  --check \
  --descriptor-set contract-diagnostics/crm-schema.binpb
```

`pnpm web:generate` also regenerates the registry so all generated client/contract artifacts use one entrypoint.

## Permanent invariants

Contract CI fails when any of the following occurs:

- a module manifest is omitted from the generated registry, including modules with zero provided contracts;
- a capability/event binding is absent or has the wrong binding kind;
- a referenced RPC or message does not exist;
- an RPC input/output differs from the manifest declaration;
- a capability/event ID and version has more than one provider;
- the committed generated JSON differs byte-for-byte from canonical output;
- Protobuf formatting, lint or breaking checks fail.

Adding a module or publishing a contract is therefore one atomic authoring action: the manifest declaration, Protobuf surface and generated registry must agree on the same commit.
