# Ultimate CRM — Passport Recognition and Identity Documents 10/10 Plan

Status: **Expert product and architecture design proposal**  
Scope: customer/Party identity-document capture, recognition, verification evidence, review, lifecycle and product UX  
Primary owner: `crm.parties`  
Supporting platform boundaries: governed files, capability/query runtime, Customer 360, Customer Privacy, Identity Resolution, product plane

This document defines the full end-state design for passport recognition in the customer card. It intentionally does **not** define an artificially reduced MVP. Implementation may be delivered in bounded packets to preserve repository governance, but the target capability is global, expert-grade and functionally complete.

It does not override `SYSTEM_INVARIANTS.md`, accepted ADRs, `APPLICATION_ARCHITECTURE.md` or the repository execution order. Any implementation must preserve the same governed mutation, tenant, authorization, persistence, audit and privacy guarantees as the rest of Ultimate CRM.

---

## 1. Product objective

The customer card must support a complete identity-document workflow in which a user can capture or upload a passport, receive reliable structured recognition, understand the evidence behind every extracted value, resolve ambiguity, and apply approved values to the authoritative customer identity without creating an alternate AI mutation path.

The end-state product must support:

- desktop upload and mobile camera capture;
- live capture guidance and server-authoritative image-quality validation;
- passports from multiple countries, document generations and scripts;
- visual-zone OCR (VIZ), MRZ extraction and deterministic ICAO validation;
- transliteration-aware cross-checking between VIZ and MRZ;
- multi-provider and provider-neutral document extraction;
- country/document-profile specific parsing and normalization;
- field-level provenance, confidence, validation and disagreement evidence;
- manual review, correction and policy-controlled automatic acceptance;
- original-document retention under governed file policies;
- document replacement, supersession, expiry and lifecycle history;
- duplicate-document and customer-identity conflict detection;
- permission-aware masked display and controlled reveal;
- Customer 360 projection without leaking protected values;
- Customer Privacy access/export/restriction/legal-hold/deletion coverage;
- full audit, trace, idempotency and replay-safe worker behavior;
- optional electronic-passport NFC verification with cryptographic evidence;
- benchmarked accuracy, latency, cost, failure and manual-correction metrics;
- provider substitution without changing Party domain semantics.

Recognition is not allowed to become the source of truth. `crm.parties` remains authoritative for identity-document state.

---

## 2. Architectural decision

### 2.1 Authoritative owner

`crm.parties` should own structured identity-document state because it already owns canonical person/organization identity.

The target Party-owned business concepts are:

```text
parties.party                    existing authoritative Party identity
parties.person_profile           structured person identity/profile
parties.identity_document        authoritative accepted identity document
parties.identity_document_source immutable source/evidence reference metadata
```

Passport recognition itself is **not** a separate authoritative business domain. A package named `crm-passport-ocr` or similar must not become an owner of customer identity.

### 2.2 Supporting technical boundaries

The processing architecture should be:

```text
Product plane
  -> governed file intake
  -> Party identity-document intake capability
  -> owner/background recognition workflow
  -> server-side image-quality and normalization pipeline
  -> provider-neutral DocumentExtractionPort
  -> one or more extraction providers
  -> deterministic MRZ/profile validation
  -> typed recognition candidate + evidence
  -> user/policy review
  -> Party acceptance capability
  -> authoritative Party identity-document state
  -> Customer 360 / privacy-aware projections
```

Physical object storage, OCR providers and model SDKs are infrastructure details. They must not leak into `crm.parties`.

### 2.3 No alternate mutation path

The following is forbidden:

```text
passport image -> model -> JSON -> direct UPDATE/INSERT
```

The model produces a **candidate**. Only an exact, versioned, authorized Party capability may make canonical changes.

### 2.4 Real extraction boundary

A dedicated technical crate is justified only if it protects a real provider/network/process/trust boundary. Its public contract should be provider-neutral, for example:

```text
DocumentExtractionPort
DocumentExtractionRequest
DocumentExtractionResult
DocumentProviderExecutionEvidence
```

Names such as `OpenAiPassportService` should not appear in Party domain/application APIs.

---

## 3. Product experience in the customer card

### 3.1 Identity Documents section

A production Party/customer record should expose an `Identity documents` section with:

- document type;
- issuing country;
- masked document number;
- issue/expiry dates;
- status;
- recognition/verification strength;
- latest validation result;
- source date;
- replacement/supersession state;
- permission-aware actions.

Example:

```text
Identity documents

Passport · Belarus
KH•••••523
Issued 2019-10-15 · Expires 2029-10-15
Status: verified from document
Evidence: VIZ + MRZ, all check digits valid

[View] [Review evidence] [Replace]

+ Add identity document
```

