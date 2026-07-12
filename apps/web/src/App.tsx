import { useMemo, useSyncExternalStore } from "react";
import type { SessionState } from "@ultimate-crm/client";
import { AppShell, FeedbackPanel, PageHeader } from "@ultimate-crm/ui";
import { createDevelopmentSessionStore } from "./developmentSession";
import {
  canNavigateToRoute,
  PRODUCT_ROUTES,
  routeForPath,
  type KnownProductCapability,
  type NavigationAccessSnapshot,
} from "./routes";

const sessionStore = createDevelopmentSessionStore();

export function App() {
  const session = useSyncExternalStore(
    (listener) => sessionStore.subscribe(listener),
    () => sessionStore.getSnapshot(),
  );
  const access = useMemo(() => developmentAccessSnapshot(), []);
  const currentRoute = routeForPath(window.location.pathname);
  const navigation = PRODUCT_ROUTES.filter((route) =>
    canNavigateToRoute(route, session, access),
  ).map((route) => ({
    id: route.id,
    href: route.path,
    label: route.label,
    current: currentRoute?.id === route.id,
  }));

  return (
    <AppShell
      productName="Ultimate CRM"
      navigation={navigation}
      accountSlot={<SessionSummary session={session} />}
    >
      <RouteContent session={session} access={access} />
    </AppShell>
  );
}

function RouteContent({
  session,
  access,
}: {
  session: SessionState;
  access: NavigationAccessSnapshot;
}) {
  if (session.status !== "authenticated") {
    return (
      <>
        <PageHeader
          eyebrow="Product shell"
          title="Authentication required"
          description="The browser product plane does not invent an actor or tenant. Configure the explicit development session adapter for local integration; production identity integration remains a replaceable boundary."
        />
        <FeedbackPanel tone="warning" title="No authenticated session">
          Set the development-only bearer token and tenant variables when running locally. The governed backend remains authoritative for authentication and tenant access.
        </FeedbackPanel>
      </>
    );
  }

  const route = routeForPath(window.location.pathname);
  if (!route) {
    return (
      <>
        <PageHeader eyebrow="Navigation" title="Page not found" />
        <FeedbackPanel tone="neutral" title="This route is not registered">
          The product shell exposes only typed registered routes.
        </FeedbackPanel>
      </>
    );
  }

  if (!canNavigateToRoute(route, session, access)) {
    return (
      <>
        <PageHeader eyebrow="Navigation" title="Route unavailable" />
        <FeedbackPanel tone="danger" title="This route is not available">
          Client-side route eligibility is a user-experience hint only. The backend still performs the authoritative live authorization check for every request.
        </FeedbackPanel>
      </>
    );
  }

  if (route.id === "search") {
    return (
      <>
        <PageHeader
          eyebrow="Governed read path"
          title="Global search"
          description="The generated gRPC-Web client boundary is established in this packet. The first live search workflow is completed in the behavior checkpoint after exact-SHA build verification."
        />
        <FeedbackPanel tone="neutral" title="Client boundary ready for integration">
          No feature component is permitted to bypass the generated governed gateway client with an ad-hoc CRM API call.
        </FeedbackPanel>
      </>
    );
  }

  return (
    <>
      <PageHeader
        eyebrow="Phase 7C"
        title="Product shell foundation"
        description="A typed product-plane boundary for future Admin Studio and expert CRM domain waves. Business invariants, authorization and authoritative state remain on the governed backend path."
      />
      <FeedbackPanel tone="success" title="Shell composition is active">
        Session state, permission-aware navigation, design-system primitives and the generated client boundary are now separate product-plane responsibilities.
      </FeedbackPanel>
    </>
  );
}

function SessionSummary({ session }: { session: SessionState }) {
  if (session.status !== "authenticated") {
    return <span>Signed out</span>;
  }
  return <span>{session.actorLabel ?? "Authenticated actor"} · {session.tenantLabel ?? session.tenantId}</span>;
}

function developmentAccessSnapshot(): NavigationAccessSnapshot {
  if (!import.meta.env.DEV) {
    return { capabilities: new Set<KnownProductCapability>() };
  }
  const configured = new Set(
    (import.meta.env.VITE_CRM_DEV_CAPABILITIES ?? "")
      .split(",")
      .map((value) => value.trim())
      .filter(Boolean),
  );
  const capabilities = new Set<KnownProductCapability>();
  if (configured.has("search.global.query")) {
    capabilities.add("search.global.query");
  }
  return { capabilities };
}
