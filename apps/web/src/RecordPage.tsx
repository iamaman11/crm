import { useState } from "react";
import {
  PageHeader,
  UiExtensionSlot,
  type UiExtensionFailureEvent,
} from "@ultimate-crm/ui";
import {
  recordExtensionRegistry,
  type RecordExtensionContext,
} from "./recordExtensions";

const RECORD_CONTEXT: Readonly<RecordExtensionContext> = {
  record: {
    recordType: "sales.deal",
    recordId: "phase7i-demo-deal",
    displayName: "Northstar renewal",
    stageLabel: "Proposal",
    amountLabel: "USD 25,000.00",
  },
  locale: "en",
};

export function RecordPage() {
  const [failures, setFailures] = useState<UiExtensionFailureEvent[]>([]);

  const recordFailure = (event: UiExtensionFailureEvent) => {
    setFailures((current) => {
      const alreadyRecorded = current.some(
        (failure) =>
          failure.coordinate === event.coordinate &&
          failure.attempt === event.attempt &&
          failure.code === event.code,
      );
      return alreadyRecorded ? current : [...current, event];
    });
  };

  return (
    <div>
      <PageHeader
        eyebrow="Typed UI extension host"
        title="Record page"
        description="Core record content is owned by the host. Typed extensions receive only bounded host context and fail independently without taking down the shell, record page, or sibling extensions."
      />

      <div className="crm-record-layout">
        <div className="crm-record-core" data-testid="record-core-content">
          <section className="crm-record-panel" aria-labelledby="record-overview-title">
            <h2 id="record-overview-title">Deal overview</h2>
            <dl className="crm-record-fields">
              <div>
                <dt>Name</dt>
                <dd>{RECORD_CONTEXT.record.displayName}</dd>
              </div>
              <div>
                <dt>Record ID</dt>
                <dd>{RECORD_CONTEXT.record.recordId}</dd>
              </div>
              <div>
                <dt>Stage</dt>
                <dd>{RECORD_CONTEXT.record.stageLabel}</dd>
              </div>
              <div>
                <dt>Amount</dt>
                <dd>{RECORD_CONTEXT.record.amountLabel}</dd>
              </div>
            </dl>
          </section>

          <UiExtensionSlot
            registry={recordExtensionRegistry}
            surface="record.detail.main"
            context={RECORD_CONTEXT}
            onFailure={recordFailure}
          />
        </div>

        <aside className="crm-record-sidebar" aria-label="Record extensions">
          <UiExtensionSlot
            registry={recordExtensionRegistry}
            surface="record.detail.sidebar"
            context={RECORD_CONTEXT}
            onFailure={recordFailure}
          />

          <div
            className="crm-extension-evidence"
            data-testid="ui-extension-failure-evidence"
            aria-live="polite"
          >
            <strong>{failures.length}</strong> isolated extension failure
            {failures.length === 1 ? "" : "s"} recorded.
            {failures.length > 0 ? (
              <ul>
                {failures.map((failure) => (
                  <li
                    key={`${failure.coordinate}:${failure.attempt}:${failure.code}`}
                  >
                    {failure.code} · {failure.coordinate} · attempt {failure.attempt}
                  </li>
                ))}
              </ul>
            ) : null}
          </div>
        </aside>
      </div>
    </div>
  );
}
