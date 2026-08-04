import { describe, it, expect } from "vitest";
import {
  canNavigateToRoute,
  routeForPath,
  type ProductRouteDefinition,
} from "./routes";
import type { SessionState } from "@ultimate-crm/client";

const authenticatedSession: SessionState = {
  status: "authenticated",
  bearerToken: "token",
  tenantId: "tenant",
};
const developmentEnvironment = { development: true } as const;
const productionEnvironment = { development: false } as const;

describe("Route Eligibility", () => {
  const publicRoute: ProductRouteDefinition = {
    id: "home",
    path: "/",
    label: "Home",
    authentication: "public",
  };
  const authenticatedRoute: ProductRouteDefinition = {
    id: "home",
    path: "/",
    label: "Home",
    authentication: "required",
  };
  const searchRoute: ProductRouteDefinition = {
    id: "search",
    path: "/search",
    label: "Search",
    authentication: "required",
    requiredCapability: "search.global.query",
  };
  const privacyRoute = routeForPath("/customer/privacy")!;

  it("permits public routes to any session", () => {
    expect(
      canNavigateToRoute(
        publicRoute,
        { status: "unauthenticated" },
        { capabilities: new Set() },
        developmentEnvironment,
      ),
    ).toBe(true);
  });

  it("requires authentication for protected routes", () => {
    expect(
      canNavigateToRoute(
        authenticatedRoute,
        { status: "unauthenticated" },
        { capabilities: new Set() },
        developmentEnvironment,
      ),
    ).toBe(false);
    expect(
      canNavigateToRoute(
        authenticatedRoute,
        authenticatedSession,
        { capabilities: new Set() },
        developmentEnvironment,
      ),
    ).toBe(true);
  });

  it("requires the declared route capability", () => {
    expect(
      canNavigateToRoute(
        searchRoute,
        authenticatedSession,
        { capabilities: new Set() },
        developmentEnvironment,
      ),
    ).toBe(false);
    expect(
      canNavigateToRoute(
        searchRoute,
        authenticatedSession,
        { capabilities: new Set(["search.global.query"]) },
        developmentEnvironment,
      ),
    ).toBe(true);
  });

  it("keeps Customer Privacy hidden without list eligibility", () => {
    expect(privacyRoute.id).toBe("customer-privacy");
    expect(privacyRoute.requiredCapability).toBe("customer_privacy.case.list");
    expect(
      canNavigateToRoute(
        privacyRoute,
        authenticatedSession,
        { capabilities: new Set(["customer_privacy.case.get"]) },
        developmentEnvironment,
      ),
    ).toBe(false);
    expect(
      canNavigateToRoute(
        privacyRoute,
        authenticatedSession,
        { capabilities: new Set(["customer_privacy.case.list"]) },
        developmentEnvironment,
      ),
    ).toBe(true);
  });

  it("keeps the record extension proof authenticated and development-only", () => {
    const route = routeForPath("/records/phase7i-demo")!;
    expect(
      canNavigateToRoute(
        route,
        authenticatedSession,
        { capabilities: new Set() },
        developmentEnvironment,
      ),
    ).toBe(true);
    expect(
      canNavigateToRoute(
        route,
        authenticatedSession,
        { capabilities: new Set() },
        productionEnvironment,
      ),
    ).toBe(false);
  });
});
