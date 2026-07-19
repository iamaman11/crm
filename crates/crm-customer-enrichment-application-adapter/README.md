# CRM Customer Enrichment Application Adapter

This non-runtime crate owns deterministic persistence planning for the first governed Customer Enrichment owner-application slice.

It deliberately separates three phases:

1. validate one exact immutable suggestion and accepted review decision;
2. durably create one deterministic pending application attempt before external owner mutation;
3. append one exact success or safe failure outcome after the authoritative owner capability returns.

The crate never writes Party-owned records and never invokes Party adapters directly. The only authoritative target remains `parties.party.update@1.0.0`, which a separately owned composition must call through the governed capability boundary using the attempt's deterministic target idempotency key and exact expected Party version.

Exact replay is handled by capability idempotency. A semantically duplicate outcome submitted under a different idempotency key becomes an audited aggregate no-op; conflicting outcome evidence is rejected.

Neither application coordinate is registered in the production route inventory by this crate.
