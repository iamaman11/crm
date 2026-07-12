import type { SessionState } from "@ultimate-crm/client";

export type ProductRouteId = "home" | "search";
export type KnownProductCapability = "search.global.query";

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
