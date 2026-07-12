import { describe, it, expect, vi } from "vitest";
import {
  MutableSessionStore,
  requireAuthenticatedSession,
  SessionUnavailableError,
  mapGatewayError,
  ProductClientError,
} from "./index";
import { ConnectError, Code } from "@connectrpc/connect";

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
