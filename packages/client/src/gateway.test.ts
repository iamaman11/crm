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
          const responseHash = CONTRACT_HASHES["crm.search.v1.SearchResponse"];

          return {
            output: {
              descriptorHash: responseHash ?? new Uint8Array(),
              encoding: "protobuf",
              payload: payloadBytes,
            }
          } as any;
        },
        async mutate() {
          return {} as any;
        }
      });
    });

    const sessionProvider = {
      getSnapshot: () => ({
        status: "authenticated" as const,
        bearerToken: "valid-token",
        tenantId: "tenant-a",
      }),
      subscribe: () => () => {},
    };

    const client = new GovernedClient({
      baseUrl: "http://mock",
      sessionProvider,
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
    expect(capturedQueryRequest).not.toBeNull();
    expect(capturedQueryRequest.ownerModuleId).toBe("crm.search");
    expect(capturedQueryRequest.capabilityId).toBe("search.global.query");
    expect(capturedQueryRequest.capabilityVersion).toBe("1.0.0");
  });

  it("safely handles output contract mismatch (drift detection)", async () => {
    const mockTransport = createRouterTransport(({ service }) => {
      service(ApplicationGatewayService, {
        async query() {
          return {
            output: {
              descriptorHash: new Uint8Array(32), // Invalid hash
              encoding: "protobuf",
              payload: new Uint8Array(),
            }
          };
        },
        async mutate() {
          return {};
        }
      });
    });

    const sessionProvider = {
      getSnapshot: () => ({
        status: "authenticated" as const,
        bearerToken: "valid-token",
        tenantId: "tenant-a",
      }),
      subscribe: () => () => {},
    };

    const client = new GovernedClient({
      baseUrl: "http://mock",
      sessionProvider,
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
});
