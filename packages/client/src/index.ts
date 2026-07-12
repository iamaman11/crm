export {
  GovernedClient,
  ProductClientError,
  mapGatewayError,
} from "./gateway";

export type {
  GovernedGatewayClientOptions,
  SearchGlobalOptions,
  SearchGlobalResult,
  ProductClientErrorKind,
} from "./gateway";

export {
  MutableSessionStore,
  SessionUnavailableError,
  requireAuthenticatedSession,
} from "./session";

export type {
  SessionState,
  SessionProvider,
} from "./session";

export type { SearchHit } from "../gen/crm/search/v1/search_pb";