### 3.2 Capture flow

Supported input modes:

- file upload;
- drag-and-drop;
- desktop camera;
- mobile camera;
- mobile guided document capture;
- optional multi-page capture when a document profile requires more than one page;
- optional NFC/ePassport chip read on supported mobile devices.

The capture UI should provide live guidance where available:

- align document inside frame;
- move closer/farther;
- insufficient resolution;
- blur;
- glare;
- severe perspective;
- cropped edge;
- low/high exposure;
- MRZ not visible;
- page/document mismatch.

Browser checks are advisory UX only. The backend remains authoritative.

### 3.3 Recognition progress

The UI should expose meaningful stages rather than a generic spinner:

```text
Uploading securely
Checking image quality
Detecting document
Reading visual fields
Reading MRZ
Validating document data
Comparing evidence
Ready for review
```

Failures must be actionable, e.g. `Retake: lower-right MRZ corner is cropped`, not `OCR failed`.

### 3.4 Review screen

Every field should show:

- normalized proposed value;
- visible-source value where appropriate;
- MRZ-derived value where appropriate;
- source classification (`VIZ`, `MRZ`, `CHIP`, `MANUAL`, `MULTIPLE`);
- confidence/evidence state;
- deterministic validation result;
- disagreement/warning reason;
- user correction control where policy permits.

Example:

```text
Surname             YAKIMOVICH       VIZ + MRZ   ✓
Given names         IRYNA            VIZ + MRZ   ✓
Birth date          1972-09-10       VIZ + MRZ   ✓ checksum
Passport number     KH•••••523       VIZ + MRZ   ✓ checksum
Expiry date         2029-10-15       VIZ + MRZ   ✓ checksum
Native-script name  —               not present   informational
```

The product must never synthesize a native-script name merely from Latin transliteration and present it as document evidence.

### 3.5 Masking and reveal

Sensitive values should be masked by default. Exact reveal should be a separately permissioned query/action, traceable and purpose-aware where required.

Suggested UI principles:

- ordinary record views show masked document number;
- exact number and raw image require stronger permissions;
- copy-to-clipboard is separately controllable;
- reveal may be time-limited in the UI;
- sensitive values are never placed in URLs, browser analytics or generic telemetry.

---

## 4. Party domain model

### 4.1 PersonProfile

`PersonProfile` should be a Party-owned structured identity profile, distinct from the existing minimal Party display name.

Candidate fields include:

- legal/family name;
- given names;
- middle/patronymic names where applicable;
- native-script names;
- Latin/transliterated names;
- date of birth;
- place of birth;
- sex/gender marker as represented by authoritative document profile;
- nationality/citizenship references;
- country-specific person identifiers where legally/product-appropriate;
- provenance references for each accepted field;
- effective/superseded lineage.

The final wire shape should avoid forcing one cultural name model onto all countries. Name parts, scripts and transliteration evidence should be explicit.

### 4.2 IdentityDocument

Target business state:

```text
IdentityDocument
  identity_document_id
  party_id
  document_kind
  issuing_country
  issuing_authority?             when available
  document_number
  document_number_normalized
  document_profile_id/version
  issue_date?
  expiry_date?
  nationality?
  holder_name_snapshot
  birth_date?
  sex_marker?
  personal/national identifier?  profile-specific
  status
  verification_strength
  accepted_source_id
  created_at
  updated_at
  accepted_at?
  superseded_by?
  version
```

Document number and national identifiers must be modeled as sensitive typed values rather than generic free text.

### 4.3 Lifecycle

Suggested lifecycle states:

```text
DRAFT
SOURCE_ATTACHED
QUALITY_REJECTED
RECOGNITION_PENDING
RECOGNITION_RUNNING
RECOGNITION_RETRYABLE_FAILURE
RECOGNITION_TERMINAL_FAILURE
READY_FOR_REVIEW
REVIEWED_ACCEPTED
REVIEWED_REJECTED
ACTIVE
SUPERSEDED
EXPIRED
REVOKED
PRIVACY_MINIMIZED
```

Processing state and authoritative document lifecycle can be separate internal state machines if that produces cleaner invariants.

### 4.4 RecognitionAttempt

Recognition execution should be durable and separately versioned from the accepted document:

```text
RecognitionAttempt
  attempt_id
  party_id
  identity_document_id/draft_id
  source_file_id
  extraction_profile_id/version
  provider_policy_id/version
  provider/model execution metadata
  input_digest
  normalized-input digest
  status
  retry_generation
  created_at/started_at/completed_at
  safe failure code
  usage/cost evidence
  output_digest
```

