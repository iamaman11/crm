import { describe, it, expect, vi } from "vitest";
import {
  MutableSessionStore,
  SessionUnavailableError,
  mapGatewayError,
  ProductClientError,
  GovernedClient,
} from "./index";
import { ConnectError, Code } from "@connectrpc/connect";
import {
  TypedPayloadSchema,
  QueryResponseSchema,
  QueryRequestSchema,
  type TypedPayload,
} from "../gen/crm/gateway/v1/gateway_pb";
import { SearchResponseSchema } from "../gen/crm/search/v1/search_pb";
import { create, toBinary, fromBinary } from "@bufbuild/protobuf";
import { CONTRACT_HASHES } from "./contract_hashes";

const testSessionProvider = {
  getSnapshot: () => ({
    status: "authenticated" as const,
    bearerToken: "valid-token",
    tenantId: "tenant-a",
  }),
  subscribe: () => () => {},
};

function encodeGrpcWebFrame(payload: Uint8Array, isTrailer = false): Uint8Array {
  const frame = new Uint8Array(5 + payload.length);
  frame[0] = isTrailer ? 0x80 : 0x00;
  const view = new DataView(frame.buffer);
  view.setUint32(1, payload.length, false);
  frame.set(payload, 5);
  return frame;
}

function decodeGrpcWebFrame(body: Uint8Array): Uint8Array {
  return body.slice(5);
}

describe("Session State and Session Store", () => {
  it("initializes with default unknown status", () => {
    const store = new MutableSessionStore();
    expect(store.getSnapshot()).toEqual({ status: "unknown" });
  });

  it("notifies subscribers on status change", () => {
    const store = new MutableSessionStore({ status: "unauthenticated" });
    const listener = vi.fn();
    const unsubscribe = store.subscribe(listener);

    store.setState({
      status: "authenticated",
      bearerToken: "token-abc",
      tenantId: "tenant-123",
    });

    expect(listener).toHaveBeenCalledTimes(1);
    expect(store.getSnapshot().status).toBe("authenticated");

    unsubscribe();
    store.setState({ status: "expired" });
    expect(listener).toHaveBeenCalledTimes(1);
  });
});

describe("Gateway Error Mapping", () => {
  it("maps SessionUnavailableError to unauthenticated ProductClientError", () => {
    const sessionErr = new SessionUnavailableError("expired");
    const mapped = mapGatewayError(sessionErr);
    expect(mapped).toBeInstanceOf(ProductClientError);
    expect(mapped.kind).toBe("unauthenticated");
    expect(mapped.retryable).toBe(false);
  });

  it("maps generic Error to network ProductClientError", () => {
    const mapped = mapGatewayError(new Error("Broken pipe"));
    expect(mapped).toBeInstanceOf(ProductClientError);
    expect(mapped.kind).toBe("network");
    expect(mapped.retryable).toBe(true);
  });

  it("maps ConnectError correctly based on custom x-error-code metadata", () => {
    const err = new ConnectError("Denied", Code.PermissionDenied);
    err.metadata.set("x-error-code", "TENANT_FORBIDDEN");
    const mapped = mapGatewayError(err);
    expect(mapped.kind).toBe("permission_denied");
    expect(mapped.safeCode).toBe("TENANT_FORBIDDEN");
    expect(mapped.retryable).toBe(false);

    const expiredErr = new ConnectError("Expired", Code.Unauthenticated);
    expiredErr.metadata.set("x-error-code", "AUTHENTICATION_EXPIRED");
    const mappedExpired = mapGatewayError(expiredErr);
    expect(mappedExpired.kind).toBe("unauthenticated");
    expect(mappedExpired.safeCode).toBe("AUTHENTICATION_EXPIRED");
  });

  it("maps ConnectError correctly based on standard Connect Code fallback", () => {
    const unauthConnectErr = new ConnectError("Unauth", Code.Unauthenticated);
    const mapped = mapGatewayError(unauthConnectErr);
    expect(mapped.kind).toBe("unauthenticated");
    expect(mapped.retryable).toBe(false);

    const abortedConnectErr = new ConnectError("Transaction aborted", Code.Aborted);
    const mappedConflict = mapGatewayError(abortedConnectErr);
    expect(mappedConflict.kind).toBe("conflict");
    expect(mappedConflict.retryable).toBe(false);

    const unavailableErr = new ConnectError("Server shut down", Code.Unavailable);
    const mappedUnavailable = mapGatewayError(unavailableErr);
    expect(mappedUnavailable.kind).toBe("unavailable");
    expect(mappedUnavailable.retryable).toBe(true);
  });
});

