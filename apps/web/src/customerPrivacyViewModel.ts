import type { PrivacyCase } from "@ultimate-crm/client";

export interface CustomerPrivacyCaseViewModel {
  id: string;
  kind: string;
  status: string;
  version: string;
  policyVersion: string;
  createdAt: string;
  updatedAt: string;
}

const CASE_KIND_LABELS: Readonly<Record<number, string>> = {
  1: "Access",
  2: "Portability export",
  3: "Restrict processing",
  4: "Erasure",
};

const CASE_STATUS_LABELS: Readonly<Record<number, string>> = {
  1: "Draft",
  2: "Submitted",
  3: "Subject verified",
  4: "Scoping",
  5: "Scoped",
  6: "Planned",
  7: "Awaiting approval",
  8: "Executing",
  9: "Converging",
  10: "Rescope required",
  11: "Retryable failure",
  12: "Completed",
  13: "Partially completed",
  14: "Denied",
  15: "Cancelled",
  16: "Terminal failure",
};

export function customerPrivacyCaseViewModel(
  privacyCase: PrivacyCase,
): CustomerPrivacyCaseViewModel {
  return {
    id: privacyCase.privacyCaseRef?.privacyCaseId || "Unavailable",
    kind: CASE_KIND_LABELS[privacyCase.kind] ?? "Unspecified",
    status: CASE_STATUS_LABELS[privacyCase.status] ?? "Unspecified",
    version: privacyCase.version.toString(),
    policyVersion: privacyCase.policyVersion || "Not recorded",
    createdAt: formatUnixMillis(privacyCase.createdAtUnixMs),
    updatedAt: formatUnixMillis(privacyCase.updatedAtUnixMs),
  };
}

function formatUnixMillis(value: bigint): string {
  const milliseconds = Number(value);
  if (!Number.isSafeInteger(milliseconds) || milliseconds <= 0) {
    return "Not recorded";
  }
  const date = new Date(milliseconds);
  if (Number.isNaN(date.getTime())) {
    return "Not recorded";
  }
  return date.toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });
}
