# Checkpoint

Implementation checkpoint `f1f03aa1d50056a3e00b71956c7f2a98da9389f3` is green across all 17 applicable workflows. The following evidence-only commits do not expand production registration.

Delivered at this checkpoint:

- resource-specific `SuggestionReviewPolicyPort` and PostgreSQL review-policy composition;
- exact suggestion, Party version and proposed-value digest binding;
- mandatory approval evidence when policy requires it;
- one atomic immutable review decision with idempotency, outbox and audit evidence;
- exact replay without duplicate records, events, audits, idempotency rows or transactions;
- permission-aware `suggestion.get` and `suggestion.list_by_party` process proof;
- Party/profile/status filtering, not-found hiding for get and empty-page hiding for list;
- proof that reads create no records, events, audits or transactions;
- capability-specific bootstrap visibility for the existing four production enrichment queries;
- current-schema fresh-PostgreSQL review fixture and canonical cursor-codec construction;
- deterministic pending application-attempt persistence before external I/O and append-once outcome persistence afterwards;
- strict application-attempt record versions `1 → 2`, exact replay without duplicates, audited semantic-duplicate no-op behavior and fail-closed conflicting-outcome rejection;
- fresh-PostgreSQL application process proof over exact suggestion/review lineage;
- governed owner boundary that invokes only `parties.party.update@1.0.0` through `CapabilityClient` with deterministic target idempotency and business-transaction lineage;
- ordinary Party authorization, semantic validation and optimistic locking remain authoritative;
- typed Party response, identity, expected-version increment and affected-resource evidence validation;
- stale-version resolution through governed `PartySnapshotPort`, without parsing error text.

Production inventory remains exactly **4 mutations + 4 permission-aware queries**. Suggestion review/query and application coordinates remain non-runtime. The next implementation block is final owner-application policy orchestration, target-success/outcome-missing recovery, remaining provider failure scenarios and activation-gated production composition.