describe("GovernedClient Session Validation", () => {
  it("throws unauthenticated ProductClientError when session status is unauthenticated", async () => {
    const client = new GovernedClient({
      baseUrl: "http://mock",
      sessionProvider: {
        getSnapshot: () => ({ status: "unauthenticated" as const }),
        subscribe: () => () => {},
      },
    });

    await expect(
      client.searchGlobal({ text: "", resourceTypes: [], pageSize: 10, cursor: "" })
    ).rejects.toThrowError(
      new ProductClientError({
        kind: "unauthenticated",
        message: "Your session is not available. Sign in again.",
        retryable: false,
      })
    );
  });

  it("throws unauthenticated ProductClientError when session status is expired", async () => {
    const client = new GovernedClient({
      baseUrl: "http://mock",
      sessionProvider: {
        getSnapshot: () => ({ status: "expired" as const }),
        subscribe: () => () => {},
      },
    });

    await expect(
      client.searchGlobal({ text: "", resourceTypes: [], pageSize: 10, cursor: "" })
    ).rejects.toThrowError(
      new ProductClientError({
        kind: "unauthenticated",
        message: "Your session is not available. Sign in again.",
        retryable: false,
      })
    );
  });

  it("throws unauthenticated ProductClientError when session status is revoked", async () => {
    const client = new GovernedClient({
      baseUrl: "http://mock",
      sessionProvider: {
        getSnapshot: () => ({ status: "revoked" as const }),
        subscribe: () => () => {},
      },
    });

    await expect(
      client.searchGlobal({ text: "", resourceTypes: [], pageSize: 10, cursor: "" })
    ).rejects.toThrowError(
      new ProductClientError({
        kind: "unauthenticated",
        message: "Your session is not available. Sign in again.",
        retryable: false,
      })
    );
  });
});

