import { createClient, ConnectError, Code } from "@connectrpc/connect";
import { createConnectTransport } from "@connectrpc/connect-web";
import { PartyService } from "../gen/crm/parties/v2/party_pb";
import { PartyService as PartyV3 } from "../gen/crm/parties/v3/party_pb";
import { AccountService } from "../gen/crm/accounts/v2/account_pb";
import { ContactPointService } from "../gen/crm/contact_points/v2/contact_point_pb";
import { PartyRelationshipService } from "../gen/crm/party_relationships/v2/relationship_pb";
import { TaskService } from "../gen/crm/activities/v2/task_pb";
import { DataQualityService } from "../gen/crm/data_quality/v2/quality_pb";
import { IdentityResolutionService } from "../gen/crm/identity_resolution/v2/identity_pb";
import { ConsentAuthorizationService } from "../gen/crm/consents/v2/consent_pb";
import { DealService as DealServiceV2 } from "../gen/crm/sales/v2/deal_pb";

import { PrivacyCaseService } from "../gen/crm/customer_privacy/v2/case_pb";
import { EnrichmentConfigurationService } from "../gen/crm/customer_enrichment/v2/config_pb";
import { PartyCsvService } from "../gen/crm/customer_data_operations/v2/party_csv_pb";
import { Customer360Service } from "../gen/crm/customer_360/v2/customer_360_pb";

export type { Party } from "../gen/crm/parties/v3/party_pb";
export type { Account } from "../gen/crm/accounts/v2/account_pb";
export type { ContactPoint } from "../gen/crm/contact_points/v2/contact_point_pb";
export type { PartyRelationship } from "../gen/crm/party_relationships/v2/relationship_pb";
export type { Task } from "../gen/crm/activities/v2/task_pb";
export type { Deal } from "../gen/crm/sales/v2/deal_pb";
export type { AccountOverview } from "../gen/crm/customer_360/v2/customer_360_pb";
export type { PrivacyCase } from "../gen/crm/customer_privacy/v2/case_pb";
export type { ProviderProfileVersion, MappingVersion } from "../gen/crm/customer_enrichment/v2/config_pb";
export type { ConsentAuthorization } from "../gen/crm/consents/v2/consent_pb";
export type { MutationReceipt, TimestampPatch } from "../gen/crm/core/v2/operation_pb";

// One generated binary Connect client for the single Rust Worker.
// Cloudflare Access supplies identity. Browser NEVER supplies tenant/actor.
// The v3/v2 candidate services remain HTTP 410 in contract-only production;
// inclusion here does NOT activate them before the full cutover.
export function createCloudflareCrmClient(baseUrl: string) {
  const transport = createConnectTransport({ baseUrl, useBinaryFormat: true });
  return {
    parties: createClient(PartyService, transport), // existing published read RPC
    partyRecords: createClient(PartyV3, transport),
    accounts: createClient(AccountService, transport),
    contactPoints: createClient(ContactPointService, transport),
    relationships: createClient(PartyRelationshipService, transport),
    tasks: createClient(TaskService, transport),
    dataQuality: createClient(DataQualityService, transport),
    identityResolution: createClient(IdentityResolutionService, transport),
    consents: createClient(ConsentAuthorizationService, transport),
    deals: createClient(DealServiceV2, transport),
    privacy: createClient(PrivacyCaseService, transport),
    enrichment: createClient(EnrichmentConfigurationService, transport),
    csv: createClient(PartyCsvService, transport),
    overview: createClient(Customer360Service, transport),
    csvDownloadUrl: new URL("/api/customer-data-operations/parties.csv", baseUrl).href,
    describeError(error: unknown): string {
      if (!(error instanceof ConnectError)) {
        return error instanceof Error ? error.message : "Не удалось выполнить запрос.";
      }
      const explanations: Partial<Record<Code, string>> = {
        [Code.Unauthenticated]: "Сессия завершена. Войдите через Cloudflare Access.",
        [Code.PermissionDenied]: "Недостаточно прав для операции.",
        [Code.NotFound]: "Запись не найдена.",
        [Code.Aborted]: "Конфликт изменения. Обновите запись и проверьте введённые данные.",
        [Code.InvalidArgument]: "Проверьте введённые данные.",
        [Code.FailedPrecondition]: "Операция недоступна в текущем состоянии.",
        [Code.Unavailable]: "Сервис временно недоступен. Повторите тот же запрос.",
      };
      const requestId = error.metadata.get("x-request-id");
      return [explanations[error.code] ?? "Ошибка сервиса.", error.rawMessage,
        requestId ? "Запрос: " + requestId : ""].filter(Boolean).join(" ");
    },
  };
}
