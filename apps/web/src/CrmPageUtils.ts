import { ProductClientError } from "@ultimate-crm/client";

export function crmMessage(error: unknown): string {
  if (!(error instanceof ProductClientError)) return "The request did not finish. Check the connection and try again.";
  switch (error.kind) {
    case "unauthenticated": return "Session expired. Please sign in again.";
    case "permission_denied": return "Your account does not have permission for this action.";
    case "invalid_argument": return "Check the entered details and try again.";
    case "conflict": return "Someone changed this record. Reload it before saving again.";
    case "not_found": return "This record is no longer available.";
    case "rate_limited": return "Too many requests. Please try again later.";
    case "network":
    case "unavailable": return "The connection was interrupted. Retry the same action to avoid duplicates.";
    default: return "The CRM request failed. Please contact support if it happens again.";
  }
}

export function validContact(kind: "email" | "phone", value: string): boolean {
  const text = value.trim();
  if (kind === "email") return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(text) && text.length <= 200;
  return /^[+0-9() .-]{5,32}$/.test(text);
}
