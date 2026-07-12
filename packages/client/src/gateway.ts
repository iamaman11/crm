import {
  Code,
  ConnectError,
  createClient,
  type Interceptor,
  type Client,
} from "@connectrpc/connect";
import { createGrpcWebTransport } from "@connectrpc/connect-web";
import { create, toBinary, fromBinary } from "@bufbuild/protobuf";
import { ApplicationGatewayService, TypedPayloadSchema } from "../gen/crm/gateway/v1/gateway_pb";
import { SearchRequestSchema, SearchResponseSchema, type SearchHit } from "../gen/crm/search/v1/search_pb";
import { CONTRACT_HASHES } from "./contract_hashes";
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
  public readonly safeCode: string | undefined;

  public constructor(options: {
    kind: ProductClientErrorKind;
    message: string;
    retryable: boolean;
    cause?: unknown;
    safeCode?: string;
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

export interface SearchGlobalOptions {
  text: string;
  resourceTypes: string[];
  pageSize: number;
  cursor: string;
}

export interface SearchGlobalResult {
  hits: SearchHit[];
  nextCursor: string;
}

export class GovernedClient {
  private readonly gatewayClient: Client<typeof ApplicationGatewayService>;
  private readonly sessionProvider: SessionProvider;

  public constructor(options: GovernedGatewayClientOptions) {
    this.sessionProvider = options.sessionProvider;
    const idFactory = options.idFactory ?? defaultRequestId;
    const sessionInterceptor: Interceptor = (next) => async (request) => {
      const session = this.sessionProvider.getSnapshot();
      const requestId = idFactory();

      if (session.status === "authenticated") {
        request.header.set("authorization", `Bearer ${session.bearerToken}`);
        request.header.set(TENANT_HEADER, session.tenantId);
      }
      request.header.set(REQUEST_ID_HEADER, requestId);
      request.header.set(CORRELATION_ID_HEADER, requestId);
      request.header.set(TRACE_ID_HEADER, requestId);

      return await next(request);
    };

    const transport = createGrpcWebTransport({
      baseUrl: normalizeBaseUrl(options.baseUrl),
      interceptors: [sessionInterceptor],
    });

    this.gatewayClient = createClient(ApplicationGatewayService, transport);
  }

  public async searchGlobal(options: SearchGlobalOptions): Promise<SearchGlobalResult> {
    try {
      requireAuthenticatedSession(this.sessionProvider.getSnapshot());

      const messageName = "crm.search.v1.SearchRequest";
      const descriptorHash = CONTRACT_HASHES[messageName];
      if (!descriptorHash) {
        throw new ProductClientError({
          kind: "internal",
          message: `Missing local contract descriptor hash for ${messageName}`,
          retryable: false,
        });
      }

      const searchRequest = create(SearchRequestSchema, {
        text: options.text,
        resourceTypes: options.resourceTypes,
        pageSize: options.pageSize,
        cursor: options.cursor,
      });

      const payloadBytes = toBinary(SearchRequestSchema, searchRequest);

      const inputPayload = create(TypedPayloadSchema, {
        ownerModuleId: "crm.search",
        schemaId: "crm.search.v1.SearchRequest",
        schemaVersion: "1.0.0",
        descriptorHash,
        dataClass: "confidential",
        encoding: "protobuf",
        maximumSizeBytes: 1048576n,
        retentionPolicyId: "standard",
        payload: payloadBytes,
      });

      const response = await this.gatewayClient.query({
        ownerModuleId: "crm.search",
        capabilityId: "search.global.query",
        capabilityVersion: "1.0.0",
        input: inputPayload,
      });

      if (!response.output) {
        throw new ProductClientError({
          kind: "internal",
          message: "Gateway response did not contain an output payload.",
          retryable: false,
        });
      }

      const output = response.output;

      if (
        !output.ownerModuleId ||
        !output.schemaId ||
        !output.schemaVersion ||
        !output.descriptorHash ||
        output.descriptorHash.length === 0 ||
        !output.dataClass ||
        !output.encoding ||
        !output.retentionPolicyId
      ) {
        throw new ProductClientError({
          kind: "internal",
          message: "Contract verification failed: missing or invalid contract identity fields",
          retryable: false,
        });
      }

      if (output.ownerModuleId !== "crm.search") {
        throw new ProductClientError({
          kind: "internal",
          message: `Contract verification failed: expected ownerModuleId "crm.search", got "${output.ownerModuleId}"`,
          retryable: false,
        });
      }
      if (output.schemaId !== "crm.search.v1.SearchResponse") {
        throw new ProductClientError({
          kind: "internal",
          message: `Contract verification failed: expected schemaId "crm.search.v1.SearchResponse", got "${output.schemaId}"`,
          retryable: false,
        });
      }
      if (output.schemaVersion !== "1.0.0") {
        throw new ProductClientError({
          kind: "internal",
          message: `Contract verification failed: expected schemaVersion "1.0.0", got "${output.schemaVersion}"`,
          retryable: false,
        });
      }
      if (output.dataClass !== "confidential") {
        throw new ProductClientError({
          kind: "internal",
          message: `Contract verification failed: expected dataClass "confidential", got "${output.dataClass}"`,
          retryable: false,
        });
      }
      if (output.encoding !== "protobuf") {
        throw new ProductClientError({
          kind: "internal",
          message: `Contract verification failed: expected encoding "protobuf", got "${output.encoding}"`,
          retryable: false,
        });
      }

      const expectedResponseName = "crm.search.v1.SearchResponse";
      const expectedResponseHash = CONTRACT_HASHES[expectedResponseName];
      if (!expectedResponseHash) {
        throw new ProductClientError({
          kind: "internal",
          message: `Missing local contract descriptor hash for ${expectedResponseName}`,
          retryable: false,
        });
      }
      if (!equalUint8Arrays(output.descriptorHash, expectedResponseHash)) {
        throw new ProductClientError({
          kind: "internal",
          message: `Contract drift detected! Expected ${expectedResponseName} descriptor hash does not match server response.`,
          retryable: false,
        });
      }

      if (output.maximumSizeBytes !== 1048576n) {
        throw new ProductClientError({
          kind: "internal",
          message: `Contract verification failed: expected maximumSizeBytes to be 1048576, got ${output.maximumSizeBytes}`,
          retryable: false,
        });
      }
      if (BigInt(output.payload.length) > output.maximumSizeBytes) {
        throw new ProductClientError({
          kind: "internal",
          message: `Contract verification failed: payload size ${output.payload.length} exceeds maximumSizeBytes ${output.maximumSizeBytes}`,
          retryable: false,
        });
      }

      if (output.retentionPolicyId !== "standard") {
        throw new ProductClientError({
          kind: "internal",
          message: `Contract verification failed: expected retentionPolicyId "standard", got "${output.retentionPolicyId}"`,
          retryable: false,
        });
      }

      let searchResponse;
      try {
        searchResponse = fromBinary(SearchResponseSchema, output.payload);
      } catch (err) {
        throw new ProductClientError({
          kind: "internal",
          message: "Contract verification failed: malformed payload - could not decode SearchResponse",
          retryable: false,
          cause: err,
        });
      }

      return {
        hits: searchResponse.hits,
        nextCursor: searchResponse.nextCursor,
      };
    } catch (error) {
      throw mapGatewayError(error);
    }
  }
}

function equalUint8Arrays(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return false;
  }
  return true;
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

  if (safeCode) {
    switch (safeCode) {
      case "AUTHENTICATION_REQUIRED":
      case "AUTHENTICATION_INVALID":
      case "AUTHENTICATION_EXPIRED":
      case "AUTHENTICATION_REVOKED":
        return productError("unauthenticated", false, safeCode, error);
      case "AUTHENTICATION_UNAVAILABLE":
        return productError("unavailable", true, safeCode, error);
      case "TENANT_REQUIRED":
      case "TENANT_INVALID":
      case "TENANT_FORBIDDEN":
      case "CAPABILITY_PERMISSION_DENIED":
      case "QUERY_PERMISSION_DENIED":
        return productError("permission_denied", false, safeCode, error);
      case "CAPABILITY_RATE_LIMITED":
        return productError("rate_limited", true, safeCode, error);
      case "QUERY_DEADLINE_EXCEEDED":
      case "CAPABILITY_DEADLINE_EXCEEDED":
        return productError("unavailable", true, safeCode, error);
    }
  }

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
  const options = {
    kind,
    message: cause.rawMessage || "The CRM request failed.",
    retryable,
    cause,
  };
  return safeCode === undefined
    ? new ProductClientError(options)
    : new ProductClientError({ ...options, safeCode });
}

function normalizeBaseUrl(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}

function defaultRequestId(): string {
  return globalThis.crypto.randomUUID();
}
