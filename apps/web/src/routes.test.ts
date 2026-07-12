import { describe, it, expect } from "vitest";
import { canNavigateToRoute, type ProductRouteDefinition } from "./routes";
import type { SessionState } from "@ultimate-crm/client";

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

  const capabilityRoute: ProductRouteDefinition = {
    id: "search",
    path: "/search",
    label: "Search",
    authentication: "required",
    requiredCapability: "search.global.query",
  };

  it("permits public routes to any session", () => {
    const unauthSession: SessionState = { status: "unauthenticated" };
    const access = { capabilities: new Set<"search.global.query">() };
    expect(canNavigateToRoute(publicRoute, unauthSession, access)).toBe(true);
  });

  it("denies authenticated required routes to unauthenticated sessions", () => {
    const unauthSession: SessionState = { status: "unauthenticated" };
    const access = { capabilities: new Set<"search.global.query">() };
    expect(canNavigateToRoute(authenticatedRoute, unauthSession, access)).toBe(false);
  });

  it("permits authenticated required routes to authenticated sessions", () => {
    const authSession: SessionState = {
      status: "authenticated",
      bearerToken: "token",
      tenantId: "tenant",
    };
    const access = { capabilities: new Set<"search.global.query">() };
    expect(canNavigateToRoute(authenticatedRoute, authSession, access)).toBe(true);
  });

  it("denies capability-required routes when session lacks the capability", () => {
    const authSession: SessionState = {
      status: "authenticated",
      bearerToken: "token",
      tenantId: "tenant",
    };
    const accessWithoutCap = { capabilities: new Set<"search.global.query">() };
    expect(canNavigateToRoute(capabilityRoute, authSession, accessWithoutCap)).toBe(false);
  });

  it("permits capability-required routes when session has the capability", () => {
    const authSession: SessionState = {
      status: "authenticated",
      bearerToken: "token",
      tenantId: "tenant",
    };
    const accessWithCap = { capabilities: new Set<"search.global.query">(["search.global.query"]) };
    expect(canNavigateToRoute(capabilityRoute, authSession, accessWithCap)).toBe(true);
  });
});
