import type { SessionState } from "@ultimate-crm/client";

export type ProductRouteId =
  | "home"
  | "customers"
  | "deals"
  | "tasks"
  | "search"
  | "customer-privacy"
  | "admin-studio"
  | "record-extension-proof";
export type KnownProductCapability =
  | "search.global.query"
  | "parties.party.list"
  | "parties.party.create"
  | "parties.party.update"
  | "parties.party.get"
  | "contact-points.contact-point.list"
  | "contact-points.contact-point.get"
  | "contact-points.contact-point.create"
  | "contact-points.contact-point.update"
  | "accounts.account.list"
  | "accounts.account.create"
  | "sales.deal.list"
  | "sales.deal.create"
  | "sales.deal.update"
  | "sales.deal.advance_stage"
  | "activities.task.list"
  | "activities.task.create"
  | "activities.task.update"
  | "activities.task.complete"
  | "customer_privacy.case.list"
  | "customer_privacy.case.get"
  | "metadata.activation.get";

export interface ProductRouteDefinition {
  id: ProductRouteId;
  path: `/${string}` | "/";
  label: string;
  authentication: "public" | "required";
  requiredCapability?: KnownProductCapability;
  developmentOnly?: boolean;
}

export interface NavigationAccessSnapshot {
  capabilities: ReadonlySet<KnownProductCapability>;
}

export interface ProductEnvironment {
  development: boolean;
}

export const PRODUCT_ROUTES: readonly ProductRouteDefinition[] = [
  {
    id: "home",
    path: "/",
    label: "Home",
    authentication: "required",
  },
  {
    id: "customers", path: "/customers", label: "Customers", authentication: "required", requiredCapability: "parties.party.list",
  },
  {
    id: "deals", path: "/deals", label: "Deals", authentication: "required", requiredCapability: "sales.deal.list",
  },
  {
    id: "tasks", path: "/tasks", label: "Tasks", authentication: "required", requiredCapability: "activities.task.list",
  },
  {
    id: "search",
    path: "/search",
    label: "Search",
    authentication: "required",
    requiredCapability: "search.global.query",
  },
  {
    id: "customer-privacy",
    path: "/customer/privacy",
    label: "Customer Privacy",
    authentication: "required",
    requiredCapability: "customer_privacy.case.list",
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
    developmentOnly: true,
  },
] as const;

export function routeForPath(pathname: string): ProductRouteDefinition | undefined {
  return PRODUCT_ROUTES.find((route) => route.path === pathname);
}

export function canNavigateToRoute(
  route: ProductRouteDefinition,
  session: SessionState,
  access: NavigationAccessSnapshot,
  environment: ProductEnvironment,
): boolean {
  if (route.developmentOnly === true && !environment.development) {
    return false;
  }
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
