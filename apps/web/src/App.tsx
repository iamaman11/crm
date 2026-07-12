import { useMemo, useSyncExternalStore, useState } from "react";
import {
  createGovernedGatewayClient,
  TypedPayloadSchema,
  SearchRequestSchema,
  SearchResponseSchema,
  create,
  toBinary,
  fromBinary,
  type SessionState,
  type SearchHit,
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
const client = createGovernedGatewayClient({
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
      const searchRequest = create(SearchRequestSchema, {
        text: queryText,
        resourceTypes: [],
        pageSize: 25,
        cursor: "",
      });
      const searchRequestBytes = toBinary(SearchRequestSchema, searchRequest);
      const searchRequestDescriptorHash = new Uint8Array([
        0x6e, 0x09, 0x97, 0x8a, 0xe7, 0x42, 0x43, 0x21, 0x2d, 0xf9, 0xf7, 0xb5, 0x8c, 0xb4, 0x01, 0xfd, 0xef, 0x0e, 0x60, 0x98, 0xad, 0xdd, 0x57, 0xb4, 0xae, 0xc7, 0x0c, 0x96, 0x57, 0xd3, 0x42, 0x61
      ]);

      const input = create(TypedPayloadSchema, {
        ownerModuleId: "crm.search",
        schemaId: "crm.search.v1.SearchRequest",
        schemaVersion: "1.0.0",
        descriptorHash: searchRequestDescriptorHash,
        dataClass: "confidential",
        encoding: "protobuf",
        maximumSizeBytes: 1024n,
        retentionPolicyId: "standard",
        payload: searchRequestBytes,
      });

      const response = await client.query({
        ownerModuleId: "crm.search",
        capabilityId: "search.global.query",
        capabilityVersion: "1.0.0",
        input,
      });

      if (!response.output) {
        throw new Error("Missing query output");
      }

      const searchResponse = fromBinary(SearchResponseSchema, response.output.payload);
      setResults(searchResponse.hits);
    } catch (err) {
      console.error(err);
      if (err instanceof Error) {
        setError(err.message);
      } else {
        setError("An unexpected error occurred.");
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

