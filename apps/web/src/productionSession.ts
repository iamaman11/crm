import { MutableSessionStore, type SessionState } from "@ultimate-crm/client";
import type { KnownProductCapability, NavigationAccessSnapshot } from "./routes";

const KNOWN_CAPABILITIES: ReadonlySet<string> = new Set([
  "search.global.query",
  "customer_privacy.case.list",
  "customer_privacy.case.get",
  "metadata.activation.get",
]);

export interface ProductionSession {
  session: SessionState;
  access: NavigationAccessSnapshot;
}

const NO_ACCESS: NavigationAccessSnapshot = {
  capabilities: new Set<KnownProductCapability>(),
};

export function createProductionSessionStore(): MutableSessionStore {
  return new MutableSessionStore({ status: "unknown" });
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function safeSession(value: unknown): ProductionSession {
  if (!isRecord(value) || value.status !== "authenticated") {
    throw new Error("Invalid production session response");
  }
  if (
    typeof value.tenantId !== "string" ||
    !value.tenantId ||
    typeof value.actorLabel !== "string" ||
    !value.actorLabel ||
    typeof value.csrfToken !== "string" ||
    value.csrfToken.length < 24 ||
    !Array.isArray(value.capabilities)
  ) {
    throw new Error("Incomplete production session response");
  }
  if (
    typeof value.expiresAtUnixMillis !== "number" ||
    !Number.isSafeInteger(value.expiresAtUnixMillis) ||
    value.expiresAtUnixMillis <= Date.now()
  ) {
    throw new Error("Expired production session response");
  }
  const capabilities = new Set<KnownProductCapability>();
  for (const capability of value.capabilities) {
    if (typeof capability !== "string") {
      throw new Error("Invalid session capability");
    }
    if (KNOWN_CAPABILITIES.has(capability)) {
      capabilities.add(capability as KnownProductCapability);
    }
  }
  const session: SessionState = {
    status: "authenticated",
    authentication: "cookie",
    tenantId: value.tenantId,
    actorLabel: value.actorLabel,
    tenantLabel: typeof value.tenantLabel === "string" ? value.tenantLabel : value.tenantId,
    csrfToken: value.csrfToken,
    expiresAtUnixMillis: value.expiresAtUnixMillis,
  };
  return { session, access: { capabilities } };
}

/**
 * Only the same-origin server can attest to a production session.
 * No browser bearer, dev environment token, localStorage credential or fake principal.
 */
export async function fetchProductionSession(fetcher: typeof fetch = fetch): Promise<ProductionSession> {
  const response = await fetcher("/auth/session", {
    method: "GET",
    credentials: "same-origin",
    cache: "no-store",
    redirect: "error",
    headers: { accept: "application/json" },
  });
  if (response.status === 401 || response.status === 403) {
    return { session: { status: "unauthenticated" }, access: NO_ACCESS };
  }
  if (!response.ok) {
    throw new Error("Identity service is unavailable");
  }
  return safeSession(await response.json());
}

export async function logoutProductionSession(
  session: SessionState,
  fetcher: typeof fetch = fetch,
): Promise<void> {
  if (session.status !== "authenticated" || session.authentication !== "cookie") {
    throw new Error("No production session to revoke");
  }
  const response = await fetcher("/auth/logout", {
    method: "POST",
    credentials: "same-origin",
    cache: "no-store",
    redirect: "error",
    headers: { "x-crm-csrf": session.csrfToken },
  });
  if (!response.ok && response.status !== 401) {
    throw new Error("Could not confirm session logout");
  }
}

export const PRODUCTION_LOGIN_PATH = "/auth/login";
