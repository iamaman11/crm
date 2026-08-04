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

export { GovernedCustomerPrivacyClient } from "./customerPrivacy";

export type { ListCustomerPrivacyCasesOptions } from "./customerPrivacy";

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
  GetPrivacyCaseResponse,
  ListPrivacyCasesResponse,
} from "../gen/crm/customer_privacy/v1/cases_pb";
export type { PrivacyCase } from "../gen/crm/customer_privacy/v1/types_pb";
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
