import { useMemo, useSyncExternalStore, useState } from "react";
import {
  GovernedClient,
  type SearchHit,
  type SessionState,
} from "@ultimate-crm/client";
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
if (import.meta.env.DEV) {
  (window as any).sessionStore = sessionStore;
}
const client = new GovernedClient({
  baseUrl: window.location.origin,
  sessionProvider: sessionStore,
});

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
    return <SearchPage />;
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

function SearchPage() {
  const [queryText, setQueryText] = useState("");
  const [loading, setLoading] = useState(false);
  const [results, setResults] = useState<SearchHit[]>([]);
  const [error, setError] = useState<string | null>(null);

  const handleSearch = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!queryText.trim()) return;

    setLoading(true);
    setError(null);
    try {
      const response = await client.searchGlobal({
        text: queryText,
        resourceTypes: ["sales.deal", "activities.task"],
        pageSize: 25,
        cursor: "",
      });
      setResults(response.hits);
    } catch (err) {
      console.error(err);
      const isProductError = err && typeof err === "object" && "kind" in err;
      if (isProductError) {
        const productErr = err as any;
        if (productErr.kind === "unauthenticated") {
          setError("Your session has expired. Please sign in again.");
        } else if (productErr.kind === "permission_denied") {
          setError("You do not have permission to access the requested resource.");
        } else if (productErr.kind === "not_found") {
          setError("The requested resource could not be found.");
        } else if (productErr.kind === "invalid_argument") {
          setError("The search query contains invalid parameters.");
        } else if (productErr.kind === "conflict") {
          setError("A data conflict occurred. Please reload the page.");
        } else if (productErr.kind === "rate_limited") {
          setError("Too many requests. Please try again later.");
        } else if (productErr.kind === "unavailable") {
          setError("The CRM service is temporarily unavailable. Please try again later.");
        } else if (productErr.kind === "network") {
          setError("Network connection issue. Please check your internet connection.");
        } else {
          setError("An unexpected server error occurred. Please try again later.");
        }
      } else {
        setError("An unexpected error occurred. Please try again later.");
      }
    } finally {
      setLoading(false);
    }
  };

  return (
    <div>
      <PageHeader
        eyebrow="Governed read path"
        title="Global search"
        description="Search CRM records (Deals and Tasks) using the governed search query capability."
      />

      <form onSubmit={handleSearch} className="crm-search-form">
        <input
          type="text"
          value={queryText}
          onChange={(e) => setQueryText(e.target.value)}
          placeholder="Type deal or task name..."
          className="crm-input"
          id="search-input"
          disabled={loading}
        />
        <button
          type="submit"
          className="crm-button crm-button-primary"
          id="search-submit"
          disabled={loading || !queryText.trim()}
        >
          {loading ? "Searching..." : "Search"}
        </button>
      </form>

      {error ? (
        <FeedbackPanel tone="danger" title="Search failed">
          {error}
        </FeedbackPanel>
      ) : null}

      {loading ? (
        <FeedbackPanel tone="neutral" title="Searching records..." busy={true} />
      ) : null}

      {!loading && !error && results.length === 0 && queryText.trim() ? (
        <FeedbackPanel tone="neutral" title="No results found">
          No records matched your search query.
        </FeedbackPanel>
      ) : null}

      <div className="crm-results-list" id="search-results">
        {results.map((hit, index) => (
          <div key={`${hit.resourceId}-${index}`} className="crm-hit-card" data-testid="search-hit">
            <h3 className="crm-hit-card-title">
              {hit.fields.name || hit.resourceId}
            </h3>
            <div className="crm-hit-card-meta">
              <span className="crm-badge">{hit.resourceType}</span>
              <span>ID: {hit.resourceId}</span>
              <span>Module: {hit.ownerModuleId}</span>
            </div>
            {Object.keys(hit.fields).length > 0 ? (
              <div className="crm-hit-card-fields">
                {Object.entries(hit.fields).map(([name, value]) => (
                  <div key={name} className="crm-hit-field">
                    <span className="crm-hit-field-name">{name}:</span>
                    <span className="crm-hit-field-value">{value}</span>
                  </div>
                ))}
              </div>
            ) : null}
          </div>
        ))}
      </div>
    </div>
  );
}

