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

export { GovernedMetadataClient } from "./metadata";

export type {
  ActivateMetadataRevisionOptions,
  PublishMetadataBundleOptions,
  RollbackMetadataRevisionOptions,
} from "./metadata";

export {
  createObjectMetadataDefinitionInput,
  MetadataAuthoringError,
  METADATA_DEFINITION_SCHEMA_VERSION,
} from "./metadataAuthoring";

export type {
  MetadataAuthoringField,
  ObjectMetadataDraft,
} from "./metadataAuthoring";

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
export type {
  ActivateMetadataRevisionResponse,
  GetMetadataActivationResponse,
  GetMetadataImpactResponse,
  GetMetadataRevisionResponse,
  MetadataActivationState,
  MetadataChange,
  MetadataDefinitionInput,
  MetadataDocument,
  MetadataImpact,
  MetadataRevision,
  PublishMetadataBundleResponse,
  RollbackMetadataRevisionResponse,
} from "../gen/crm/metadata/v1/metadata_pb";
