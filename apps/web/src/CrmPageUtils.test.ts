import { describe, expect, it } from "vitest";
import { validContact, crmMessage } from "./CrmPageUtils";
import { ProductClientError } from "@ultimate-crm/client";

describe("CRM form validation and safe feedback", () => {
  it("accepts ordinary email and telephone values and rejects invalid ones", () => {
    expect(validContact("email", "operator@example.org")).toBe(true);
    expect(validContact("email", "not-an-email")).toBe(false);
    expect(validContact("phone", "+48 123 456 789")).toBe(true);
    expect(validContact("phone", "invalid")).toBe(false);
  });
  it("does not expose raw server errors to an operator", () => {
    const raw = new ProductClientError({
      kind: "permission_denied",
      message: "SQL private error secret tenant/actor",
      retryable: false,
    });
    expect(crmMessage(raw)).toBe("Your account does not have permission for this action.");
    expect(crmMessage(raw)).not.toContain("SQL");
  });
});
