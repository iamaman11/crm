import {
  createUiExtensionRegistry,
  type UiExtensionDefinition,
  type UiExtensionProps,
} from "@ultimate-crm/ui";

export interface RecordExtensionContext {
  record: {
    recordType: "sales.deal";
    recordId: string;
    displayName: string;
    stageLabel: string;
    amountLabel: string;
  };
  locale: "en";
}

function DealSummaryExtension({
  context,
}: UiExtensionProps<RecordExtensionContext>) {
  return (
    <section
      className="crm-extension-card"
      data-testid="healthy-main-extension"
    >
      <h3>Module summary extension</h3>
      <p>
        {context.record.displayName} is currently in {context.record.stageLabel}.
      </p>
    </section>
  );
}

function DealHealthExtension({
  context,
}: UiExtensionProps<RecordExtensionContext>) {
  return (
    <section
      className="crm-extension-card"
      data-testid="healthy-sidebar-extension"
    >
      <h3>Deal health</h3>
      <p>
        Typed host context received for {context.record.recordType} · {context.record.amountLabel}.
      </p>
    </section>
  );
}

function RenderFailureExtension(): never {
  throw new Error("Deliberate Phase 7I render isolation proof");
}

const definitions: readonly UiExtensionDefinition<RecordExtensionContext>[] = [
  {
    id: "deal.summary",
    ownerModuleId: "crm.sales",
    surface: "record.detail.main",
    order: 10,
    load: async () => ({ default: DealSummaryExtension }),
  },
  {
    id: "deal.health",
    ownerModuleId: "crm.sales",
    surface: "record.detail.sidebar",
    order: 10,
    load: async () => ({ default: DealHealthExtension }),
  },
  {
    id: "deal.render-failure-proof",
    ownerModuleId: "crm.activities",
    surface: "record.detail.sidebar",
    order: 20,
    load: async () => ({ default: RenderFailureExtension }),
  },
  {
    id: "deal.load-failure-proof",
    ownerModuleId: "crm.sales-activities-link",
    surface: "record.detail.sidebar",
    order: 30,
    load: async () => {
      throw new Error("Deliberate Phase 7I lazy-load isolation proof");
    },
  },
];

export const recordExtensionRegistry =
  createUiExtensionRegistry<RecordExtensionContext>(definitions);
