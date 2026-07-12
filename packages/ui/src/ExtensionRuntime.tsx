import {
  Component,
  Suspense,
  lazy,
  useMemo,
  useState,
  type ComponentType,
  type ErrorInfo,
  type ReactNode,
} from "react";

export const UI_EXTENSION_SURFACES = [
  "record.detail.main",
  "record.detail.sidebar",
] as const;

export type UiExtensionSurface = (typeof UI_EXTENSION_SURFACES)[number];

export interface UiExtensionProps<Context> {
  context: Readonly<Context>;
}

export type UiExtensionComponent<Context> = ComponentType<
  UiExtensionProps<Context>
>;

export interface UiExtensionModule<Context> {
  default: UiExtensionComponent<Context>;
}

export interface UiExtensionDefinition<Context> {
  id: string;
  ownerModuleId: string;
  surface: UiExtensionSurface;
  order?: number;
  load: () => Promise<UiExtensionModule<Context>>;
}

export type UiExtensionRegistrationErrorCode =
  | "INVALID_EXTENSION_ID"
  | "INVALID_OWNER_MODULE_ID"
  | "INVALID_SURFACE"
  | "INVALID_ORDER"
  | "DUPLICATE_COORDINATE";

export class UiExtensionRegistrationError extends Error {
  public readonly code: UiExtensionRegistrationErrorCode;

  public constructor(code: UiExtensionRegistrationErrorCode, message: string) {
    super(message);
    this.name = "UiExtensionRegistrationError";
    this.code = code;
  }
}

export type UiExtensionFailurePhase = "load" | "render";

export interface UiExtensionFailureEvent {
  code: "UI_EXTENSION_LOAD_FAILED" | "UI_EXTENSION_RENDER_FAILED";
  coordinate: string;
  extensionId: string;
  ownerModuleId: string;
  surface: UiExtensionSurface;
  phase: UiExtensionFailurePhase;
  attempt: number;
}

interface NormalizedUiExtensionDefinition<Context>
  extends UiExtensionDefinition<Context> {
  order: number;
}

const textEncoder = new TextEncoder();
const MAX_IDENTIFIER_BYTES = 180;
const MAX_ORDER = 10_000;
const IDENTIFIER_PATTERN = /^[a-z][a-z0-9]*(?:[._-][a-z0-9]+)*$/;

export class UiExtensionRegistry<Context> {
  readonly #definitions: readonly NormalizedUiExtensionDefinition<Context>[];

  public constructor(definitions: readonly UiExtensionDefinition<Context>[]) {
    const coordinates = new Set<string>();
    const normalized = definitions.map((definition) => {
      const next = normalizeDefinition(definition);
      const coordinate = uiExtensionCoordinate(next);
      if (!coordinates.add(coordinate)) {
        throw new UiExtensionRegistrationError(
          "DUPLICATE_COORDINATE",
          `Duplicate UI extension coordinate: ${coordinate}`,
        );
      }
      return next;
    });

    this.#definitions = Object.freeze(
      normalized.sort((left, right) => {
        if (left.surface !== right.surface) {
          return left.surface.localeCompare(right.surface);
        }
        if (left.order !== right.order) {
          return left.order - right.order;
        }
        return uiExtensionCoordinate(left).localeCompare(
          uiExtensionCoordinate(right),
        );
      }),
    );
  }

  public forSurface(
    surface: UiExtensionSurface,
  ): readonly UiExtensionDefinition<Context>[] {
    assertSupportedSurface(surface);
    return this.#definitions.filter(
      (definition) => definition.surface === surface,
    );
  }
}

export function createUiExtensionRegistry<Context>(
  definitions: readonly UiExtensionDefinition<Context>[],
): UiExtensionRegistry<Context> {
  return new UiExtensionRegistry(definitions);
}

export function uiExtensionCoordinate<Context>(
  definition: Pick<
    UiExtensionDefinition<Context>,
    "id" | "ownerModuleId" | "surface"
  >,
): string {
  return `${definition.ownerModuleId}:${definition.id}@${definition.surface}`;
}

export interface UiExtensionSlotProps<Context> {
  registry: UiExtensionRegistry<Context>;
  surface: UiExtensionSurface;
  context: Readonly<Context>;
  onFailure?: (event: UiExtensionFailureEvent) => void;
  emptyFallback?: ReactNode;
  loadingFallback?: ReactNode;
}

export function UiExtensionSlot<Context>({
  registry,
  surface,
  context,
  onFailure,
  emptyFallback = null,
  loadingFallback,
}: UiExtensionSlotProps<Context>) {
  const definitions = registry.forSurface(surface);
  if (definitions.length === 0) {
    return <>{emptyFallback}</>;
  }

  return (
    <div className="crm-extension-slot" data-extension-surface={surface}>
      {definitions.map((definition) => (
        <UiExtensionInstance
          key={uiExtensionCoordinate(definition)}
          definition={definition}
          context={context}
          onFailure={onFailure}
          loadingFallback={loadingFallback}
        />
      ))}
    </div>
  );
}

