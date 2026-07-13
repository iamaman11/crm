# CRM Parties

Authoritative owner-module foundation for `crm.parties`.

`crm.parties` owns canonical person and organization identity. Sales, Service, Marketing, Billing and other domains may reference published Party identities but may not define a competing customer identity master or access Party storage directly.

This packet establishes the stable module identity and versioned public contracts only. The production Party aggregate, persistence, governed adapters and process-level acceptance are delivered in the follow-on 8A.2 vertical slice.