Raw provider response should not be the canonical persisted format.

### 4.5 RecognitionCandidate

A typed candidate contains fields plus evidence:

```text
RecognitionCandidate
  document_classification
  document_profile
  fields[]
  mrz_result?
  visual_zone_result?
  chip_result?
  cross_validation_result
  quality_report
  authenticity_signals
  warnings
```

Each field should carry source/evidence metadata rather than one global confidence number.

### 4.6 FieldEvidence

For each extracted field:

```text
FieldEvidence
  field_id
  raw_observation?          retained only when policy permits
  normalized_value
  source_kind
  region/reference
  confidence_basis_points?
  parser_profile
  deterministic_checks[]
  compared_sources[]
  conflict_code?
```

A candidate may have multiple observations for one field. The system should preserve disagreement instead of silently choosing whichever provider returned last.

---

## 5. Proposed public capability/query surface

Exact names are subject to contract review, but the owner surface should remain coherent and versioned.

Potential mutations:

```text
parties.identity_document.draft.create@1.0.0
parties.identity_document.source.attach@1.0.0
parties.identity_document.recognition.request@1.0.0
parties.identity_document.review.accept@1.0.0
parties.identity_document.review.reject@1.0.0
parties.identity_document.correct@1.0.0
parties.identity_document.replace@1.0.0
parties.identity_document.revoke@1.0.0
```

Potential queries:

```text
parties.identity_document.get@1.0.0
parties.identity_document.list@1.0.0
parties.identity_document.recognition.get@1.0.0
parties.identity_document.evidence.get@1.0.0
parties.identity_document.source.download_authorization@1.0.0
parties.identity_document.exact_lookup@1.0.0
```

Potential events:

```text
parties.identity_document.draft_created@1.0.0
parties.identity_document.source_attached@1.0.0
parties.identity_document.recognition_completed@1.0.0
parties.identity_document.reviewed@1.0.0
parties.identity_document.accepted@1.0.0
parties.identity_document.corrected@1.0.0
parties.identity_document.superseded@1.0.0
parties.identity_document.revoked@1.0.0
```

Events must contain stable references and minimized metadata, not raw protected fields.

Recognition execution itself may remain internal/worker-only where public invocation would not add product value.

---

## 6. Governed file intake

### 6.1 Use the existing file boundary

Original passport bytes should enter through the governed immutable file-artifact layer rather than direct object-storage SDK access from the Party module.

The authoritative source relationship is:

```text
Party identity-document draft -> FileId
```

Object-storage keys remain infrastructure metadata.

### 6.2 Recommended data classification

A passport scan should normally be classified as at least `SensitivePersonal` in this platform vocabulary.

A photograph embedded in a passport image is not automatically a biometric processing operation. If the platform performs face recognition, face comparison or biometric template extraction, those derived payloads must be treated as `Biometric` and governed separately.

### 6.3 Persistent versus transient derivatives

Persist the immutable original when business/legal policy requires it.

Prefer transient generation of:

- deskewed image;
- perspective-corrected image;
- enhanced grayscale/color variants;
- MRZ crop;
- face/document-region crops;
- provider-specific encoded payloads.

Persistent derivatives require an explicit business/forensic reason, retention policy and data-class declaration.

### 6.4 File metadata

Required metadata should include:

- tenant;
- owner module;
- Party/document reference;
- declared/detected media type;
- exact size;
- SHA-256;
- encryption key version;
- data class;
- malware/scan state where applicable;
- retention policy;
- legal hold;
- source/capture channel;
- capture timestamp;
- deletion/minimization state.

---

## 7. Image-quality and normalization pipeline

The quality gate should run before expensive external recognition whenever possible.

### 7.1 Client-side advisory checks

Useful instant checks:

- file type;
- file size;
- dimensions;
- obvious blur;
- camera focus indication;
- document framing guidance.

These do not replace backend validation.

### 7.2 Server-authoritative checks

The backend pipeline should evaluate:

- document/page detection;
- four-edge completeness;
- orientation;
- skew;
- perspective distortion;
- minimum effective pixels per document width/height;
- focus/blur score;
- motion blur;
- glare/specular reflection coverage;
- under/over-exposure;
- shadow coverage;
- contrast;
- occlusion/finger coverage;
- crop/missing-corner detection;
- compression artifacts;
- screenshot/photocopy/display recapture indicators where feasible;
- MRZ-region visibility;
- portrait-region visibility when required;
- security-feature regions when supported by the document profile.

The result should be a typed `DocumentQualityReport` with reason codes and actionable retake instructions.