interface UiExtensionInstanceProps<Context> {
  definition: UiExtensionDefinition<Context>;
  context: Readonly<Context>;
  onFailure?: (event: UiExtensionFailureEvent) => void;
  loadingFallback?: ReactNode;
}

function UiExtensionInstance<Context>({
  definition,
  context,
  onFailure,
  loadingFallback,
}: UiExtensionInstanceProps<Context>) {
  const [attempt, setAttempt] = useState(1);
  const LazyExtension = useMemo(
    () =>
      lazy(async () => {
        try {
          return await definition.load();
        } catch {
          throw new UiExtensionLoadError();
        }
      }),
    [attempt, definition],
  );
  const coordinate = uiExtensionCoordinate(definition);

  const reportFailure = (phase: UiExtensionFailurePhase) => {
    onFailure?.({
      code:
        phase === "load"
          ? "UI_EXTENSION_LOAD_FAILED"
          : "UI_EXTENSION_RENDER_FAILED",
      coordinate,
      extensionId: definition.id,
      ownerModuleId: definition.ownerModuleId,
      surface: definition.surface,
      phase,
      attempt,
    });
  };

  return (
    <UiExtensionErrorBoundary
      key={attempt}
      coordinate={coordinate}
      onFailure={reportFailure}
      onRetry={() => setAttempt((current) => current + 1)}
    >
      <Suspense
        fallback={
          loadingFallback ?? (
            <div className="crm-extension-loading" role="status">
              Loading extension…
            </div>
          )
        }
      >
        <LazyExtension context={context} />
      </Suspense>
    </UiExtensionErrorBoundary>
  );
}

interface UiExtensionErrorBoundaryProps {
  coordinate: string;
  onFailure: (phase: UiExtensionFailurePhase) => void;
  onRetry: () => void;
  children: ReactNode;
}

interface UiExtensionErrorBoundaryState {
  failed: boolean;
  phase: UiExtensionFailurePhase;
}

class UiExtensionErrorBoundary extends Component<
  UiExtensionErrorBoundaryProps,
  UiExtensionErrorBoundaryState
> {
  public state: UiExtensionErrorBoundaryState = {
    failed: false,
    phase: "render",
  };

  #reported = false;

  public static getDerivedStateFromError(
    error: unknown,
  ): UiExtensionErrorBoundaryState {
    return {
      failed: true,
      phase: error instanceof UiExtensionLoadError ? "load" : "render",
    };
  }

  public componentDidCatch(_error: Error, _info: ErrorInfo) {
    if (!this.#reported) {
      this.#reported = true;
      this.props.onFailure(this.state.phase);
    }
  }

  public render() {
    if (this.state.failed) {
      return (
        <section
          className="crm-extension-fallback"
          data-testid="ui-extension-fallback"
          data-extension-coordinate={this.props.coordinate}
          role="status"
        >
          <div>
            <h3>Extension unavailable</h3>
            <p>
              This extension failed in isolation. The host page and other
              extensions remain available.
            </p>
          </div>
          <button
            className="crm-button crm-button-secondary"
            type="button"
            data-testid="ui-extension-retry"
            onClick={this.props.onRetry}
          >
            Retry extension
          </button>
        </section>
      );
    }

    return this.props.children;
  }
}

class UiExtensionLoadError extends Error {
  public constructor() {
    super("UI extension load failed");
    this.name = "UiExtensionLoadError";
  }
}

function normalizeDefinition<Context>(
  definition: UiExtensionDefinition<Context>,
): NormalizedUiExtensionDefinition<Context> {
  validateIdentifier(
    definition.id,
    "INVALID_EXTENSION_ID",
    "UI extension ID",
  );
  validateIdentifier(
    definition.ownerModuleId,
    "INVALID_OWNER_MODULE_ID",
    "owner module ID",
  );
  assertSupportedSurface(definition.surface);

  const order = definition.order ?? 0;
  if (!Number.isSafeInteger(order) || Math.abs(order) > MAX_ORDER) {
    throw new UiExtensionRegistrationError(
      "INVALID_ORDER",
      `UI extension order must be a safe integer between -${MAX_ORDER} and ${MAX_ORDER}.`,
    );
  }

  return Object.freeze({ ...definition, order });
}

function validateIdentifier(
  value: string,
  code: Extract<
    UiExtensionRegistrationErrorCode,
    "INVALID_EXTENSION_ID" | "INVALID_OWNER_MODULE_ID"
  >,
  label: string,
) {
  if (
    !IDENTIFIER_PATTERN.test(value) ||
    textEncoder.encode(value).length > MAX_IDENTIFIER_BYTES
  ) {
    throw new UiExtensionRegistrationError(
      code,
      `${label} must be a lowercase dotted identifier of at most ${MAX_IDENTIFIER_BYTES} UTF-8 bytes.`,
    );
  }
}

function assertSupportedSurface(surface: UiExtensionSurface) {
  if (!(UI_EXTENSION_SURFACES as readonly string[]).includes(surface)) {
    throw new UiExtensionRegistrationError(
      "INVALID_SURFACE",
      `Unsupported UI extension surface: ${String(surface)}`,
    );
  }
}