describe("GovernedClient Governed Search API", () => {
  const responseHash = CONTRACT_HASHES["crm.search.v1.SearchResponse"] ?? new Uint8Array();

  function createMockResponse(outputOverrides: Partial<Omit<TypedPayload, "$type">> = {}): Uint8Array {
    const defaultFields = {
      ownerModuleId: "crm.search",
      schemaId: "crm.search.v1.SearchResponse",
      schemaVersion: "1.0.0",
      dataClass: "confidential",
      descriptorHash: responseHash,
      encoding: "protobuf",
      maximumSizeBytes: 1048576n,
      retentionPolicyId: "standard",
      payload: new Uint8Array(),
    };
    const output = create(TypedPayloadSchema, {
      ...defaultFields,
      ...outputOverrides,
    } as Parameters<typeof create<typeof TypedPayloadSchema>>[1]);

    const queryResponse = create(QueryResponseSchema, { output });
    return toBinary(QueryResponseSchema, queryResponse);
  }

  function createMockFetch(
    outputOverrides: Partial<Omit<TypedPayload, "$type">> = {},
    fetchError?: ConnectError,
    captureFn?: (headers: Headers, requestBytes: Uint8Array) => void
  ): typeof fetch {
    return (async (_url: string, init?: RequestInit) => {
      const headers = new Headers(init?.headers);
      const requestBody = init?.body instanceof Uint8Array ? init.body : new Uint8Array();
      
      if (captureFn) {
        captureFn(headers, requestBody);
      }

      if (fetchError) {
        const trailerText = `grpc-status: ${fetchError.code}\r\ngrpc-message: ${encodeURIComponent(fetchError.rawMessage)}\r\n`;
        const headersInit: Record<string, string> = {
          "content-type": "application/grpc-web+proto",
          "grpc-status": String(fetchError.code),
          "grpc-message": encodeURIComponent(fetchError.rawMessage),
        };
        const errorCode = fetchError.metadata.get("x-error-code");
        if (errorCode) {
          headersInit["x-error-code"] = errorCode;
        }
        
        const trailerBytes = encodeGrpcWebFrame(new TextEncoder().encode(trailerText), true);
        return new Response(new Blob([trailerBytes.buffer as ArrayBuffer]), {
          status: 200,
          headers: new Headers(headersInit),
        });
      }

      const queryResponseBytes = createMockResponse(outputOverrides);
      const bodyBytes = encodeGrpcWebFrame(queryResponseBytes);
      const trailerBytes = encodeGrpcWebFrame(new TextEncoder().encode("grpc-status: 0\r\n"), true);

      const responseBytes = new Uint8Array(bodyBytes.length + trailerBytes.length);
      responseBytes.set(bodyBytes);
      responseBytes.set(trailerBytes, bodyBytes.length);

      return new Response(new Blob([responseBytes.buffer as ArrayBuffer]), {
        status: 200,
        headers: new Headers({
          "content-type": "application/grpc-web+proto",
        }),
      });
    }) as typeof fetch;
  }

  it("emits the exact governed coordinates and parses valid search response via custom fetch", async () => {
    let capturedHeaders: Headers | null = null;
    let capturedBodyBytes: Uint8Array | null = null;

    const searchResponse = create(SearchResponseSchema, {
      hits: [
        {
          ownerModuleId: "crm.sales",
          resourceType: "sales.deal",
          resourceId: "deal-1",
          fields: { name: "Test Deal" },
          matchedFields: ["name"],
        }
      ],
      nextCursor: "next-page",
    });
    const payloadBytes = toBinary(SearchResponseSchema, searchResponse);
    
    const successFetch = createMockFetch(
      { payload: payloadBytes },
      undefined,
      (headers, body) => {
        capturedHeaders = headers;
        capturedBodyBytes = body;
      }
    );
    
    const clientWithPayload = new GovernedClient({
      baseUrl: "http://mock",
      sessionProvider: testSessionProvider,
      _testFetch: successFetch,
    });

    const result = await clientWithPayload.searchGlobal({
      text: "query text",
      resourceTypes: ["sales.deal"],
      pageSize: 10,
      cursor: "",
    });

    expect(result.hits).toHaveLength(1);
    expect(result.hits[0]?.resourceId).toBe("deal-1");
    expect(result.nextCursor).toBe("next-page");

    // Verify governed coordinates in request payload
    expect(capturedBodyBytes).not.toBeNull();
    const queryRequestPayload = decodeGrpcWebFrame(capturedBodyBytes!);
    const queryRequest = fromBinary(QueryRequestSchema, queryRequestPayload);
    expect(queryRequest.ownerModuleId).toBe("crm.search");
    expect(queryRequest.capabilityId).toBe("search.global.query");
    expect(queryRequest.capabilityVersion).toBe("1.0.0");

    // Verify session headers injected by interceptor
    expect(capturedHeaders).not.toBeNull();
    expect(capturedHeaders!.get("authorization")).toBe("Bearer valid-token");
    expect(capturedHeaders!.get("x-tenant-id")).toBe("tenant-a");
    expect(capturedHeaders!.get("x-request-id")).toBeDefined();
  });

  it("safely handles output contract mismatch (drift detection)", async () => {
    const mockFetch = createMockFetch({ descriptorHash: new Uint8Array(32) });
    const client = new GovernedClient({
      baseUrl: "http://mock",
      sessionProvider: testSessionProvider,
      _testFetch: mockFetch,
    });

    await expect(
      client.searchGlobal({
        text: "query text",
        resourceTypes: ["sales.deal"],
        pageSize: 10,
        cursor: "",
      })
    ).rejects.toThrow("Contract drift detected");
  });

  it("rejects response with wrong ownerModuleId", async () => {
    const mockFetch = createMockFetch({ ownerModuleId: "crm.wrong" });
    const client = new GovernedClient({
      baseUrl: "http://mock",
      sessionProvider: testSessionProvider,
      _testFetch: mockFetch,
    });

    await expect(
      client.searchGlobal({ text: "", resourceTypes: [], pageSize: 10, cursor: "" })
    ).rejects.toThrow('expected ownerModuleId "crm.search", got "crm.wrong"');
  });

  it("rejects response with wrong schemaId", async () => {
    const mockFetch = createMockFetch({ schemaId: "crm.search.v1.WrongResponse" });
    const client = new GovernedClient({
      baseUrl: "http://mock",
      sessionProvider: testSessionProvider,
      _testFetch: mockFetch,
    });

    await expect(
      client.searchGlobal({ text: "", resourceTypes: [], pageSize: 10, cursor: "" })
    ).rejects.toThrow('expected schemaId "crm.search.v1.SearchResponse", got "crm.search.v1.WrongResponse"');
  });

  it("rejects response with wrong schemaVersion", async () => {
    const mockFetch = createMockFetch({ schemaVersion: "2.0.0" });
    const client = new GovernedClient({
      baseUrl: "http://mock",
      sessionProvider: testSessionProvider,
      _testFetch: mockFetch,
    });

    await expect(
      client.searchGlobal({ text: "", resourceTypes: [], pageSize: 10, cursor: "" })
    ).rejects.toThrow('expected schemaVersion "1.0.0", got "2.0.0"');
  });

  it("rejects response with wrong dataClass", async () => {
    const mockFetch = createMockFetch({ dataClass: "public" });
    const client = new GovernedClient({
      baseUrl: "http://mock",
      sessionProvider: testSessionProvider,
      _testFetch: mockFetch,
    });

    await expect(
      client.searchGlobal({ text: "", resourceTypes: [], pageSize: 10, cursor: "" })
    ).rejects.toThrow('expected dataClass "confidential", got "public"');
  });

  it("rejects response with wrong encoding", async () => {
    const mockFetch = createMockFetch({ encoding: "json" });
    const client = new GovernedClient({
      baseUrl: "http://mock",
      sessionProvider: testSessionProvider,
      _testFetch: mockFetch,
    });

    await expect(
      client.searchGlobal({ text: "", resourceTypes: [], pageSize: 10, cursor: "" })
    ).rejects.toThrow('expected encoding "protobuf", got "json"');
  });

  it("rejects response with missing/invalid contract identity", async () => {
    const mockFetch = createMockFetch({ schemaId: "" });
    const client = new GovernedClient({
      baseUrl: "http://mock",
      sessionProvider: testSessionProvider,
      _testFetch: mockFetch,
    });

    await expect(
      client.searchGlobal({ text: "", resourceTypes: [], pageSize: 10, cursor: "" })
    ).rejects.toThrow("missing or invalid contract identity fields");
  });

  it("rejects response with incorrect maximumSizeBytes", async () => {
    const mockFetch = createMockFetch({ maximumSizeBytes: 0n });
    const client = new GovernedClient({
      baseUrl: "http://mock",
      sessionProvider: testSessionProvider,
      _testFetch: mockFetch,
    });

    await expect(
      client.searchGlobal({ text: "", resourceTypes: [], pageSize: 10, cursor: "" })
    ).rejects.toThrow("expected maximumSizeBytes to be 1048576, got 0");
  });

  it("rejects response with oversized payload", async () => {
    const mockFetch = createMockFetch({
      maximumSizeBytes: 1048576n,
      payload: new Uint8Array(1048577),
    });
    const client = new GovernedClient({
      baseUrl: "http://mock",
      sessionProvider: testSessionProvider,
      _testFetch: mockFetch,
    });

    await expect(
      client.searchGlobal({ text: "", resourceTypes: [], pageSize: 10, cursor: "" })
    ).rejects.toThrow("payload size 1048577 exceeds maximumSizeBytes 1048576");
  });

  it("rejects response with wrong retentionPolicyId", async () => {
    const mockFetch = createMockFetch({ retentionPolicyId: "short" });
    const client = new GovernedClient({
      baseUrl: "http://mock",
      sessionProvider: testSessionProvider,
      _testFetch: mockFetch,
    });

    await expect(
      client.searchGlobal({ text: "", resourceTypes: [], pageSize: 10, cursor: "" })
    ).rejects.toThrow('expected retentionPolicyId "standard", got "short"');
  });

  it("rejects response with malformed payload (protobuf decoding failure)", async () => {
    const mockFetch = createMockFetch({ payload: new Uint8Array([255, 255, 255, 255]) });
    const client = new GovernedClient({
      baseUrl: "http://mock",
      sessionProvider: testSessionProvider,
      _testFetch: mockFetch,
    });

    await expect(
      client.searchGlobal({ text: "", resourceTypes: [], pageSize: 10, cursor: "" })
    ).rejects.toThrow("malformed payload - could not decode SearchResponse");
  });
});