### 7.3 Normalization

Potential deterministic transforms:

- EXIF orientation normalization;
- metadata stripping;
- document boundary crop;
- perspective correction;
- dewarping;
- rotation;
- color normalization;
- contrast-local enhancement;
- denoise/sharpen only under calibrated rules;
- separate OCR-optimized and authenticity-analysis variants.

The original must never be overwritten.

### 7.4 Avoid destructive enhancement

Aggressive sharpening, denoising or generative image enhancement can invent glyph-like structures. OCR preprocessing must therefore be versioned and benchmarked. Any generative enhancement should be treated as derived evidence, never as a replacement for original pixels.

---

## 8. Document classification and profile registry

A global system needs a versioned `DocumentProfile` registry rather than one hard-coded Belarus/Russia/etc parser.

Each profile can define:

```text
profile_id/version
country/territory
passport/document family
specimen/version range
document dimensions/layout
expected pages/sides
MRZ type
VIZ fields and regions
scripts/languages
transliteration profile
date/number formats
checksums/control digits
known optional fields
security-feature metadata
chip/eMRTD capabilities
field normalization rules
```

The registry should be immutable/versioned after publication. A recognition attempt records the exact profile used.

Document classification may use image layout, visible labels, MRZ structure and provider output, but final profile selection should be explainable and uncertainty-preserving.

---

## 9. MRZ extraction and deterministic validation

MRZ is a first-class deterministic evidence source, not merely another OCR string.

### 9.1 Standards coverage

At minimum support the ICAO 9303 machine-readable structures relevant to passports and related identity documents, including TD3 passport MRZ. The parsing library should be extensible for TD1/TD2/MRV forms if the wider identity-document family is enabled.

### 9.2 Parsing

The parser should normalize:

- `<` filler semantics;
- allowed A-Z/0-9 character vocabulary;
- name separators;
- document code;
- issuing state;
- nationality;
- birth date;
- sex marker;
- expiry date;
- document number;
- optional/personal data;
- check digits;
- composite check digit.

### 9.3 Check digits

Implement the ICAO weighted check calculation deterministically using the 7-3-1 repeating weight pattern and canonical character mapping.

Validation should produce independent results for:

- document number check digit;
- birth date check digit;
- expiry date check digit;
- optional/personal number check digit where applicable;
- final composite check digit.

### 9.4 OCR ambiguity repair

Characters such as `0/O`, `1/I`, `2/Z`, `5/S`, `8/B` must not be globally auto-replaced. A constrained correction algorithm may test candidate substitutions only within the valid MRZ grammar and checksum equations, recording exactly which ambiguity was resolved and why.

### 9.5 Century/date resolution

Two-digit MRZ dates require deterministic profile/time-context rules. The selected full date and the resolution rule must be traceable. No model should silently infer a century without recorded policy.

---

## 10. Visual-zone OCR

### 10.1 Script-aware extraction

The visual zone should support:

- Latin;
- Cyrillic;
- Arabic;
- Greek;
- Hebrew;
- CJK and other scripts where document profiles require them;
- mixed-script documents;
- profile-specific labels and ordering.

### 10.2 Structured output

Providers should return schema-bound typed candidate fields, not prose.

The extraction port should accept an explicit expected schema/profile and reject unknown/oversized output.

### 10.3 Names

Names require particularly careful modeling:

- exact printed native-script observation;
- exact printed Latin/transliterated observation;
- MRZ name;
- normalized matching representation;
- accepted canonical Party name change as a separate owner decision.

The system should preserve the distinction between `printed value`, `normalized value`, `transliteration`, and `inferred value`.

---

## 11. Multi-source cross-validation

The strongest recognition result combines independent evidence.

Potential sources:

```text
VIZ image OCR
MRZ OCR + deterministic parser/check digits
country/profile rules
barcode/2D code where document family supports it
NFC/ePassport chip DG data
manual operator correction
prior accepted Party state (comparison only, never override)
```

For every field, derive a comparison state such as:

```text
EXACT_MATCH
NORMALIZED_MATCH
TRANSLITERATION_MATCH
FORMAT_EQUIVALENT
SOURCE_ONLY
CONFLICT
CHECKSUM_INVALID
UNREADABLE
NOT_PRESENT
```

Conflicts must be visible to the reviewer and must block automatic acceptance according to policy.

---

## 12. Electronic passport / NFC capability

For expert-grade mobile identity-document handling, the target architecture should support eMRTD chip reads where the operating system/device allows them.

### 12.1 Supported evidence

Potentially ingest and validate:

