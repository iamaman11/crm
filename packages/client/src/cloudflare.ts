import { createClient, ConnectError, Code } from "@connectrpc/connect";
import { createConnectTransport } from "@connectrpc/connect-web";
import { DomainErrorDetailSchema, MutationCommitOutcome } from "../gen/crm/core/v2/operation_pb";
import { PartyService as PartyV3 } from "../gen/crm/parties/v3/party_pb";
import { AccountService } from "../gen/crm/accounts/v2/account_pb";
import { ContactPointService } from "../gen/crm/contact_points/v2/contact_point_pb";
import { PartyRelationshipService } from "../gen/crm/party_relationships/v2/relationship_pb";
import { TaskService } from "../gen/crm/activities/v2/task_pb";
import { MemberService } from "../gen/crm/access/v2/member_pb";
import { InteractionService } from "../gen/crm/activities/v2/interaction_pb";
import { AutomationService } from "../gen/crm/activities/v2/automation_pb";
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
export type { Member, Team } from "../gen/crm/access/v2/member_pb";
export type { Interaction } from "../gen/crm/activities/v2/interaction_pb";
export type { AutomationRule, AutomationRun } from "../gen/crm/activities/v2/automation_pb";
export { AutomationTrigger, AutomationRunStatus, AutomationRunDisposition } from "../gen/crm/activities/v2/automation_pb";
export type { Deal, DealCapabilities, OpportunityParticipant } from "../gen/crm/sales/v2/deal_pb";
export { OpportunityTeamRole } from "../gen/crm/sales/v2/deal_pb";
export type { SalesQuota, SalesQuotaConfiguration, SalesQuotaScope, SalesQuotaFactCursor, GetSalesQuotaPerformanceResponse } from "../gen/crm/sales/v2/deal_pb";
export type { SalesPerformanceReport, GetSalesPerformanceReportResponse, PreviewSalesPerformanceReportResponse, SalesForecastSource } from "../gen/crm/sales/v2/deal_pb";
export type { ExportSalesAnalyticsRequest, ExportSalesAnalyticsResponse } from "../gen/crm/sales/v2/deal_pb";
export { SalesAnalyticsExportKind } from "../gen/crm/sales/v2/deal_pb";
export type { GetSalesScorecardRequest, GetSalesScorecardResponse, SalesScorecardCell, SalesScorecardBucket, SalesScorecardDetailRequest, SalesScorecardDetail } from "../gen/crm/sales/v2/deal_pb";
export { SalesForecastDisposition, SalesQuotaCadence, SalesPerformanceComparisonStatus, SalesScorecardAxis } from "../gen/crm/sales/v2/deal_pb";
export type { CommercialCloseFact, DealEvent } from "../gen/crm/sales/v2/deal_pb";
export type { SalesTerritory, SalesAccountAssignment } from "../gen/crm/sales/v2/deal_pb";
export { SalesOverlayRole, SalesDispatchMode, SalesCapacityOverflow, SalesRoutingSource } from "../gen/crm/sales/v2/deal_pb";
export type { SalesRoutingRule, SalesRoutingRuleConfiguration, SalesRoutingCriteria, SalesRoutingDecision, PreviewSalesRoutingRequest, PreviewSalesRoutingResponse } from "../gen/crm/sales/v2/deal_pb";
export type { SalesPipeline, SalesDealView } from "../gen/crm/sales/v2/deal_pb";
export type { AccountOverview } from "../gen/crm/customer_360/v2/customer_360_pb";
export type { PrivacyCase } from "../gen/crm/customer_privacy/v2/case_pb";
export type { ProviderProfileVersion, MappingVersion } from "../gen/crm/customer_enrichment/v2/config_pb";
export type { ConsentAuthorization } from "../gen/crm/consents/v2/consent_pb";
export type { ActorOrTeamOwner, MutationReceipt, TimestampPatch, TeamDistributionOptions, TeamDistributionScope } from "../gen/crm/core/v2/operation_pb";

export { AccountStatus } from "../gen/crm/accounts/v2/account_pb";
export { PartyKind } from "../gen/crm/parties/v3/party_pb";
export { ContactPointKind, ContactPointStatus } from "../gen/crm/contact_points/v2/contact_point_pb";
export { RelationshipDirectionality, RelationshipStatus } from "../gen/crm/party_relationships/v2/relationship_pb";
export { TaskPriority, TaskStatus } from "../gen/crm/activities/v2/task_pb";
export { MemberRole } from "../gen/crm/access/v2/member_pb";
export { InteractionKind } from "../gen/crm/activities/v2/interaction_pb";
export { DealStatus, DealEventKind, ProbabilitySource, ForecastCategory } from "../gen/crm/sales/v2/deal_pb";
export { CommunicationChannel, ConsentEffect, ConsentStatus, EvaluationReason } from "../gen/crm/consents/v2/consent_pb";
export { PrivacyCaseKind, PrivacyCaseStatus } from "../gen/crm/customer_privacy/v2/case_pb";
export { RawPayloadPolicy } from "../gen/crm/customer_enrichment/v2/config_pb";

