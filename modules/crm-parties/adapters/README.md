# Adapter boundary for `crm.parties`

Production capability, query, persistence and event adapters must remain outside the pure `crm.parties` business-module core and depend on it through narrow typed contracts.

Direct SQLx/PostgreSQL, brokers, arbitrary HTTP, secret stores, LLM providers and another business module's internals are forbidden here. Public mutation/query execution must enter through governed application composition.
