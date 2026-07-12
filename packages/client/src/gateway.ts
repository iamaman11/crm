import {
  Code,
  ConnectError,
  createClient,
  type Interceptor,
} from "@connectrpc/connect";
import { createGrpcWebTransport } from "@connectrpc/connect-web";
import { ApplicationGatewayService } from "./gen/crm/gateway/v1/gateway_pb";
import {
  requireAuthenticatedSession,
  type SessionProvider,
  SessionUnavailableError,
} from "./session";

const TENANT_HEADER = "x-tenant-id";
const REQUEST_ID_HEADER = "x-request-id";
const CORRELATION_ID_HEADER = "x-correlation-id";
const TRACE_ID_HEADER = "x-trace-id";

export type ProductClientErrorKind =
  | "unauthenticated"
  | "permission_denied"
  | "not_found"
  | "invalid_argument"
  | "conflict"
  | "rate_limited"
  | "unavailable"
  | "network"
  | "internal";

export class ProductClientError extends Error {
  public readonly kind: ProductClientErrorKind;
  public readonly retryable: boolean;
  public readonly safeCode?: string;

  public constructor(options: {
    kind: ProductClientErrorKind;
    message: string;
    retryable: boolean;
    safeCode?: string;
    cause?: unknown;
  }) {
    super(options.message, { cause: options.cause });
    this.name = "ProductClientError";
    this.kind = options.kind;
    this.retryable = options.retryable;
    this.safeCode = options.safeCode;
  }
}

export interface GovernedGatewayClientOptions {
  baseUrl: string;
  sessionProvider: SessionProvider;
  idFactory?: () => string;
}

export function createGovernedGatewayClient(options: GovernedGatewayClientOptions) {
  const idFactory = options.idFactory ?? defaultRequestId;
  const sessionInterceptor: Interceptor = (next) => async (request) => {
    const session = requireAuthenticatedSession(options.sessionProvider.getSnapshot());
    const requestId = idFactory();

    request.header.set("authorization", `Bearer ${session.bearerToken}`);
    request.header.set(TENANT_HEADER, session.tenantId);
    request.header.set(REQUEST_ID_HEADER, requestId);
    request.header.set(CORRELATION_ID_HEADER, requestId);
    request.header.set(TRACE_ID_HEADER, requestId);

    try {
      return await next(request);
    } catch (error) {
      throw mapGatewayError(error);
    }
  };

  const transport = createGrpcWebTransport({
    baseUrl: normalizeBaseUrl(options.baseUrl),
    interceptors: [sessionInterceptor],
  });

  return createClient(ApplicationGatewayService, transport);
}

export function mapGatewayError(error: unknown): ProductClientError {
  if (error instanceof ProductClientError) {
    return error;
  }
  if (error instanceof SessionUnavailableError) {
    return new ProductClientError({
      kind: "unauthenticated",
      message: "Your session is not available. Sign in again.",
      retryable: false,
      cause: error,
    });
  }
  if (!(error instanceof ConnectError)) {
    return new ProductClientError({
      kind: "network",
      message: "The CRM service could not be reached.",
      retryable: true,
      cause: error,
    });
  }

  const safeCode = error.metadata.get("x-error-code") ?? undefined;
  switch (error.code) {
    case Code.Unauthenticated:
      return productError("unauthenticated", false, safeCode, error);
    case Code.PermissionDenied:
      return productError("permission_denied", false, safeCode, error);
    case Code.NotFound:
      return productError("not_found", false, safeCode, error);
    case Code.InvalidArgument:
      return productError("invalid_argument", false, safeCode, error);
    case Code.Aborted:
    case Code.AlreadyExists:
    case Code.FailedPrecondition:
      return productError("conflict", false, safeCode, error);
    case Code.ResourceExhausted:
      return productError("rate_limited", true, safeCode, error);
    case Code.Unavailable:
    case Code.DeadlineExceeded:
      return productError("unavailable", true, safeCode, error);
    default:
      return productError("internal", false, safeCode, error);
  }
}

function productError(
  kind: ProductClientErrorKind,
  retryable: boolean,
  safeCode: string | undefined,
  cause: ConnectError,
): ProductClientError {
  return new ProductClientError({
    kind,
    message: cause.rawMessage || "The CRM request failed.",
    retryable,
    safeCode,
    cause,
  });
}

function normalizeBaseUrl(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}

function defaultRequestId(): string {
  return globalThis.crypto.randomUUID();
}
