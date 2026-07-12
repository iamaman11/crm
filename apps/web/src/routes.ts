import type { SessionState } from "@ultimate-crm/client";

export type ProductRouteId =
  | "home"
  | "search"
  | "admin-studio"
  | "record-extension-proof";
export type KnownProductCapability =
  | "search.global.query"
  | "metadata.activation.get";

export interface ProductRouteDefinition {
  id: ProductRouteId;
  path: `/${string}` | "/";
  label: string;
  authentication: "public" | "required";
  requiredCapability?: KnownProductCapability;
}

export interface NavigationAccessSnapshot {
  capabilities: ReadonlySet<KnownProductCapability>;
}

export const PRODUCT_ROUTES: readonly ProductRouteDefinition[] = [
  {
    id: "home",
    path: "/",
    label: "Home",
    authentication: "required",
  },
  {
    id: "search",
    path: "/search",
    label: "Search",
    authentication: "required",
    requiredCapability: "search.global.query",
  },
  {
    id: "admin-studio",
    path: "/admin/metadata",
    label: "Admin Studio",
    authentication: "required",
    requiredCapability: "metadata.activation.get",
  },
  {
    id: "record-extension-proof",
    path: "/records/phase7i-demo",
    label: "Record page",
    authentication: "required",
  },
] as const;

export function routeForPath(pathname: string): ProductRouteDefinition | undefined {
  return PRODUCT_ROUTES.find((route) => route.path === pathname);
}

export function canNavigateToRoute(
  route: ProductRouteDefinition,
  session: SessionState,
  access: NavigationAccessSnapshot,
): boolean {
  if (route.authentication === "required" && session.status !== "authenticated") {
    return false;
  }
  if (
    route.requiredCapability !== undefined &&
    !access.capabilities.has(route.requiredCapability)
  ) {
    return false;
  }
  return true;
}
