import { useRef, useState, type FormEvent } from "react";
import {
  ProductClientError,
  type GovernedCustomerPrivacyClient,
  type PrivacyCase,
} from "@ultimate-crm/client";
import { FeedbackPanel, PageHeader } from "@ultimate-crm/ui";
import { customerPrivacyCaseViewModel } from "./customerPrivacyViewModel";

export function CustomerPrivacyPage({
  client,
}: {
  client: GovernedCustomerPrivacyClient;
}) {
  const [canonicalPartyId, setCanonicalPartyId] = useState("");
  const [lastSubmittedPartyId, setLastSubmittedPartyId] = useState("");
  const [cases, setCases] = useState<PrivacyCase[]>([]);
  const [selectedCase, setSelectedCase] = useState<PrivacyCase | null>(null);
  const [loading, setLoading] = useState(false);
  const [loadingDetail, setLoadingDetail] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [announcement, setAnnouncement] = useState(
    "Enter a canonical Party reference to review privacy cases.",
  );
  const resultsHeadingRef = useRef<HTMLHeadingElement>(null);
  const detailHeadingRef = useRef<HTMLHeadingElement>(null);
  const errorHeadingRef = useRef<HTMLHeadingElement>(null);

  const loadCases = async (partyId: string) => {
    const normalizedPartyId = partyId.trim();
    if (!normalizedPartyId) return;
    setLoading(true);
    setError(null);
    setSelectedCase(null);
    setLastSubmittedPartyId(normalizedPartyId);
    setAnnouncement("Loading privacy cases.");
    try {
      const response = await client.listCases({
        canonicalPartyId: normalizedPartyId,
        pageSize: 25,
      });
      setCases(response.privacyCases);
      setAnnouncement(
        response.privacyCases.length === 0
          ? "No privacy cases were found for this Party reference."
          : `${response.privacyCases.length} privacy case${response.privacyCases.length === 1 ? "" : "s"} loaded.`,
      );
      queueMicrotask(() => resultsHeadingRef.current?.focus());
    } catch (caught) {
      setCases([]);
      setError(customerPrivacyMessageForError(caught));
      setAnnouncement("Privacy cases could not be loaded.");
      queueMicrotask(() => errorHeadingRef.current?.focus());
    } finally {
      setLoading(false);
    }
  };

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    await loadCases(canonicalPartyId);
  };

  const handleSelect = async (privacyCaseId: string) => {
    setLoadingDetail(true);
    setError(null);
    setAnnouncement("Loading privacy case details.");
    try {
      const response = await client.getCase(privacyCaseId);
      if (!response.privacyCase) {
        throw new ProductClientError({
          kind: "internal",
          message: "The verified response did not contain a privacy case.",
          retryable: false,
          safeCode: "CUSTOMER_PRIVACY_CASE_MISSING",
        });
      }
      setSelectedCase(response.privacyCase);
      setAnnouncement("Privacy case details loaded.");
      queueMicrotask(() => detailHeadingRef.current?.focus());
    } catch (caught) {
      setSelectedCase(null);
      setError(customerPrivacyMessageForError(caught));
      setAnnouncement("Privacy case details could not be loaded.");
      queueMicrotask(() => errorHeadingRef.current?.focus());
    } finally {
      setLoadingDetail(false);
    }
  };

  return (
    <div>
      <PageHeader
        eyebrow="Governed Customer Privacy read path"
        title="Customer Privacy cases"
        description="Review bounded case status for an explicit canonical Party reference. The backend performs live authorization and tenant isolation for every request."
      />

      <form onSubmit={handleSubmit} aria-labelledby="privacy-case-search-heading">
        <h2 id="privacy-case-search-heading">Find cases by canonical Party</h2>
        <label htmlFor="canonical-party-id">Canonical Party reference</label>
        <p id="canonical-party-help">
          Enter the opaque Party identifier. Do not enter a name, email address, passport number, or other personal data.
        </p>
        <input
          id="canonical-party-id"
          name="canonical-party-id"
          value={canonicalPartyId}
          onChange={(event) => setCanonicalPartyId(event.target.value)}
          aria-describedby="canonical-party-help"
          autoComplete="off"
          maxLength={128}
          disabled={loading}
        />
        <button type="submit" disabled={loading || canonicalPartyId.trim().length === 0}>
          {loading ? "Loading cases…" : "Load cases"}
        </button>
      </form>

      <p role="status" aria-live="polite" aria-atomic="true">
        {announcement}
      </p>

      {error ? (
        <section aria-labelledby="privacy-error-heading">
          <h2 id="privacy-error-heading" ref={errorHeadingRef} tabIndex={-1}>
            Request unavailable
          </h2>
          <FeedbackPanel tone="danger" title="Customer Privacy request failed">
            <p>{error}</p>
            {lastSubmittedPartyId ? (
              <button type="button" onClick={() => void loadCases(lastSubmittedPartyId)}>
                Retry case list
              </button>
            ) : null}
          </FeedbackPanel>
        </section>
      ) : null}

      <section aria-labelledby="privacy-results-heading" aria-busy={loading}>
        <h2 id="privacy-results-heading" ref={resultsHeadingRef} tabIndex={-1}>
          Privacy cases
        </h2>
        {!loading && !error && lastSubmittedPartyId && cases.length === 0 ? (
          <FeedbackPanel tone="neutral" title="No cases found">
            No privacy cases are visible for this canonical Party reference.
          </FeedbackPanel>
        ) : null}
        {cases.length > 0 ? (
          <ul>
            {cases.map((privacyCase) => {
              const view = customerPrivacyCaseViewModel(privacyCase);
              return (
                <li key={view.id}>
                  <button
                    type="button"
                    onClick={() => void handleSelect(view.id)}
                    aria-pressed={selectedCase?.privacyCaseRef?.privacyCaseId === view.id}
                    disabled={loadingDetail}
                  >
                    <span>{view.kind}</span>
                    <span> — {view.status}</span>
                    <span> — Case {view.id}</span>
                  </button>
                </li>
              );
            })}
          </ul>
        ) : null}
      </section>

      {selectedCase ? (
        <PrivacyCaseDetail privacyCase={selectedCase} headingRef={detailHeadingRef} />
      ) : null}
    </div>
  );
}