- BAC/PACE session establishment;
- LDS data groups;
- DG1 biographic/MRZ data;
- DG2 facial image;
- SOD security object;
- document signer certificate chain;
- CSCA trust material;
- passive authentication;
- chip authentication / active authentication where document generation supports it.

### 12.2 Trust evidence

Cryptographic verification output must be typed and explainable:

```text
chip_read_succeeded
sod_signature_valid
signer_chain_valid
trust_anchor_version
passive_authentication_result
chip_authentication_result
active_authentication_result
read_timestamp
```

A model must never fabricate cryptographic verification evidence.

### 12.3 Mobile boundary

NFC handling belongs in a dedicated mobile/device technical boundary, not in Party domain. The Party acceptance capability receives validated evidence and stable references.

---

## 13. Authenticity and fraud signals

Recognition and authenticity verification are related but distinct. The product should make the distinction explicit.

Potential image/document signals:

- photo replacement/tamper indicators;
- layout inconsistency against known profile;
- font/glyph anomalies;
- MRZ/VIZ inconsistency;
- invalid control digits;
- impossible dates;
- unusual document-number format;
- copy/screenshot/print recapture signals;
- portrait-region manipulation signals;
- provider-specific document-authenticity signals;
- chip/VIZ mismatch;
- chip signature failure.

Each signal should have a source and reason. A single opaque `fraud_score` is insufficient.

If a composite risk score is used, its versioned policy must be explainable and must retain the contributing signals.

---

## 14. Provider architecture

### 14.1 Provider-neutral port

The Party application layer should depend on a stable port roughly equivalent to:

```text
trait DocumentExtractionPort {
  extract(request) -> DocumentExtractionResult
}
```

The request should contain only governed references and bounded bytes/derived input provided by the infrastructure layer.

### 14.2 Provider adapters

Adapters may target:

- multimodal LLM providers;
- specialist document-recognition providers;
- deterministic OCR engines;
- on-prem/self-hosted OCR;
- country-specific verification services;
- an ensemble/router that chooses providers by document profile, residency, latency, quality or cost.

### 14.3 Provider policy

Tenant/provider policy should govern:

- allowed providers;
- region/residency;
- allowed data classes;
- purpose/legal basis;
- model/provider versions;
- retention/no-training contractual profile;
- retry/fallback behavior;
- maximum cost;
- timeout;
- concurrency/rate limit;
- data-transfer restrictions.

### 14.4 Retry and fallback

Retry semantics must distinguish:

- transport timeout;
- rate limit;
- provider unavailable;
- invalid provider output;
- low-quality input;
- unsupported document;
- deterministic validation failure;
- policy denial.

A fallback provider must not convert a terminal input defect into repeated paid requests.

### 14.5 Provider evidence

Persist only bounded execution evidence needed for reproducibility/operations, e.g.:

- provider profile/version;
- model/engine identifier;
- request/attempt ID;
- input digest;
- output digest;
- latency;
- token/page/unit usage where available;
- cost accounting reference;
- finish/status class;
- safe failure code.

Credentials, full prompts containing passport data, raw protected payloads and unrestricted model dumps must not enter ordinary logs/audit.

---

## 15. Ensemble and confidence strategy

For high-value workflows, provider output may be combined with deterministic OCR or a second provider.

Recommended confidence model:

- field-level confidence, not one global confidence;
- deterministic MRZ checksum state treated separately from probabilistic OCR confidence;
- source agreement increases evidence strength;
- source conflict lowers acceptance strength even when one provider is highly confident;
- provider confidence is calibrated on the CRM benchmark rather than trusted at face value;
- automatic acceptance thresholds are country/profile/field specific.

Potential modes:

```text
single-provider + MRZ validation
specialist OCR + MRZ + LLM reconciliation
primary provider + fallback provider
independent double extraction for high-risk workflows
NFC chip + VIZ/MRZ comparison
```

---

## 16. Automatic acceptance policy

The platform should support policy-controlled automation, but not unconditional model-driven auto-save.

A high-assurance policy could require:

- supported, unambiguous document profile;
- quality gate pass;
- all required fields present;
- MRZ grammar valid;
- all applicable check digits valid;
- VIZ/MRZ exact or approved normalized match;
- no unresolved OCR ambiguity;
- no high-severity authenticity signal;
- no duplicate active document conflict;
- live authorization immediately before canonical mutation;
- configured tenant policy permitting auto-accept for that document/field set.

Otherwise the candidate enters human review.

Automatic acceptance must produce the same event, audit, idempotency and version evidence as interactive acceptance.

---

## 17. Manual correction and provenance

A correction is not equivalent to OCR output. Preserve:

```text
provider observation
normalized provider value
reviewer-entered correction
reviewer identity
reason code
review timestamp
accepted value
source/evidence relationship
```

If a user changes `1972-09-10` to `1973-09-10`, the system must not rewrite historical recognition evidence as if the model originally returned 1973.

---

## 18. Identity resolution and duplicate detection

A passport can be a powerful identity-resolution signal but must not bypass `crm.identity-resolution` authority for merge decisions.

Recommended flow:

```text
accepted Party identity document
  -> minimized versioned signal/reference
  -> Identity Resolution candidate detection
  -> governed merge/review semantics
```

Potential duplicate checks:

- same normalized document number + issuing country;
- same personal identifier under a country-specific scheme;
- same chip/document identity;
- same high-confidence person attributes as a secondary signal.

Exact document values should not be copied into generic events. Use stable references or tenant-scoped keyed digests where appropriate.

Cross-tenant matching is forbidden unless a separately governed platform-global legal/product feature is explicitly designed.

---

## 19. Exact document lookup without global search leakage

Global search should not casually index passport number, national identifier or raw MRZ.

For legitimate exact lookup, prefer a dedicated permission-aware Party query backed by a tenant-scoped deterministic lookup index, for example an HMAC/keyed digest of canonical `(issuing_country, document_number)` under a versioned key/profile.

Properties:

- exact-match only;
- tenant scoped;
- key rotation/version support;
- no plaintext number in search documents;
- authorization before disclosure;
- uniform not-found/concealment behavior.

---

## 20. Customer 360 integration

Customer 360 should expose a bounded projection such as:

```text
identity_documents[]:
  kind
  issuing_country
  masked_document_number
  issue_date?
  expiry_date?
  lifecycle_status
  verification_strength
```

Raw image, raw MRZ, exact document number, national identifier and provider payload should not be copied into ordinary Customer 360 documents.

Projection rebuild must honor Party privacy tombstones/minimization and be safe under replay.

---

## 21. Customer Privacy integration

Identity documents and source files must be integrated into the existing privacy owner scope from their first production release.

Coverage must include:

- subject discovery;
- access/export;
- restriction placement/release;
- legal hold placement/release;
- mandatory retention precedence;
- deletion/anonymization/minimization rules;
- provider-evidence retention;
- file retention/deletion;
- projection/search/cache convergence;
- immutable audit/event lineage where legally retained.

The privacy plan must distinguish data that must be erased from business state from immutable evidence that is retained under an allowed legal/retention basis.

---

## 22. Security and threat model

Threats to design for include:

- cross-tenant document access;
- unauthorized exact-number reveal;
- raw image leakage via logs/traces/error reporting;
- malicious image/parser payloads;
- decompression bombs;
- polyglot/misdeclared files;
- malware-bearing PDFs/images;
- prompt injection text embedded in document images;
- provider prompt/response exfiltration;
- stale signed download URLs;
- replayed upload/finalization;
- duplicate processing side effects;
- provider account compromise;
- document substitution between upload and recognition;
- tampering with derived image/evidence;
- weak retention/deletion propagation;
- browser analytics collecting sensitive form values;
- overbroad clipboard/export behavior;
- model hallucination being accepted as document evidence.

Required controls include exact byte digest binding, immutable file identity, typed schemas, bounded decode, content-type detection, tenant-scoped authorization, short-lived download authorization, provider policy, deterministic validation and safe error mapping.

---

## 23. Audit and observability

Audit records should explain:

- actor;
- Party/document stable reference;
- capability;
- action type;
- policy/authorization decision references;
- recognition/review attempt stable reference;
- result class;
- changed canonical fields by field identifiers, not raw secret values.

Operational telemetry should include:

- recognition requests;
- quality-gate reject rate;
- provider dispatch rate;
- provider success/retry/terminal failure;
- latency p50/p95/p99 by stage;
- queue latency;
- cost per document/profile/provider;
- MRZ parse success;
- checksum success/failure;
- VIZ/MRZ conflict rate;
- human review rate;
- manual correction rate by field;
- automatic acceptance rate;
- unsupported document rate;
- expiration/renewal counts;
- privacy deletion/retention convergence;
- provider fallback rate.

No metric label should contain document number, name, MRZ or national identifier.

---

## 24. Performance and reliability targets

Targets should be measured by document profile and capture channel rather than hidden in a single global average.

Suggested product objectives to validate empirically:

