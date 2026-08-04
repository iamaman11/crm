import { create } from "@bufbuild/protobuf";
import { describe, expect, it } from "vitest";
import { PrivacyCaseSchema } from "@ultimate-crm/client/gen/crm/customer_privacy/v1/types_pb";
import { PrivacyCaseRefSchema } from "@ultimate-crm/client/gen/crm/customer_privacy/v1/reference_pb";
import { ProductClientError } from "@ultimate-crm/client";
import { customerPrivacyMessageForError } from "./CustomerPrivacyPage";
import { customerPrivacyCaseViewModel } from "./customerPrivacyViewModel";

describe("Customer Privacy presentation", () => {
  it("maps bounded case metadata without exposing internal evidence", () => {
    const view = customerPrivacyCaseViewModel(
      create(PrivacyCaseSchema, {
        privacyCaseRef: create(PrivacyCaseRefSchema, { privacyCaseId: "case-a" }),
        kind: 1,
        status: 12,
        version: 3n,
        policyVersion: "policy-v1",
        createdAtUnixMs: 1710000000000n,
        updatedAtUnixMs: 1710001000000n,
      }),
    );

    expect(view).toMatchObject({
      id: "case-a",
      kind: "Access",
      status: "Completed",
      version: "3",
      policyVersion: "policy-v1",
    });
    expect(Object.keys(view)).not.toContain("subjectBinding");
    expect(Object.keys(view)).not.toContain("approval");
  });

  it("conceals permission and not-found distinctions from browser-visible errors", () => {
    const denied = new ProductClientError({
      kind: "permission_denied",
      message: "raw denied detail",
      retryable: false,
    });
    const missing = new ProductClientError({
      kind: "not_found",
      message: "raw missing detail",
      retryable: false,
    });

    expect(customerPrivacyMessageForError(denied)).toBe(
      customerPrivacyMessageForError(missing),
    );
    expect(customerPrivacyMessageForError(denied)).not.toContain("raw");
  });
});
