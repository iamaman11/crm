# Phase 8A Customer Privacy product-plane acceptance

Status: **Repository Step 20A accepted through PR #292; Repository Step 20 remains in progress**

This bounded slice exposes only the already accepted permission-aware Customer Privacy `case.list@1.0.0` and `case.get@1.0.0` queries through the governed product plane. It does not add a backend capability, route, manifest, schema, migration, dependency or owner behavior.

## Boundary

- An authenticated browser route is eligible only when the development/session access snapshot contains `customer_privacy.case.list`.
- Client-side eligibility is a navigation hint. Every request still enters `ApplicationGatewayService.Query` with the live authenticated session and is authorized by the backend.
- The typed client uses exact owner, capability, version, schema, descriptor hash, data class, protobuf encoding, maximum size and retention identity.
- The user must provide an opaque canonical Party reference. The page explicitly warns against entering names, email addresses, passport numbers or other personal data.
- The page renders only bounded case reference, kind, status, version, policy version and timestamps. It does not expose subject-binding evidence, approval actor data, audit envelopes, owner actions, outcomes or internal failure details.
- Permission-denied and not-found errors share the same browser-visible message to preserve concealment.

## Accessibility and browser contract

The accepted Step 20A evidence proves semantic labels and headings, keyboard-only form submission and case selection, polite live announcements, deterministic focus movement after list/detail/error transitions, explicit loading/empty/error/retry states and a real Chromium run against PostgreSQL plus assembled `crm-api`.

## Non-completion statement

Repository Step 20 remains in progress after this slice. Step 20B still owns restore, SLO, observability, performance, security and supply-chain operations evidence. Customer Privacy, Phase 8A.11, Phase 8A, product-complete expert modules, architecture 10/10 and the Universal CRM product remain incomplete. Frozen baselines remain 5,377 public Rust items, 91 suppressions and a 7,269 non-comment/source LOC ceiling for `crm-application-runtime`.

## Accepted Repository Step 20A evidence

PR #292 / source `938cebed1e78bf7debf40dc544431bfe819970f4` / squash merge `fffd6baf35544eea736d183af0a5ba38518cce9a` / 17 of 17 applicable permanent workflows on one unchanged exact head accepts the bounded Customer Privacy product-plane slice.

The accepted evidence proves:

- exact typed `customer_privacy.case.list@1.0.0` and `customer_privacy.case.get@1.0.0` governed clients with envelope, contract, descriptor-hash, data-class, payload-size and retention checks before rendering;
- an authenticated capability-gated `/customer/privacy` route while backend authentication, tenant isolation, authorization and visibility remain authoritative;
- a bounded accessible case list/detail experience with explicit loading, empty, error and retry states, live announcements, deterministic focus behavior and permission/not-found concealment;
- a governed Party and verified PrivacyCase fixture created through assembled production composition and mutations, with no direct Customer Privacy record writes and no mock backend;
- real PostgreSQL, assembled `crm-api`, Vite and Chromium acceptance for keyboard-only list/detail review, session expiry and cross-tenant concealment;
- no backend route, capability, contract, manifest, schema, migration, dependency, lockfile or Rust production-source change.

Step 20A is accepted. Repository Step 20 remains in progress; Step 20B restore, SLO, observability, performance, security and supply-chain operations evidence is the only next permitted packet. Phase 8A.11, Phase 8A, product-complete expert modules, architecture 10/10 and the Universal CRM product remain incomplete. The accepted one-worker Customer Privacy inventory, seven public mutations, four permission-aware public queries, 5,377 public Rust items, 91 suppressions and the `crm-application-runtime` 7,269 LOC ceiling remain unchanged.
