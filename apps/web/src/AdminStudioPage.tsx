import { useEffect, useMemo, useRef, useState } from "react";
import {
  GovernedMetadataClient,
  MetadataAuthoringError,
  ProductClientError,
  createObjectMetadataDefinitionInput,
  type MetadataActivationState,
  type MetadataImpact,
  type ObjectMetadataDraft,
} from "@ultimate-crm/client";
import { FeedbackPanel, PageHeader } from "@ultimate-crm/ui";

interface AdminStudioPageProps {
  client: GovernedMetadataClient;
}

type AdminOperation =
  | "idle"
  | "loading_activation"
  | "publishing"
  | "loading_impact"
  | "activating"
  | "rolling_back";

interface MutationIntentKeys {
  publish?: string;
  activate?: string;
  rollback?: string;
}

const INITIAL_DRAFT: ObjectMetadataDraft = {
  id: "crm.custom.asset",
  ownerModuleId: "crm.custom",
  label: "Asset",
  pluralLabel: "Assets",
  description: "",
  tags: ["custom"],
};

export function AdminStudioPage({ client }: AdminStudioPageProps) {
  const [draft, setDraft] = useState<ObjectMetadataDraft>(INITIAL_DRAFT);
  const [tagsText, setTagsText] = useState(INITIAL_DRAFT.tags.join(", "));
  const [candidateRevisionId, setCandidateRevisionId] = useState<string | null>(null);
  const [candidateWasNew, setCandidateWasNew] = useState<boolean | null>(null);
  const [impact, setImpact] = useState<MetadataImpact | null>(null);
  const [activation, setActivation] = useState<MetadataActivationState | null>(null);
  const [confirmBreakingChanges, setConfirmBreakingChanges] = useState(false);
  const [operation, setOperation] = useState<AdminOperation>("idle");
  const [error, setError] = useState<string | null>(null);
  const mutationIntentKeys = useRef<MutationIntentKeys>({});

  const busy = operation !== "idle";
  const candidateIsActive =
    candidateRevisionId !== null &&
    activation?.activeRevisionId === candidateRevisionId;
  const tags = useMemo(
    () =>
      tagsText
        .split(",")
        .map((value) => value.trim())
        .filter(Boolean),
    [tagsText],
  );

  useEffect(() => {
    let cancelled = false;
    setOperation("loading_activation");
    setError(null);
    void client
      .getActivation()
      .then((response) => {
        if (!cancelled) {
          setActivation(response.state ?? null);
        }
      })
      .catch((caught: unknown) => {
        if (!cancelled) {
          setError(productErrorMessage(caught));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setOperation("idle");
        }
      });
    return () => {
      cancelled = true;
    };
  }, [client]);

  const resetCandidate = () => {
    setCandidateRevisionId(null);
    setCandidateWasNew(null);
    setImpact(null);
    setConfirmBreakingChanges(false);
    delete mutationIntentKeys.current.publish;
    delete mutationIntentKeys.current.activate;
  };

  const updateDraft = <K extends keyof ObjectMetadataDraft>(
    field: K,
    value: ObjectMetadataDraft[K],
  ) => {
    setDraft((current) => ({ ...current, [field]: value }));
    resetCandidate();
  };

  const publishCandidate = async (event: React.FormEvent) => {
    event.preventDefault();
    setOperation("publishing");
    setError(null);
    const idempotencyKey =
      mutationIntentKeys.current.publish ?? crypto.randomUUID();
    mutationIntentKeys.current.publish = idempotencyKey;
    try {
      const definition = createObjectMetadataDefinitionInput({ ...draft, tags });
      const response = await client.publishBundle({
        definitions: [definition],
        idempotencyKey,
      });
      delete mutationIntentKeys.current.publish;
      delete mutationIntentKeys.current.activate;
      setCandidateRevisionId(response.revisionId);
      setCandidateWasNew(response.newlyPublished);
      setImpact(null);
      setConfirmBreakingChanges(false);
    } catch (caught) {
      if (!isRetryableProductError(caught)) {
        delete mutationIntentKeys.current.publish;
      }
      setError(productErrorMessage(caught));
    } finally {
      setOperation("idle");
    }
  };

  const reviewImpact = async () => {
    if (!candidateRevisionId) return;
    setOperation("loading_impact");
    setError(null);
    try {
      const response = await client.getImpact(candidateRevisionId);
      setImpact(response.impact ?? null);
      setConfirmBreakingChanges(false);
      delete mutationIntentKeys.current.activate;
    } catch (caught) {
      setError(productErrorMessage(caught));
    } finally {
      setOperation("idle");
    }
  };

  const activateCandidate = async () => {
    if (!candidateRevisionId || !impact || candidateIsActive) return;
    setOperation("activating");
    setError(null);
    const idempotencyKey =
      mutationIntentKeys.current.activate ?? crypto.randomUUID();
    mutationIntentKeys.current.activate = idempotencyKey;
    try {
      const response = await client.activateRevision({
        revisionId: candidateRevisionId,
        expectedGeneration: activation?.generation ?? 0n,
        confirmBreakingChanges: impact.hasBreakingChanges
          ? confirmBreakingChanges
          : false,
        idempotencyKey,
      });
      delete mutationIntentKeys.current.activate;
      delete mutationIntentKeys.current.rollback;
      setActivation(response.state ?? null);
      setImpact(response.impact ?? impact);
    } catch (caught) {
      if (!isRetryableProductError(caught)) {
        delete mutationIntentKeys.current.activate;
      }
      setError(productErrorMessage(caught));
    } finally {
      setOperation("idle");
    }
  };

  const rollback = async () => {
    if (!activation || activation.rollbackDepth === 0) return;
    setOperation("rolling_back");
    setError(null);
    const idempotencyKey =
      mutationIntentKeys.current.rollback ?? crypto.randomUUID();
    mutationIntentKeys.current.rollback = idempotencyKey;
    try {
      const response = await client.rollbackRevision({
        expectedGeneration: activation.generation,
        idempotencyKey,
      });
      delete mutationIntentKeys.current.rollback;
      delete mutationIntentKeys.current.activate;
      setActivation(response.state ?? null);
      setCandidateRevisionId(null);
      setCandidateWasNew(null);
      setImpact(null);
      setConfirmBreakingChanges(false);
    } catch (caught) {
      if (!isRetryableProductError(caught)) {
        delete mutationIntentKeys.current.rollback;
      }
      setError(productErrorMessage(caught));
    } finally {
      setOperation("idle");
    }
  };

  return (
    <div>
      <PageHeader
        eyebrow="Governed metadata lifecycle"
        title="Admin Studio"
        description="Author a typed object definition, publish an immutable candidate revision, inspect structural impact, activate with optimistic concurrency, and roll back through the governed metadata API."
      />

      {error ? (
        <FeedbackPanel tone="danger" title="Admin Studio request failed">
          {error}
        </FeedbackPanel>
      ) : null}

      <div className="crm-admin-grid">
        <section className="crm-panel" aria-labelledby="object-definition-title">
          <div className="crm-panel-heading">
            <div>
              <p className="crm-eyebrow">Step 1</p>
              <h2 id="object-definition-title">Object definition</h2>
            </div>
            <span className="crm-badge">typed v1</span>
          </div>

          <form className="crm-form-grid" onSubmit={publishCandidate}>
            <label className="crm-field">
              <span>Object ID</span>
              <input
                id="metadata-object-id"
                className="crm-input"
                value={draft.id}
                onChange={(event) => updateDraft("id", event.target.value)}
                disabled={busy}
                autoComplete="off"
              />
            </label>

            <label className="crm-field">
              <span>Owner module ID</span>
              <input
                id="metadata-owner-module-id"
                className="crm-input"
                value={draft.ownerModuleId}
                onChange={(event) =>
                  updateDraft("ownerModuleId", event.target.value)
                }
                disabled={busy}
                autoComplete="off"
              />
            </label>

            <label className="crm-field">
              <span>Singular label</span>
              <input
                id="metadata-label"
                className="crm-input"
                value={draft.label}
                onChange={(event) => updateDraft("label", event.target.value)}
                disabled={busy}
              />
            </label>

            <label className="crm-field">
              <span>Plural label</span>
              <input
                id="metadata-plural-label"
                className="crm-input"
                value={draft.pluralLabel}
                onChange={(event) =>
                  updateDraft("pluralLabel", event.target.value)
                }
                disabled={busy}
              />
            </label>

            <label className="crm-field crm-field-wide">
              <span>Description</span>
              <textarea
                id="metadata-description"
                className="crm-input crm-textarea"
                value={draft.description}
                onChange={(event) =>
                  updateDraft("description", event.target.value)
                }
                disabled={busy}
                rows={4}
              />
            </label>

            <label className="crm-field crm-field-wide">
              <span>Tags</span>
              <input
                id="metadata-tags"
                className="crm-input"
                value={tagsText}
                onChange={(event) => {
                  setTagsText(event.target.value);
                  resetCandidate();
                }}
                disabled={busy}
                placeholder="custom, operations"
              />
              <small>Comma-separated. Duplicate tags are rejected rather than silently changed.</small>
            </label>

            <div className="crm-actions crm-field-wide">
              <button
                id="metadata-publish"
                className="crm-button crm-button-primary"
                type="submit"
                disabled={busy}
              >
                {operation === "publishing" ? "Publishing…" : "Publish candidate"}
              </button>
            </div>
          </form>
        </section>

        <section className="crm-panel" aria-labelledby="candidate-review-title">
          <div className="crm-panel-heading">
            <div>
              <p className="crm-eyebrow">Steps 2–4</p>
              <h2 id="candidate-review-title">Review and activation</h2>
            </div>
          </div>

          {!candidateRevisionId ? (
            <p className="crm-muted">Publish a typed candidate bundle to begin impact review.</p>
          ) : (
            <div className="crm-stack">
              <div>
                <p className="crm-field-label">Candidate revision</p>
                <code className="crm-revision-code" data-testid="metadata-candidate-revision">
                  {candidateRevisionId}
                </code>
                <p className="crm-muted">
                  {candidateWasNew
                    ? "A new immutable revision was published."
                    : "The same immutable content already existed for this tenant and was reused idempotently."}
                </p>
              </div>

              <div className="crm-actions">
                <button
                  id="metadata-review-impact"
                  className="crm-button"
                  type="button"
                  onClick={reviewImpact}
                  disabled={busy}
                >
                  {operation === "loading_impact" ? "Reviewing…" : "Review impact"}
                </button>
              </div>

              {impact ? (
                <div className="crm-impact" data-testid="metadata-impact">
                  <div className="crm-panel-heading">
                    <h3>Structural impact</h3>
                    <span className={`crm-badge ${impact.hasBreakingChanges ? "crm-badge-danger" : ""}`}>
                      {impact.hasBreakingChanges ? "Breaking changes" : "No breaking changes"}
                    </span>
                  </div>
                  <p className="crm-muted">
                    {impact.changes.length === 0
                      ? "The candidate is structurally identical to the active revision."
                      : `${impact.changes.length} structural change${impact.changes.length === 1 ? "" : "s"} detected.`}
                  </p>
                  <ul className="crm-impact-list">
                    {impact.changes.map((change, index) => (
                      <li key={`${change.changeType}-${change.key?.id ?? index}`}>
                        <strong>{metadataChangeTypeLabel(change.changeType)}</strong>
                        {" · "}
                        {metadataKindLabel(change.key?.kind)}
                        {" · "}
                        {change.key?.id ?? "unknown metadata"}
                        {" · "}
                        {metadataImpactSeverityLabel(change.severity)}
                      </li>
                    ))}
                  </ul>

                  {impact.hasBreakingChanges ? (
                    <label className="crm-confirmation">
                      <input
                        id="metadata-confirm-breaking"
                        type="checkbox"
                        checked={confirmBreakingChanges}
                        onChange={(event) =>
                          setConfirmBreakingChanges(event.target.checked)
                        }
                        disabled={busy}
                      />
                      <span>I reviewed the impact and explicitly confirm the breaking changes.</span>
                    </label>
                  ) : null}

                  <div className="crm-actions">
                    <button
                      id="metadata-activate"
                      className="crm-button crm-button-primary"
                      type="button"
                      onClick={activateCandidate}
                      disabled={
                        busy ||
                        candidateIsActive ||
                        (impact.hasBreakingChanges && !confirmBreakingChanges)
                      }
                    >
                      {candidateIsActive
                        ? "Revision is active"
                        : operation === "activating"
                          ? "Activating…"
                          : "Activate revision"}
                    </button>
                  </div>
                </div>
              ) : null}
            </div>
          )}
        </section>
      </div>

      <section className="crm-panel crm-activation-panel" aria-labelledby="activation-title">
        <div className="crm-panel-heading">
          <div>
            <p className="crm-eyebrow">Current tenant state</p>
            <h2 id="activation-title">Active metadata revision</h2>
          </div>
          {activation ? <span className="crm-badge">Generation {activation.generation.toString()}</span> : null}
        </div>

        {operation === "loading_activation" ? (
          <p className="crm-muted">Loading activation state…</p>
        ) : activation ? (
          <div className="crm-stack">
            <code className="crm-revision-code" data-testid="metadata-active-revision">
              {activation.activeRevisionId}
            </code>
            <p className="crm-muted">
              Rollback depth: {activation.rollbackDepth.toString()}. Optimistic generation: {activation.generation.toString()}.
            </p>
            <div className="crm-actions">
              <button
                id="metadata-rollback"
                className="crm-button"
                type="button"
                onClick={rollback}
                disabled={busy || activation.rollbackDepth === 0}
              >
                {operation === "rolling_back" ? "Rolling back…" : "Roll back one revision"}
              </button>
            </div>
          </div>
        ) : (
          <p className="crm-muted">No metadata revision is active for this tenant yet.</p>
        )}
      </section>
    </div>
  );
}

