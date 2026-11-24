# Ultimate CRM — Functional Capability Coverage Baseline

Status: **Normative product-scope coverage baseline**  
Parent roadmap: [`IMPLEMENTATION_ROADMAP.md`](IMPLEMENTATION_ROADMAP.md)  
Detailed Phase 8 sequencing: [`PHASE8_DELIVERY_PLAN.md`](PHASE8_DELIVERY_PLAN.md)

## 1. Purpose

This document prevents the platform roadmap from becoming infrastructure-complete but CRM-incomplete.

The target product is a universal modular expert CRM platform. A capability may be implemented by a first-party module, an optional vertical package or a governed integration, but every capability family below must have an explicit owner, integration boundary and lifecycle decision before the product can claim broad CRM completeness.

This is a coverage baseline, not permission to collapse domains into one module. Customer identity, commercial commitments, communications, service, marketing, billing, analytics and platform governance remain separate owner domains with versioned integration boundaries.

## 2. Coverage rules

1. Every authoritative business concept has one explicit owner domain.
2. Cross-domain references use stable typed resource references; downstream modules may keep only explicitly justified snapshots.
3. Mutations use governed capabilities with live authorization, idempotency where applicable, audit evidence and atomic owner-domain state transitions.
4. Search, analytics, caches, AI retrieval and projections are rebuildable and non-authoritative.
5. Metadata may extend declared extension points but may not bypass owner-domain invariants.
6. AI may suggest, summarize, classify or execute approved tools but has no alternate mutation, authorization or consent path.
7. Product completeness requires end-to-end UX, accessibility, localization, import/migration paths, observability and operational evidence, not only backend types.
8. ERP, payment, tax, telephony, messaging, identity-provider and external data-provider concerns remain governed integrations unless CRM ownership is explicitly justified.

## 3. Customer 360 and master data

Owner programs: Phase 8A / #28 and later customer-data extensions.

Required coverage:

- Person and Organization Party identities;
- customer/commercial Accounts referencing parties;
- multiple email, phone, postal, web, social and messaging Contact Points;
- preferred and verified contact methods;
- Party Relationships: employment, household, parent/subsidiary, partner and configurable typed roles;
- account hierarchies and group structures;
- source-system identifiers and reconciliation;
- field-level provenance and survivorship;
- deterministic and explainable duplicate candidates;
- governed merge and unmerge with immutable lineage;
- consent, communication preferences and legal-basis evidence;
- privacy export, deletion, restriction and legal-hold interaction;
- data-quality rules, completeness indicators and stewardship queues;
- governed enrichment with provenance and source policy;
- versioned import/export, mapping, dry run, resumability and reconciliation;
- customer timeline and cross-domain Customer 360 projections;
- configurable customer attributes through governed metadata extension points.

## 4. Sales force automation

Owners: `crm.sales`, `crm.activities` and dedicated sales-performance owners where required.

Required coverage:

- leads/prospects and qualification lifecycle;
- opportunity/deal pipelines and stages;
- account/contact/opportunity relationship roles;
- activities, tasks, calls, meetings, reminders and follow-ups;
- calendar synchronization boundaries and scheduling;
- notes, attachments and timeline activity;
- pipeline views, Kanban, lists, saved views and bulk actions;
- products and line items through Catalog/CPQ references rather than Sales-owned catalog state;
- configurable sales processes and stage-entry/exit rules;
- routing, assignment, queues and workload distribution;
- territory management;
- teams, overlays and account/opportunity team roles;
- quota and target management;
- forecasting, forecast categories, rollups and scenario comparison;
- win/loss reason governance and sales-cycle analytics;
- renewal, expansion and cross-sell opportunity support;
- partner-influenced and channel-sourced opportunity attribution;
- approval processes for discounts and other high-risk commercial actions;
- sales playbooks, sequences and guided next actions;
- mobile and offline-capable field selling workflows where product tier requires them.

## 5. Product, pricing, CPQ and quote-to-revenue

Owner program: Phase 8B / #29.

Required coverage:

- product and service catalog ownership;
- product families, bundles, options, dependencies and compatibility rules;
- catalog versioning and effective dating;
- price books, currencies, price lists and customer/segment-specific pricing;
- discounts, promotions, approval thresholds and exception evidence;
- CPQ configuration and validation;
- quote creation, revisions, comparison, expiry and approval;
- quote line items, taxes/fees as governed inputs or integration results;
- order capture and order lifecycle handoff;
- contracts, amendments, renewals and termination;
- subscriptions, plans, entitlements, usage references and lifecycle changes;
- billing/ERP/payment/tax integration boundaries;
- commercial commitment lineage from opportunity through quote, order, contract and subscription;
- revenue-related analytics without moving accounting ownership into CRM.

## 6. Customer service and support

Owners: dedicated Service/Support, Knowledge, Entitlement and Field Service domains as required.

Required coverage:

- cases/tickets with configurable lifecycle and priority;
- queues, assignment, skills and routing;
- SLAs, milestones, business calendars and breach escalation;
- entitlements, service contracts and warranty references;
- omnichannel case creation and conversation linkage;
- customer, asset/product and order/contract context;
- internal notes versus customer-visible replies;
- macros, templates, guided resolution and automation;
- parent/child, duplicate and major-incident case relationships;
- escalation, swarming and collaboration;
- knowledge-base authoring, review, publication, localization and feedback;
- knowledge suggestion and deflection analytics;
- CSAT/NPS/CES or configurable feedback collection boundaries;
- service analytics, backlog, response/resolution time and SLA reporting;
- self-service portal/API boundaries;
- field service where enabled: work orders, dispatch, skills, territories, appointments, technician mobile/offline workflow and parts/inventory integration boundaries.

## 7. Communications and omnichannel engagement

Owners: dedicated Communications/Conversation domains plus governed provider adapters.

Required coverage:

- email send/receive and threading;
- telephony/call events, recordings and transcription references;
- SMS and messaging channels;
- web chat and bot handoff;
- social/direct-message channel adapters where enabled;
- unified conversation/thread model;
- inbound/outbound message identity and participant resolution;
- templates, signatures and localization;
- attachments and governed file references;
- delivery, bounce, read and provider status events where available;
- communication consent enforcement before send;
- quiet hours, frequency limits and channel preferences;
- agent inbox, queues and assignment;
- searchable interaction history with live authorization;
- provider abstraction, webhook verification, replay safety and delivery reconciliation.

## 8. Marketing automation and growth

Owners: dedicated Marketing domains referencing Customer Master and Communications.

Required coverage:

- campaigns and campaign hierarchy;
- audience segmentation and dynamic segments;
- lists and suppression lists;
- acquisition source and campaign-member history;
- forms, landing-page and event ingestion boundaries;
- lead scoring and qualification models;
- journeys, branching, waits, triggers and goals;
- consent-aware channel activation;
- A/B and multivariate experiment support where applicable;
- marketing content/template references;
- campaign costs and external ad-platform integration boundaries;
- attribution models and touchpoint lineage;
- funnel, cohort and conversion analytics;
- event/webinar registration and attendance integration;
- account-based marketing and buying-group support;
- loyalty/referral programs as optional governed modules rather than hidden campaign fields.

## 9. Customer success, retention and expansion

Owner: dedicated Customer Success domain or explicit Sales/Service link modules; not ad-hoc Account fields.

Required coverage:

- customer lifecycle stage and onboarding plans;
- success plans, objectives and milestones;
- health scores with explainable component evidence;
- adoption and usage-signal integration boundaries;
- risks, alerts and playbooks;
- customer touch plans and business reviews;
- renewal and expansion coordination;
- churn reason taxonomy and retention analytics;
- stakeholder maps and relationship coverage;
- product entitlement/subscription context through stable references.

## 10. Partner and channel relationship management

Owner: optional PRM/Partner domain referencing Party, Account and commercial domains.

Required coverage:

- partner organizations and partner contacts;
- partner tiers, programs, certifications and eligibility;
- deal registration and conflict rules;
- lead/opportunity distribution to partners;
- partner-sourced/influenced attribution;
- channel incentives/rebate integration boundaries;
- partner portal and delegated access boundaries;
- partner performance scorecards.

## 11. Work, projects and configurable operational cases

Owners: dedicated Project/Work Management domains and metadata-driven process definitions.

Required coverage:

- projects, workstreams, milestones and tasks;
- configurable business cases/process instances distinct from support cases;
- dependencies, assignees, teams and due dates;
- checklists and repeatable templates;
- status, risk and issue tracking;
- time/cost integration boundaries where enabled;
- customer-facing project visibility boundaries;
- automation and timeline integration;
- portfolio and delivery analytics.

## 12. Documents, files and e-signature

Owners: governed File/Document domain plus provider adapters.

Required coverage:

- secure file upload/download and malware scanning boundary;
- versioned documents and metadata;
- folders/workspaces or typed document relationships;
- document generation from governed templates;
- merge fields with authorization-aware data access;
- e-signature envelope, signer and status integration;
- immutable signed-document references and evidence;
- retention, legal hold and deletion policy interaction;
- preview/search/OCR as rebuildable derived capabilities, not authoritative file mutation.

## 13. Analytics, reporting and performance management

Owners: analytical platform plus domain-owned semantic contracts.

Required coverage:

- operational dashboards and KPIs;
- configurable reports with permission-aware semantic fields;
- drill-down to live-authorized source resources;
- funnels, cohorts, retention and lifecycle analytics;
- sales pipeline and forecast analytics;
- service/SLA and workforce analytics;
- marketing attribution and campaign analytics;
- customer success and churn analytics;
- territory, quota and performance scorecards;
- scheduled report delivery with authorization re-checks;
- export controls and data-class policy;
- warehouse/lakehouse/BI integration boundaries;
- metric definitions, lineage, freshness and reproducibility;
- no analytical projection becoming an undocumented system of record.

## 14. Workflow, automation and orchestration

Owners: governed workflow/platform runtime plus domain capabilities.

Required coverage:

- trigger, condition, branch, wait, timer and schedule semantics;
- exact governed capability invocation as workflow actions;
- approvals and human tasks;
- retries, idempotency and compensation/recovery strategies;
- versioned workflow definitions and immutable execution evidence;
- business calendars and SLA timers;
- event-driven and scheduled automation;
- integration actions through allowlisted governed connectors;
- no arbitrary SQL, secret access, unrestricted HTTP or hidden mutation bypass;
- simulation/test mode and execution observability.

## 15. Collaboration and personal productivity

Required coverage:

- notifications and notification preferences;
- mentions, comments and collaboration threads where appropriate;
- shared/team queues and work lists;
- personal task/calendar views;
- activity feed and recent items;
- favorites/pins;
- command palette and global navigation;
- saved views, filters and column configurations;
- bulk actions with preview, authorization and partial-failure semantics;
- imports and guided onboarding;
- keyboard accessibility and productivity shortcuts.

## 16. Search and knowledge discovery

Owners: rebuildable Search/Projection infrastructure with domain-owned indexing contracts.

Required coverage:

- global search across authorized CRM resources;
- domain-specific search and filtering;
- typo tolerance/relevance adapters where enabled;
- permission changes enforced at disclosure time;
- deterministic pagination and rebuild/switch generations;
- recent/popular suggestions as non-authoritative projections;
- knowledge and document discovery;
- semantic/vector retrieval only through permission-filtered governed paths;
- deletion/restriction/consent effects propagated with defined freshness guarantees.

## 17. Data platform and integrations

Required coverage:

- versioned bulk import/export;
- API and webhook platform;
- inbound webhook verification and replay protection;
- outbound delivery retry, reconciliation and dead-letter handling;
- connector lifecycle, credentials and secret-handle boundaries;
- mapping/transformation with versioned schemas;
- sync cursors, conflict strategy and source-of-truth declarations;
- data enrichment with provenance;
- iPaaS/event-stream/warehouse integration boundaries;
- ERP, finance, payment, tax, identity, telephony, messaging and ad-platform adapters;
- rate limits, quotas and tenant isolation for integration workloads.

## 18. Administration, customization and low-code governance

Owners: Admin Studio, metadata runtime and module platform.

Required coverage:

- custom objects and fields through governed typed schemas;
- layouts, related lists, saved views and pipelines;
- validation rules and business process configuration;
- permission templates and role/team administration;
- workflow authoring through governed actions;
- immutable publication, impact analysis, activation and rollback;
- module install/upgrade/suspend/uninstall lifecycle;
- tenant settings, branding, localization and feature policy;
- audit views and administrative diagnostics;
- sandbox/test environment strategy and controlled promotion between environments;
- no raw production JSON/SQL/script escape hatch for critical invariants.

