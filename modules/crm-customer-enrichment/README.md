# CRM Customer Enrichment

Governed **provider-neutral owner/coordinator module foundation** for `crm.customer-enrichment`.

The module owns enrichment request, immutable provider/mapping definition, response-receipt, suggestion/provenance, review-decision, provider-usage and owner-capability application-attempt evidence. It does not own authoritative Party, Account, Contact Point, Consent, Identity Resolution or Data Quality values.

The first production slice is limited to reviewed Party display-name suggestions applied only through exact capability `parties.party.update@1.0.0` after exact-version revalidation, policy/approval and final live authorization.

This foundation is intentionally not a production feature. Complete the explicit gates in `ACCEPTANCE.md`, the frozen architecture in `../../docs/PHASE8A10_CUSTOMER_ENRICHMENT_ARCHITECTURE.md` and the guardrails in `../../docs/PHASE8A10_CUSTOMER_ENRICHMENT_GUARDRAILS.md` before raising readiness.

Direct PostgreSQL, broker, arbitrary HTTP, provider SDK, secret-store and cross-module internal dependencies are forbidden in the pure module core.