function metadataKindLabel(value: number | undefined): string {
  switch (value) {
    case 1:
      return "Object";
    case 2:
      return "Field";
    case 3:
      return "Relationship";
    case 4:
      return "Layout";
    case 5:
      return "View";
    case 6:
      return "Pipeline";
    case 7:
      return "Permission";
    case 8:
      return "Workflow";
    default:
      return "Unknown kind";
  }
}

function metadataChangeTypeLabel(value: number): string {
  switch (value) {
    case 1:
      return "Added";
    case 2:
      return "Modified";
    case 3:
      return "Removed";
    default:
      return "Unknown change";
  }
}

function metadataImpactSeverityLabel(value: number): string {
  switch (value) {
    case 1:
      return "Informational";
    case 2:
      return "Review required";
    case 3:
      return "Breaking";
    default:
      return "Unspecified";
  }
}

function isRetryableProductError(caught: unknown): boolean {
  return caught instanceof ProductClientError && caught.retryable;
}

function productErrorMessage(caught: unknown): string {
  if (caught instanceof MetadataAuthoringError) {
    return caught.message;
  }
  if (!(caught instanceof ProductClientError)) {
    return "An unexpected error occurred. Review the input and try again.";
  }

  switch (caught.kind) {
    case "unauthenticated":
      return "Your session is no longer available. Sign in again before changing metadata.";
    case "permission_denied":
      return "You do not have permission to perform this metadata operation.";
    case "not_found":
      return "The requested metadata revision was not found or is not visible in this tenant.";
    case "invalid_argument":
      return "The metadata request failed validation. Review the typed definition and try again.";
    case "conflict":
      return "Metadata changed concurrently. Reload the current activation state before retrying.";
    case "rate_limited":
      return "Too many metadata requests were submitted. Try again after the rate limit clears.";
    case "unavailable":
    case "network":
      return "The CRM service is temporarily unavailable. No local state was treated as authoritative.";
    case "internal":
      return "The metadata response failed a governed contract check. The operation was not trusted.";
    default:
      return "The governed metadata request failed.";
  }
}
