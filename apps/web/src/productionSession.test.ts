import { describe, expect, it, vi } from "vitest";
import {
  PRODUCTION_LOGIN_PATH,
  createProductionSessionStore,
  fetchProductionSession,
  logoutProductionSession,
} from "./productionSession";

const valid = () => ({
  status: "authenticated",
  tenantId: "tenant-a",
  actorLabel: "Operator",
  actorId: "operator-a",
  tenantLabel: "Team A",
  expiresAtUnixMillis: Date.now() + 30_000,
  csrfToken: "01234567890123456789012345678901",
  capabilities: ["search.global.query", "metadata.activation.get", "parties.party.list", "sales.deal.list"],
});

function fakeResponse(body: unknown, status = 200): typeof fetch {
  return vi.fn(async () => new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  })) as unknown as typeof fetch;
}

describe("production identity adapter", () => {
  it("starts unknown without making up an actor or grants", () => {
    expect(createProductionSessionStore().getSnapshot()).toEqual({ status: "unknown" });
    expect(PRODUCTION_LOGIN_PATH).toBe("/auth/login");
  });

  it("admits only a server-attested cookie session with no browser bearer", async () => {
    const fetcher = fakeResponse(valid());
    const result = await fetchProductionSession(fetcher);
    expect(result.session.status).toBe("authenticated");
    expect(result.session).not.toHaveProperty("bearerToken");
    expect(result.session).toHaveProperty("actorId", "operator-a");
    expect(result.session).toHaveProperty("authentication", "cookie");
    expect([...result.access.capabilities].sort()).toEqual([
      "metadata.activation.get", "parties.party.list", "sales.deal.list", "search.global.query",
    ]);
    expect(fetcher).toHaveBeenCalledWith("/auth/session", expect.objectContaining({
      credentials: "same-origin",
      cache: "no-store",
      redirect: "error",
    }));
  });

  it("never grants capabilities on authentication rejection", async () => {
    const result = await fetchProductionSession(fakeResponse({}, 401));
    expect(result.session).toEqual({ status: "unauthenticated" });
    expect(result.access.capabilities.size).toBe(0);
  });

  it("rejects expired, malformed and untrusted session bodies", async () => {
    for (const body of [
      { ...valid(), expiresAtUnixMillis: Date.now() - 1 },
      { ...valid(), csrfToken: "short" },
      { ...valid(), tenantId: "" },
      { ...valid(), capabilities: "search.global.query" },
      { ...valid(), capabilities: [42] },
      { ...valid(), status: "unauthenticated" },
    ]) {
      await expect(fetchProductionSession(fakeResponse(body))).rejects.toThrow();
    }
  });

  it("ignores unknown capabilities instead of displaying unauthorized routes", async () => {
    const result = await fetchProductionSession(fakeResponse({
      ...valid(), capabilities: ["sales.secret.delete", "customer_privacy.case.list"],
    }));
    expect([...result.access.capabilities]).toEqual(["customer_privacy.case.list"]);
  });

  it("fails closed when the identity endpoint is down", async () => {
    await expect(fetchProductionSession(fakeResponse({}, 503))).rejects.toThrow(
      "Identity service is unavailable",
    );
  });

  it("sends CSRF with same-origin logout and requires confirmation", async () => {
    const result = await fetchProductionSession(fakeResponse(valid()));
    const accepted = vi.fn(async () => new Response(null, { status: 204 })) as unknown as typeof fetch;
    await logoutProductionSession(result.session, accepted);
    expect(accepted).toHaveBeenCalledWith("/auth/logout", expect.objectContaining({
      method: "POST",
      credentials: "same-origin",
      redirect: "error",
      headers: { "x-crm-csrf": valid().csrfToken },
    }));
    await expect(logoutProductionSession(result.session, fakeResponse({}, 503))).rejects.toThrow(
      "Could not confirm session logout",
    );
    await expect(logoutProductionSession(result.session, fakeResponse({}, 401))).rejects.toThrow(
      "Could not confirm session logout",
    );
  });

  it("rejects unauthenticated logout and never fabricates a server session", async () => {
    await expect(logoutProductionSession({ status: "unauthenticated" })).rejects.toThrow(
      "No production session to revoke",
    );
  });
});
