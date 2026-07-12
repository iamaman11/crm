import { describe, it, expect, vi } from "vitest";
import {
  MutableSessionStore,
  requireAuthenticatedSession,
  SessionUnavailableError,
  mapGatewayError,
  ProductClientError,
  GovernedClient,
} from "./index";
import { ConnectError, Code, createClient, createRouterTransport } from "@connectrpc/connect";
import { ApplicationGatewayService } from "../gen/crm/gateway/v1/gateway_pb";
import { SearchResponseSchema } from "../gen/crm/search/v1/search_pb";
import { create, toBinary } from "@bufbuild/protobuf";
import { CONTRACT_HASHES } from "./contract_hashes";

const testSessionProvider = {
  getSnapshot: () => ({
    status: "authenticated" as const,
    bearerToken: "valid-token",
    tenantId: "tenant-a",
  }),
  subscribe: () => () => {},
};

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
    expect(listener).toHaveBeenCalledTimes(1); // unsubscribe works
  });

  it("handles session expiration validation correctly", () => {
    const activeSession = {
      status: "authenticated" as const,
      bearerToken: "tok",
      tenantId: "ten",
      expiresAtUnixMillis: Date.now() + 10000,
    };
    expect(requireAuthenticatedSession(activeSession)).toBe(activeSession);

    const expiredSession = {
      status: "authenticated" as const,
      bearerToken: "tok",
      tenantId: "ten",
      expiresAtUnixMillis: Date.now() - 1000,
    };
    expect(() => requireAuthenticatedSession(expiredSession)).toThrow(
      SessionUnavailableError
    );

    const unauthSession = { status: "unauthenticated" as const };
    expect(() => requireAuthenticatedSession(unauthSession)).toThrow(
      SessionUnavailableError
    );
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

  it("maps ConnectError correctly based on code", () => {
    const unauthConnectErr = new ConnectError("Access token invalid", Code.Unauthenticated);
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

describe("GovernedClient Governed Search API", () => {
  const responseHash = CONTRACT_HASHES["crm.search.v1.SearchResponse"] ?? new Uint8Array();

  function createMockOutput(overrides: any = {}) {
    return {
      ownerModuleId: "crm.search",
      schemaId: "crm.search.v1.SearchResponse",
      schemaVersion: "1.0.0",
      dataClass: "confidential",
      descriptorHash: responseHash,
      encoding: "protobuf",
      maximumSizeBytes: 10485760n,
      retentionPolicyId: "standard",
      payload: new Uint8Array(),
      ...overrides,
    };
  }

  it("emits the exact governed coordinates and parses valid search response", async () => {
    let capturedQueryRequest: any = null;

    const mockTransport = createRouterTransport(({ service }) => {
      service(ApplicationGatewayService, {
        async query(req) {
          capturedQueryRequest = req;
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

          return {
            output: createMockOutput({ payload: payloadBytes }),
          } as any;
        },
        async mutate() {
          return {} as any;
        }
      });
    });

    const client = new GovernedClient({
      baseUrl: "http://mock",
      sessionProvider: testSessionProvider,
    });

    // Inject mock transport
    (client as any).gatewayClient = createClient(ApplicationGatewayService, mockTransport);

    const result = await client.searchGlobal({
      text: "query text",
      resourceTypes: ["sales.deal"],
      pageSize: 10,
      cursor: "",
    });

    expect(result.hits).toHaveLength(1);
    expect(result.hits[0]?.resourceId).toBe("deal-1");
    expect(result.nextCursor).toBe("next-page");

    // Verify governed coordinates
    expect(capturedQueryRequest.ownerModuleId).toBe("crm.search");
    expect(capturedQueryRequest.capabilityId).toBe("search.global.query");
    expect(capturedQueryRequest.capabilityVersion).toBe("1.0.0");
  });

  it("safely handles output contract mismatch (drift detection)", async () => {
    const mockTransport = createRouterTransport(({ service }) => {
      service(ApplicationGatewayService, {
        async query() {
          return {
            output: createMockOutput({ descriptorHash: new Uint8Array(32) }),
          } as any;
        },
        async mutate() {
          return {} as any;
        }
      });
    });

    const client = new GovernedClient({
      baseUrl: "http://mock",
      sessionProvider: testSessionProvider,
    });

    (client as any).gatewayClient = createClient(ApplicationGatewayService, mockTransport);

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
    const mockTransport = createRouterTransport(({ service }) => {
      service(ApplicationGatewayService, {
        async query() {
          return {
            output: createMockOutput({ ownerModuleId: "crm.wrong" }),
          } as any;
        },
        async mutate() { return {} as any; }
      });
    });
    const client = new GovernedClient({ baseUrl: "http://mock", sessionProvider: testSessionProvider });
    (client as any).gatewayClient = createClient(ApplicationGatewayService, mockTransport);

    await expect(
      client.searchGlobal({ text: "", resourceTypes: [], pageSize: 10, cursor: "" })
    ).rejects.toThrow('expected ownerModuleId "crm.search", got "crm.wrong"');
  });

  it("rejects response with wrong schemaId", async () => {
    const mockTransport = createRouterTransport(({ service }) => {
      service(ApplicationGatewayService, {
        async query() {
          return {
            output: createMockOutput({ schemaId: "crm.search.v1.WrongResponse" }),
          } as any;
        },
        async mutate() { return {} as any; }
      });
    });
    const client = new GovernedClient({ baseUrl: "http://mock", sessionProvider: testSessionProvider });
    (client as any).gatewayClient = createClient(ApplicationGatewayService, mockTransport);

    await expect(
      client.searchGlobal({ text: "", resourceTypes: [], pageSize: 10, cursor: "" })
    ).rejects.toThrow('expected schemaId "crm.search.v1.SearchResponse", got "crm.search.v1.WrongResponse"');
  });

  it("rejects response with wrong schemaVersion", async () => {
    const mockTransport = createRouterTransport(({ service }) => {
      service(ApplicationGatewayService, {
        async query() {
          return {
            output: createMockOutput({ schemaVersion: "2.0.0" }),
          } as any;
        },
        async mutate() { return {} as any; }
      });
    });
    const client = new GovernedClient({ baseUrl: "http://mock", sessionProvider: testSessionProvider });
    (client as any).gatewayClient = createClient(ApplicationGatewayService, mockTransport);

    await expect(
      client.searchGlobal({ text: "", resourceTypes: [], pageSize: 10, cursor: "" })
    ).rejects.toThrow('expected schemaVersion "1.0.0", got "2.0.0"');
  });

  it("rejects response with wrong dataClass", async () => {
    const mockTransport = createRouterTransport(({ service }) => {
      service(ApplicationGatewayService, {
        async query() {
          return {
            output: createMockOutput({ dataClass: "public" }),
          } as any;
        },
        async mutate() { return {} as any; }
      });
    });
    const client = new GovernedClient({ baseUrl: "http://mock", sessionProvider: testSessionProvider });
    (client as any).gatewayClient = createClient(ApplicationGatewayService, mockTransport);

    await expect(
      client.searchGlobal({ text: "", resourceTypes: [], pageSize: 10, cursor: "" })
    ).rejects.toThrow('expected dataClass "confidential", got "public"');
  });

  it("rejects response with wrong encoding", async () => {
    const mockTransport = createRouterTransport(({ service }) => {
      service(ApplicationGatewayService, {
        async query() {
          return {
            output: createMockOutput({ encoding: "json" }),
          } as any;
        },
        async mutate() { return {} as any; }
      });
    });
    const client = new GovernedClient({ baseUrl: "http://mock", sessionProvider: testSessionProvider });
    (client as any).gatewayClient = createClient(ApplicationGatewayService, mockTransport);

    await expect(
      client.searchGlobal({ text: "", resourceTypes: [], pageSize: 10, cursor: "" })
    ).rejects.toThrow('expected encoding "protobuf", got "json"');
  });

  it("rejects response with missing/invalid contract identity", async () => {
    const mockTransport = createRouterTransport(({ service }) => {
      service(ApplicationGatewayService, {
        async query() {
          return {
            output: createMockOutput({ schemaId: "" }),
          } as any;
        },
        async mutate() { return {} as any; }
      });
    });
    const client = new GovernedClient({ baseUrl: "http://mock", sessionProvider: testSessionProvider });
    (client as any).gatewayClient = createClient(ApplicationGatewayService, mockTransport);

    await expect(
      client.searchGlobal({ text: "", resourceTypes: [], pageSize: 10, cursor: "" })
    ).rejects.toThrow("missing or invalid contract identity fields");
  });

  it("rejects response with non-positive maximumSizeBytes", async () => {
    const mockTransport = createRouterTransport(({ service }) => {
      service(ApplicationGatewayService, {
        async query() {
          return {
            output: createMockOutput({ maximumSizeBytes: 0n }),
          } as any;
        },
        async mutate() { return {} as any; }
      });
    });
    const client = new GovernedClient({ baseUrl: "http://mock", sessionProvider: testSessionProvider });
    (client as any).gatewayClient = createClient(ApplicationGatewayService, mockTransport);

    await expect(
      client.searchGlobal({ text: "", resourceTypes: [], pageSize: 10, cursor: "" })
    ).rejects.toThrow("expected maximumSizeBytes to be positive, got 0");
  });

  it("rejects response with oversized payload", async () => {
    const mockTransport = createRouterTransport(({ service }) => {
      service(ApplicationGatewayService, {
        async query() {
          return {
            output: createMockOutput({
              maximumSizeBytes: 2n,
              payload: new Uint8Array([1, 2, 3, 4]),
            }),
          } as any;
        },
        async mutate() { return {} as any; }
      });
    });
    const client = new GovernedClient({ baseUrl: "http://mock", sessionProvider: testSessionProvider });
    (client as any).gatewayClient = createClient(ApplicationGatewayService, mockTransport);

    await expect(
      client.searchGlobal({ text: "", resourceTypes: [], pageSize: 10, cursor: "" })
    ).rejects.toThrow("payload size 4 exceeds maximumSizeBytes 2");
  });

  it("rejects response with wrong retentionPolicyId", async () => {
    const mockTransport = createRouterTransport(({ service }) => {
      service(ApplicationGatewayService, {
        async query() {
          return {
            output: createMockOutput({ retentionPolicyId: "short" }),
          } as any;
        },
        async mutate() { return {} as any; }
      });
    });
    const client = new GovernedClient({ baseUrl: "http://mock", sessionProvider: testSessionProvider });
    (client as any).gatewayClient = createClient(ApplicationGatewayService, mockTransport);

    await expect(
      client.searchGlobal({ text: "", resourceTypes: [], pageSize: 10, cursor: "" })
    ).rejects.toThrow('expected retentionPolicyId "standard", got "short"');
  });

  it("rejects response with malformed payload (protobuf decoding failure)", async () => {
    const mockTransport = createRouterTransport(({ service }) => {
      service(ApplicationGatewayService, {
        async query() {
          return {
            output: createMockOutput({ payload: new Uint8Array([255, 255, 255, 255]) }),
          } as any;
        },
        async mutate() { return {} as any; }
      });
    });
    const client = new GovernedClient({ baseUrl: "http://mock", sessionProvider: testSessionProvider });
    (client as any).gatewayClient = createClient(ApplicationGatewayService, mockTransport);

    await expect(
      client.searchGlobal({ text: "", resourceTypes: [], pageSize: 10, cursor: "" })
    ).rejects.toThrow("malformed payload - could not decode SearchResponse");
  });
});
