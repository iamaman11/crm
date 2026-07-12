import { MutableSessionStore } from "@ultimate-crm/client";

export function createDevelopmentSessionStore(): MutableSessionStore {
  if (!import.meta.env.DEV) {
    return new MutableSessionStore({ status: "unauthenticated" });
  }

  const bearerToken = import.meta.env.VITE_CRM_DEV_BEARER_TOKEN;
  const tenantId = import.meta.env.VITE_CRM_DEV_TENANT_ID;

  if (!bearerToken || !tenantId) {
    return new MutableSessionStore({ status: "unauthenticated" });
  }

  return new MutableSessionStore({
    status: "authenticated",
    bearerToken,
    tenantId,
    actorLabel: import.meta.env.VITE_CRM_DEV_ACTOR_LABEL ?? "Development actor",
    tenantLabel: import.meta.env.VITE_CRM_DEV_TENANT_LABEL ?? tenantId,
  });
}
