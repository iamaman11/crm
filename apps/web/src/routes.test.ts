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

  const adminStudioRoute: ProductRouteDefinition = {
    id: "admin-studio",
    path: "/admin/metadata",
    label: "Admin Studio",
    authentication: "required",
    requiredCapability: "metadata.activation.get",
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
    const access = { capabilities: new Set<"search.global.query">() };
    expect(canNavigateToRoute(authenticatedRoute, authenticatedSession, access)).toBe(true);
  });

  it("denies capability-required routes when session lacks the capability", () => {
    const accessWithoutCap = { capabilities: new Set<"search.global.query">() };
    expect(canNavigateToRoute(searchRoute, authenticatedSession, accessWithoutCap)).toBe(false);
  });

  it("permits capability-required routes when session has the capability", () => {
    const accessWithCap = {
      capabilities: new Set<"search.global.query">(["search.global.query"]),
    };
    expect(canNavigateToRoute(searchRoute, authenticatedSession, accessWithCap)).toBe(true);
  });

  it("keeps Admin Studio hidden until its governed metadata capability is available", () => {
    const denied = {
      capabilities: new Set<"search.global.query" | "metadata.activation.get">([
        "search.global.query",
      ]),
    };
    const allowed = {
      capabilities: new Set<"search.global.query" | "metadata.activation.get">([
        "metadata.activation.get",
      ]),
    };

    expect(canNavigateToRoute(adminStudioRoute, authenticatedSession, denied)).toBe(false);
    expect(canNavigateToRoute(adminStudioRoute, authenticatedSession, allowed)).toBe(true);
  });

  it("keeps the record extension proof behind authentication without inventing a backend capability gate", () => {
    const route = routeForPath("/records/phase7i-demo");
    expect(route?.id).toBe("record-extension-proof");

    const unauthenticatedSession: SessionState = { status: "unauthenticated" };
    const access = { capabilities: new Set<"search.global.query">() };
    expect(canNavigateToRoute(route!, unauthenticatedSession, access)).toBe(false);
    expect(canNavigateToRoute(route!, authenticatedSession, access)).toBe(true);
  });
});