function PrivacyCaseDetail({
  privacyCase,
  headingRef,
}: {
  privacyCase: PrivacyCase;
  headingRef: React.RefObject<HTMLHeadingElement | null>;
}) {
  const view = customerPrivacyCaseViewModel(privacyCase);
  return (
    <section aria-labelledby="privacy-case-detail-heading">
      <h2 id="privacy-case-detail-heading" ref={headingRef} tabIndex={-1}>
        Selected privacy case
      </h2>
      <dl>
        <div><dt>Case reference</dt><dd>{view.id}</dd></div>
        <div><dt>Kind</dt><dd>{view.kind}</dd></div>
        <div><dt>Status</dt><dd>{view.status}</dd></div>
        <div><dt>Version</dt><dd>{view.version}</dd></div>
        <div><dt>Policy version</dt><dd>{view.policyVersion}</dd></div>
        <div><dt>Created</dt><dd>{view.createdAt}</dd></div>
        <div><dt>Updated</dt><dd>{view.updatedAt}</dd></div>
      </dl>
    </section>
  );
}

export function customerPrivacyMessageForError(error: unknown): string {
  if (!(error instanceof ProductClientError)) {
    return "The Customer Privacy service is temporarily unavailable. Try again later.";
  }
  switch (error.kind) {
    case "unauthenticated":
      return "Your session is no longer available. Sign in again.";
    case "permission_denied":
    case "not_found":
      return "The requested privacy case is not available to this session.";
    case "invalid_argument":
      return "Check the opaque Party or case reference and try again.";
    case "rate_limited":
      return "Too many requests were submitted. Try again later.";
    case "conflict":
      return "The privacy case changed while it was being read. Reload the case list.";
    case "unavailable":
    case "network":
      return "The Customer Privacy service is temporarily unavailable. Try again later.";
    default:
      return "The verified Customer Privacy response could not be displayed.";
  }
}