// One generated binary Connect client for the single Rust Worker.
// Cloudflare Access supplies identity. Browser NEVER supplies tenant/actor.
export function createCloudflareCrmClient(baseUrl: string) {
  const transport = createConnectTransport({ baseUrl, useBinaryFormat: true });
  return {
    partyRecords: createClient(PartyV3, transport),
    accounts: createClient(AccountService, transport),
    contactPoints: createClient(ContactPointService, transport),
    relationships: createClient(PartyRelationshipService, transport),
    tasks: createClient(TaskService, transport),
    members: createClient(MemberService, transport),
    interactions: createClient(InteractionService, transport),
    automations: createClient(AutomationService, transport),
    dataQuality: createClient(DataQualityService, transport),
    identityResolution: createClient(IdentityResolutionService, transport),
    consents: createClient(ConsentAuthorizationService, transport),
    deals: createClient(DealServiceV2, transport),
    privacy: createClient(PrivacyCaseService, transport),
    enrichment: createClient(EnrichmentConfigurationService, transport),
    csv: createClient(PartyCsvService, transport),
    overview: createClient(Customer360Service, transport),
    describeError: (error: unknown): string => decodeCrmError(error).text,
    decodeError: decodeCrmError,
  };
}

/** One generated error decoder. Only a positive rejection releases a new intent. */
export type CrmFailure = {
  outcome: "rejected" | "unconfirmed";
  text: string;
  requestId: string | undefined;
  domainCode: string | undefined;
  fieldViolations: readonly { fieldPath: string; description: string }[];
};

export function decodeCrmError(error: unknown): CrmFailure {
  if (!(error instanceof ConnectError)) {
    return {outcome:"unconfirmed",text:"Не удалось выполнить запрос.",
      requestId:undefined,domainCode:undefined,fieldViolations:[]};
  }
  const detail = error.findDetails(DomainErrorDetailSchema)[0];
  const requestId = error.metadata.get("x-request-id") ?? undefined;
  const correlated = !!requestId && detail?.requestId === requestId;
  const outcome = correlated && detail.commitOutcome===MutationCommitOutcome.REJECTED ? "rejected" : "unconfirmed";
  const explanations: Partial<Record<Code, string>> = {
    [Code.Unauthenticated]: "Сессия завершена. Войдите через Cloudflare Access.",
    [Code.PermissionDenied]: "Недостаточно прав для операции.",
    [Code.NotFound]: "Запись не найдена.",
    [Code.Aborted]: "Конфликт изменения. Обновите запись и проверьте данные.",
    [Code.InvalidArgument]: "Проверьте введённые данные.",
    [Code.FailedPrecondition]: "Операция недоступна в текущем состоянии.",
    [Code.Unavailable]: "Сервис временно недоступен. Повторите тот же запрос.",
  };
  // Only locally translated domain and field details reach the UI.
  const domainExplanations: Readonly<Record<string, string>> = {
    SALES_DEAL_NAME_INVALID: "Введите название сделки.",
    SALES_REOPEN_OWNER_NOT_ELIGIBLE: "Ответственный недоступен. Выберите действующего ответственного для повторного открытия сделки.",
    SALES_DEAL_VERSION_CONFLICT: explanations[Code.Aborted]!,
    SALES_PIPELINE_VERSION_CONFLICT: explanations[Code.Aborted]!,
  };
  const fieldNames: Readonly<Record<string, string>> = {
    "deal.name":"Название сделки",
    "amount.currency_code":"Валюта",
  };
  const domainCode = correlated && Object.hasOwn(domainExplanations, detail.domainCode) ? detail.domainCode : undefined;
  const fieldViolations = correlated ? detail.fieldViolations.flatMap(field =>
    Object.hasOwn(fieldNames, field.fieldPath)
      ? [{fieldPath:fieldNames[field.fieldPath]!,description:"Проверьте значение."}] : []) : [];
  const text = [domainCode ? domainExplanations[domainCode] : explanations[error.code] ?? "Ошибка сервиса.",
    requestId && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(requestId)
      ? "Запрос: " + requestId : ""].filter(Boolean).join(" ");
  return {outcome,text,requestId,domainCode,fieldViolations};
}
