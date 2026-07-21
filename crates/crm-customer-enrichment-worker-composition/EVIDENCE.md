# Worker composition evidence

The non-runtime coordinator is compiled and tested separately from the public capability inventory.

Focused proof covers exact ordering of dispatch commit, provider invocation and response commit; provider suppression after dispatch failure; response suppression after replay-key mismatch; and deterministic response request identity across crash-safe retries.

Production activation remains pending real provider adapters and fresh-PostgreSQL end-to-end process acceptance.
