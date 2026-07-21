# Customer Enrichment worker composition

Non-runtime infrastructure coordinator for the provider dispatch lifecycle.

The worker:

- commits the exact pre-I/O `Dispatched` state and RequestDispatched usage evidence;
- invokes one exact kind/version provider adapter through the immutable registry;
- preserves only sanitized response evidence;
- commits the response receipt, usage, request state, idempotency, outbox and audits atomically;
- derives deterministic internal response identities for crash-safe replay;
- does not register either worker capability in the public production inventory.

Production activation still requires real provider adapters and fresh-PostgreSQL end-to-end process acceptance.
