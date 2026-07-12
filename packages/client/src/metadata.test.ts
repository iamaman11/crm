import { describe, expect, it } from "vitest";
import { ProductClientError } from "./gateway";
import { GovernedMetadataClient } from "./metadata";
import { MutableSessionStore } from "./session";

function createClient(sessionProvider: MutableSessionStore): GovernedMetadataClient {
  return new GovernedMetadataClient({
    baseUrl: "http://127.0.0.1:1",
    sessionProvider,
    idFactory: () => "request-test",
  });
}

describe("GovernedMetadataClient", () => {
  it("fails closed before transport access when no authenticated session exists", async () => {
    const client = createClient(new MutableSessionStore({ status: "unauthenticated" }));

    await expect(client.getActivation()).rejects.toMatchObject({
      name: "ProductClientError",
      kind: "unauthenticated",
      retryable: false,
    } satisfies Partial<ProductClientError>);
  });

  it("treats an expired authenticated session as unavailable before transport access", async () => {
    const client = createClient(
      new MutableSessionStore({
        status: "authenticated",
        bearerToken: "test-token",
        tenantId: "tenant-a",
        expiresAtUnixMillis: Date.now() - 1,
      }),
    );

    await expect(client.getImpact("revision-a")).rejects.toMatchObject({
      name: "ProductClientError",
      kind: "unauthenticated",
      retryable: false,
    } satisfies Partial<ProductClientError>);
  });

  it("rejects blank mutation idempotency keys before payload encoding or transport access", async () => {
    const client = createClient(
      new MutableSessionStore({
        status: "authenticated",
        bearerToken: "test-token",
        tenantId: "tenant-a",
      }),
    );

    await expect(
      client.publishBundle({
        definitions: [],
        idempotencyKey: "   ",
      }),
    ).rejects.toMatchObject({
      name: "ProductClientError",
      kind: "invalid_argument",
      retryable: false,
      safeCode: "IDEMPOTENCY_KEY_REQUIRED",
    } satisfies Partial<ProductClientError>);
  });
});