- local quality checks return in near-interactive time;
- recognition begins durably even if the browser disconnects;
- retries never duplicate accepted Party mutations;
- worker restart resumes from durable state;
- provider failure cannot strand an unqueryable draft;
- source file remains cryptographically bound to the recognition attempt;
- successful recognition is reviewable without re-calling the provider;
- exact acceptance/rejection replay returns the same idempotent result;
- processing remains bounded by tenant quotas and worker fairness.

SLOs should be established from production-like measurements per provider/profile, then made blocking for release readiness where appropriate.

---

## 25. Accuracy and benchmark program

### 25.1 Dataset

Maintain a controlled, access-restricted benchmark corpus outside ordinary public source fixtures.

Coverage should span:

- countries/territories;
- document generations/specimens;
- multiple scripts;
- high/low resolution;
- mobile devices;
- perspective;
- glare;
- blur;
- shadows;
- worn/damaged pages;
- photocopies/screens where allowed for negative tests;
- unusual names;
- long names;
- diacritics;
- ambiguous MRZ characters;
- near-expiry/expired documents;
- deliberately corrupted check digits for negative validation.

Public repository tests should use synthetic, generated or safely redacted fixtures.

### 25.2 Metrics

Measure at least:

- exact document-number accuracy;
- exact MRZ line accuracy;
- MRZ character error rate;
- name exact/normalized accuracy;
- birth-date exact accuracy;
- expiry-date exact accuracy;
- nationality/sex marker accuracy;
- personal-number accuracy where applicable;
- document-profile classification accuracy;
- quality-gate false accept/reject;
- VIZ/MRZ conflict detection precision/recall;
- manual correction rate;
- automatic-accept precision;
- unsupported-document correct rejection;
- latency and cost.

### 25.3 Promotion gate

A new provider/model/profile version should not become the default merely because it is newer. Promotion requires reproducible before/after benchmark evidence and no unacceptable regression in high-severity fields or safety behavior.

---

## 26. Testing strategy

### Pure domain/unit

- identity-document invariants;
- lifecycle transitions;
- normalization;
- typed sensitive values;
- MRZ parser/check digits;
- date resolution;
- country/profile rules;
- conflict resolution;
- automatic acceptance policy.

### Property/fuzz

- MRZ grammar;
- checksum routines;
- parser robustness;
- malformed provider output;
- Unicode/script normalization;
- image decoder/container metadata boundaries where appropriate.

### Persistence/PostgreSQL

- transaction/version/idempotency;
- FORCE RLS;
- cross-tenant negative proof;
- exact lookup digest/index;
- rollback/reapply;
- privacy scope;
- retention/legal hold;
- restart recovery.

### Provider contract

- timeout;
- malformed/oversized output;
- unknown fields;
- rate limit;
- retryable/terminal mapping;
- fallback;
- digest binding;
- policy denial;
- simulated prompt-injection content.

### Process/E2E

- customer card upload -> recognition -> review -> Party acceptance;
- browser disconnect/reconnect;
- worker restart during provider call and after provider response;
- duplicate request/idempotent convergence;
- session expiry;
- permission denial;
- tenant concealment;
- provider outage;
- poor-photo retake;
- VIZ/MRZ disagreement;
- privacy restriction/legal hold/deletion;
- Customer 360 convergence.

### Browser/accessibility

- keyboard-only capture/review where feasible;
- screen-reader labels/state changes;
- progress announcements;
- focus recovery after validation errors;
- mobile responsive capture;
- camera permission denial;
- high zoom/contrast;
- masked/reveal behavior.

---

## 27. Worker and process topology

A robust pipeline can be expressed as durable owner/process stages while keeping worker algorithms generic.

Logical stages:

```text
1. source-finalization verification
2. quality assessment
3. document classification/profile selection
4. deterministic preprocessing
5. extraction dispatch
6. provider response validation
7. MRZ parsing/checks
8. VIZ/MRZ/chip cross-validation
9. candidate materialization
10. review/auto-accept eligibility
11. downstream projection/identity-resolution signals
```

These stages do not necessarily require one worker each. Physical worker boundaries should be chosen by latency, retry, provider/process and trust concerns, not by naming every function as a new crate/process.

Worker ordering/phase registration must remain module-owned and activation-gated.

---

## 28. Suggested repository touchpoints

Expected existing areas:

```text
modules/crm-parties
proto/crm/parties/v1
crates/crm-parties-capability-adapter
crates/crm-parties-query-adapter
crates/crm-party-reference-composition
crates/crm-first-party-modules
crates/crm-core-files
packages/client
packages/ui
apps/web
Customer 360 composition/query
Parties privacy-scope adapter
Identity Resolution governed integration
```

Possible new technical boundary only if justified:

```text
crates/crm-document-extraction-*    provider-neutral port/adapter/process boundary
```

