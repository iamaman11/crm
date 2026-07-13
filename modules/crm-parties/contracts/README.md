# Published contracts for `crm.parties`

Canonical source contracts live under the repository `proto/` tree:

- `proto/crm/customer/v1/reference.proto` — cross-owner Party/Account/Contact Point references and shared public version metadata;
- `proto/crm/parties/v1/party.proto` — Party create/get and lifecycle event contracts.

Do not duplicate generated wire types inside this module. Private aggregate state and persistence schemas remain independent from the public Protobuf contract.