## 19. Identity, authorization, privacy and enterprise governance

Owner program: continuous hardening and Phase 11 production proof.

Required coverage:

- authentication with OIDC/SAML where required;
- SCIM provisioning and deprovisioning;
- tenant, organization, team, role and resource authorization;
- field-level visibility and masking;
- delegated administration and separation of duties;
- approval policies for high-risk actions;
- tenant key hierarchy and field/data-class encryption where required;
- immutable audit evidence and WORM export;
- consent and communication authorization enforcement;
- privacy access/export/deletion/restriction/legal-hold workflows;
- data residency and tenant mobility;
- backup, PITR, tenant restore and disaster recovery;
- retention and crypto-shredding strategies;
- security scans, SBOM, provenance, penetration and abuse testing;
- SLOs, alerting, incident response and operational runbooks.

## 20. AI-native CRM

Owner program: Phase 9.

Required coverage:

- AI as an authenticated audited Actor;
- tenant/purpose/data-class/residency/cost-aware model routing;
- tool schemas generated from governed capability/query registries;
- live authorization before retrieval and before every side effect;
- permission-filtered retrieval and grounding;
- summarization, drafting, classification, extraction and recommendation;
- next-best-action and risk/opportunity suggestions with explanations;
- duplicate/match suggestions without autonomous identity merge;
- human approval for configured high-risk actions;
- prompt-injection, data-leakage, hallucination and tool-correctness evaluations;
- budget, latency and provider-failure controls;
- complete actor/tool/model/prompt-policy/cost audit evidence;
- reversible actions where the underlying domain supports reversal.

## 21. Marketplace and ecosystem

Owner program: Phase 10.

Required coverage:

- signed packages and publisher identity;
- dependency and compatibility resolution;
- SBOM/provenance and vulnerability policy;
- explicit capability/data/network/secret grants;
- sandboxed untrusted execution, planned as WASM;
- quotas, timeouts and resource limits;
- install/upgrade/rollback/suspend/uninstall evidence;
- kill switch and emergency revocation;
- marketplace UI extensions through bounded host-owned context;
- extension failure isolation and no infrastructure bypass.

## 22. Product experience and delivery surfaces

Required coverage across domain waves, not as a final cosmetic phase:

- responsive desktop and mobile web experience;
- accessible navigation, forms, tables, dialogs and charts;
- localization, locale-aware dates/numbers/currency and RTL readiness where required;
- fast list, record, timeline and workspace experiences;
- empty/loading/error/offline/retry states;
- onboarding and contextual guidance;
- mobile-native or installable/offline experience where target deployment requires it;
- notification surfaces and deep links;
- safe optimistic UX only where server authority remains explicit;
- browser E2E against real governed application paths for critical workflows.

## 23. Vertical and optional extension model

The universal core must support industry packages without forcing every tenant to install every domain. Examples include healthcare, financial services, real estate, education, nonprofit, hospitality and public-sector packages.

Vertical packages must use the same module, capability, authorization, audit, metadata and marketplace rules. They may add owner domains and workflows but may not bypass canonical customer identity, privacy, tenant isolation or governed mutation boundaries.

## 24. Completion accounting

A capability family is not considered complete merely because a crate or schema exists.

For product readiness, each applicable family must be classified as one of:

- **Production-complete** — authoritative owner, governed API, persistence, authorization, audit, end-to-end UX and acceptance evidence exist;
- **Platform-ready** — reusable platform boundary exists but expert domain/product behavior is incomplete;
- **Planned** — explicit owner and delivery wave exist;
- **Optional/vertical** — intentionally not universal but supported through the governed module model;
- **External integration** — CRM owns the orchestration/reference boundary, not the external system of record.

`PROJECT_STATUS.md` and `MODULE_CATALOG.md` remain the current-state summaries. This document is the scope-completeness guardrail: no claim of an "ultimate" or broadly complete CRM is valid while a required capability family above has neither production implementation nor an explicit planned owner/boundary.