Do not add a crate for each passport capability, OCR step or country profile.

---

## 29. Implementation program

This is sequencing for delivery safety, **not a reduction of end-state scope**.

### Packet A — structured Party identity foundation

- PersonProfile domain model;
- IdentityDocument domain model;
- typed sensitive values;
- public contracts;
- persistence schema/envelope;
- manual create/review/query lifecycle;
- privacy scope integration.

### Packet B — production customer/Party card

- real Party/customer route;
- identity-document section;
- list/detail/add/edit/replace flows;
- masking/reveal permissions;
- accessibility/browser acceptance.

### Packet C — governed file source

- upload/capture flow;
- immutable file binding;
- source lifecycle;
- retention/legal hold;
- download authorization;
- negative tenant proof.

### Packet D — deterministic quality + MRZ foundation

- quality report;
- normalization pipeline;
- document profile registry foundation;
- MRZ parser/check digits;
- synthetic benchmark fixtures;
- review evidence UI.

### Packet E — provider-neutral extraction

- extraction port;
- provider policy;
- first production provider adapter;
- worker/process/retry evidence;
- structured VIZ extraction;
- cost/latency telemetry.

### Packet F — multi-source validation and acceptance automation

- VIZ/MRZ comparison;
- field evidence;
- confidence calibration;
- automatic-accept policy;
- human correction provenance;
- provider fallback/ensemble.

### Packet G — global profile expansion

- additional countries/specimen generations;
- multi-script extraction;
- country-specific fields/checks;
- benchmark-driven profile promotion.

### Packet H — ePassport/NFC authenticity

- mobile chip-read boundary;
- SOD/certificate verification;
- DG1/DG2 evidence;
- chip/VIZ/MRZ cross-check;
- cryptographic verification UI/evidence.

### Packet I — identity resolution, Customer 360 and operations closure

- duplicate-document signal;
- exact permission-aware lookup;
- identity-resolution candidate integration;
- Customer 360 masked projection;
- complete privacy lifecycle;
- restore/SLO/observability/security/supply-chain proof;
- production benchmark acceptance.

---

## 30. Definition of Done — 10/10 passport capability

The capability is product-complete only when all applicable criteria are mechanically proven:

1. `crm.parties` is the sole owner of accepted identity-document business state.
2. Provider/model code cannot mutate Party state directly.
3. Original files use the governed tenant-aware file boundary.
4. Raw protected document content is absent from logs, audit envelopes and generic events.
5. All canonical mutations use exact capabilities, live authorization, idempotency, event and audit semantics.
6. Server-authoritative quality checks run before paid extraction where technically possible.
7. MRZ parsing and check-digit validation are deterministic and fully tested.
8. VIZ/MRZ/chip disagreements are preserved and reviewable rather than silently overwritten.
9. Provider output is schema-bound, bounded and rejected on malformed/unknown data.
10. Provider/model replacement requires no Party domain change.
11. Human corrections preserve original extraction evidence and reviewer provenance.
12. Automatic acceptance is explicit policy, not model confidence alone.
13. Multi-country/document-profile support is versioned and benchmarked.
14. Native-script values are never invented when not observed.
15. Exact document values are masked by default and separately permissioned for reveal/lookup.
16. Global search does not leak passport numbers, national identifiers or raw MRZ.
17. Duplicate/identity-resolution signals use governed references or tenant-scoped protected lookup representations.
18. Customer 360 stores only the bounded permission-aware projection.
19. Customer Privacy covers access/export/restriction/legal hold/retention/deletion and projection convergence.
20. Cross-tenant negative proof exists for document state, source files, recognition evidence and reads.
21. Worker retry/restart and duplicate execution converge without duplicate Party effects.
22. Provider outages, timeouts and malformed output have stable typed recovery behavior.
23. Browser/mobile capture, review and permission journeys have accessibility acceptance.
24. Benchmark evidence covers representative supported countries, scripts, capture quality and document generations.
25. Accuracy, correction rate, latency, cost and failure metrics are observable by profile/provider without sensitive labels.
26. New profile/provider promotion is benchmark-gated and reversible.
27. ePassport cryptographic claims, when enabled, are based on actual cryptographic verification and never model inference.
28. Restore/replay/rebuild preserve source/evidence/canonical lineage correctly.
29. The feature adds no owner-specific generic-runtime algorithm branch.
30. The final implementation remains explainable through repository navigation and affected-scope tooling.

The target is not merely “OCR works”. The target is a governed global identity-document subsystem in which recognition quality, deterministic evidence, privacy, auditability, user correction, provider independence and operational resilience are all first-class product behavior.
