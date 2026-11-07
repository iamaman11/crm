import { describe, expect, it } from "vitest";
import { GovernedCrmClient, PartyKind } from "./crm";
import { MutableSessionStore } from "./session";

function client(authenticated: boolean): GovernedCrmClient {
  return new GovernedCrmClient({
    baseUrl: "http://127.0.0.1:1",
    sessionProvider: new MutableSessionStore(authenticated
      ? { status: "authenticated", tenantId: "tenant-a", bearerToken: "test" }
      : { status: "unauthenticated" }),
  });
}

describe("governed CRM typed client", () => {
  it("fails closed on list and mutations without an authenticated session", async () => {
    const api = client(false);
    await expect(api.listParties()).rejects.toMatchObject({ kind: "unauthenticated" });
    await expect(api.listDeals()).rejects.toMatchObject({ kind: "unauthenticated" });
    await expect(api.listTasks()).rejects.toMatchObject({ kind: "unauthenticated" });
    await expect(api.createParty("party-a", PartyKind.PERSON, "Alice"))
      .rejects.toMatchObject({ kind: "unauthenticated" });
  });

  it("rejects missing names or opaque references before calling the network", async () => {
    const api = client(true);
    await expect(api.createParty("party-a", PartyKind.PERSON, "  "))
      .rejects.toMatchObject({ kind: "invalid_argument" });
    await expect(api.getParty(" "))
      .rejects.toMatchObject({ kind: "invalid_argument" });
    await expect(api.createParty("x".repeat(129), PartyKind.PERSON, "Alice"))
      .rejects.toMatchObject({ kind: "invalid_argument" });
    await expect(api.createTask("task-a", " ", "actor-a", "tenant-a", "parties.party", "party-a"))
      .rejects.toMatchObject({ kind: "invalid_argument" });
  });
});
