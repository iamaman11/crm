import { describe, expect, it } from "vitest";
import { GovernedCustomerPrivacyClient } from "./customerPrivacy";
import { ProductClientError } from "./gateway";
import { MutableSessionStore } from "./session";

function authenticatedClient(): GovernedCustomerPrivacyClient {
  return new GovernedCustomerPrivacyClient({
    baseUrl: "http://127.0.0.1:1",
    sessionProvider: new MutableSessionStore({
      status: "authenticated",
      bearerToken: "test-token",
      tenantId: "tenant-a",
    }),
    idFactory: () => "request-test",
  });
}

describe("GovernedCustomerPrivacyClient", () => {
  it("fails closed before transport access without an authenticated session", async () => {
    const client = new GovernedCustomerPrivacyClient({
      baseUrl: "http://127.0.0.1:1",
      sessionProvider: new MutableSessionStore({ status: "unauthenticated" }),
      idFactory: () => "request-test",
    });

    await expect(
      client.listCases({ canonicalPartyId: "party-a" }),
    ).rejects.toMatchObject({
      name: "ProductClientError",
      kind: "unauthenticated",
      retryable: false,
    } satisfies Partial<ProductClientError>);
  });

  it("rejects blank or oversized opaque references before transport access", async () => {
    const client = authenticatedClient();

    await expect(
      client.listCases({ canonicalPartyId: "   " }),
    ).rejects.toMatchObject({
      kind: "invalid_argument",
      safeCode: "CUSTOMER_PRIVACY_REFERENCE_INVALID",
    } satisfies Partial<ProductClientError>);
    await expect(client.getCase("x".repeat(129))).rejects.toMatchObject({
      kind: "invalid_argument",
      safeCode: "CUSTOMER_PRIVACY_REFERENCE_INVALID",
    } satisfies Partial<ProductClientError>);
  });

  it("rejects unbounded page sizes before transport access", async () => {
    const client = authenticatedClient();

    await expect(
      client.listCases({ canonicalPartyId: "party-a", pageSize: 101 }),
    ).rejects.toMatchObject({
      kind: "invalid_argument",
      safeCode: "CUSTOMER_PRIVACY_PAGE_SIZE_INVALID",
    } satisfies Partial<ProductClientError>);
  });
});
