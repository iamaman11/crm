use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_core_data::{
    PostgresDataStore, RecordGetQuery, RecordListQuery, RecordQueryContinuation, RecordQuerySort,
};
use crm_data_quality::{
    FINDING_OBSERVATION_RECORD_TYPE, FINDING_RECORD_TYPE,
    PARTY_COMPLETENESS_PROFILE_VERSION_RECORD_TYPE, PARTY_COMPLETENESS_RESULT_RECORD_TYPE,
    PARTY_EVALUATION_JOB_RECORD_TYPE, PARTY_RULE_SET_VERSION_RECORD_TYPE, PartyCompletenessResult,
    PartyEvaluationJob, PartyFinding, PartyFindingObservation, PartyFindingStatus, QualitySeverity,
    decode_finding_observation_state, decode_finding_state, decode_party_completeness_result_state,
};
use crm_data_quality_capability_adapter::{
    MODULE_ID, completeness_profile_rule_set_version_id_from_snapshot,
    party_completeness_profile_from_immutable_snapshot, party_completeness_profile_to_wire,
    party_completeness_result_persisted_contract, party_evaluation_job_from_snapshot,
    party_evaluation_job_to_wire, party_finding_observation_persisted_contract,
    party_finding_persisted_contract, party_rule_set_from_snapshot, party_rule_set_to_wire,
};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, PayloadEncoding,
    PortFuture, RecordId, RecordSnapshot, RecordType, SdkError, TypedPayload,
};
use crm_parties_query_adapter::PartyQueryAdapter;
use crm_proto_contracts::crm::{
    core::v1 as core, customer::v1 as customer, data_quality::v1 as wire,
};
use crm_query_runtime::{
    CursorBinding, CursorCodec, CursorContinuation, PageSizePolicy, QueryExecutionResult,
    QueryExecutor, QueryRequest, QuerySemanticValidator, QueryVisibilityAuthorizer,
    QueryVisibilityDecision, normalized_filter_hash,
};
use prost::Message;
use std::sync::Arc;

pub const GET_PARTY_RULE_SET_CAPABILITY: &str = "data_quality.party.rule_set.get";
pub const GET_PARTY_RULE_SET_REQUEST_SCHEMA: &str =
    "crm.data_quality.v1.GetPartyRuleSetVersionRequest";
pub const GET_PARTY_RULE_SET_RESPONSE_SCHEMA: &str =
    "crm.data_quality.v1.GetPartyRuleSetVersionResponse";

pub const GET_PARTY_COMPLETENESS_PROFILE_CAPABILITY: &str =
    "data_quality.party.completeness_profile.get";
pub const GET_PARTY_COMPLETENESS_PROFILE_REQUEST_SCHEMA: &str =
    "crm.data_quality.v1.GetPartyCompletenessProfileVersionRequest";
pub const GET_PARTY_COMPLETENESS_PROFILE_RESPONSE_SCHEMA: &str =
    "crm.data_quality.v1.GetPartyCompletenessProfileVersionResponse";

pub const GET_PARTY_EVALUATION_JOB_CAPABILITY: &str = "data_quality.party.evaluation.get";
pub const GET_PARTY_EVALUATION_JOB_REQUEST_SCHEMA: &str =
    "crm.data_quality.v1.GetPartyEvaluationJobRequest";
pub const GET_PARTY_EVALUATION_JOB_RESPONSE_SCHEMA: &str =
    "crm.data_quality.v1.GetPartyEvaluationJobResponse";

pub const GET_FINDING_CAPABILITY: &str = "data_quality.finding.get";
pub const GET_FINDING_REQUEST_SCHEMA: &str = "crm.data_quality.v1.GetDataQualityFindingRequest";
pub const GET_FINDING_RESPONSE_SCHEMA: &str = "crm.data_quality.v1.GetDataQualityFindingResponse";

pub const LIST_FINDINGS_BY_PARTY_CAPABILITY: &str = "data_quality.finding.list_by_party";
pub const LIST_FINDINGS_BY_PARTY_REQUEST_SCHEMA: &str =
    "crm.data_quality.v1.ListDataQualityFindingsByPartyRequest";
pub const LIST_FINDINGS_BY_PARTY_RESPONSE_SCHEMA: &str =
    "crm.data_quality.v1.ListDataQualityFindingsByPartyResponse";

pub const LIST_ASSIGNED_FINDINGS_CAPABILITY: &str = "data_quality.finding.list_assigned";
pub const LIST_ASSIGNED_FINDINGS_REQUEST_SCHEMA: &str =
    "crm.data_quality.v1.ListAssignedDataQualityFindingsRequest";
pub const LIST_ASSIGNED_FINDINGS_RESPONSE_SCHEMA: &str =
    "crm.data_quality.v1.ListAssignedDataQualityFindingsResponse";

pub const GET_PARTY_COMPLETENESS_RESULT_CAPABILITY: &str = "data_quality.party.completeness.get";
pub const GET_PARTY_COMPLETENESS_RESULT_REQUEST_SCHEMA: &str =
    "crm.data_quality.v1.GetPartyCompletenessResultRequest";
pub const GET_PARTY_COMPLETENESS_RESULT_RESPONSE_SCHEMA: &str =
    "crm.data_quality.v1.GetPartyCompletenessResultResponse";

pub const QUERY_CAPABILITY_IDS: &[&str] = &[
    GET_PARTY_RULE_SET_CAPABILITY,
    GET_PARTY_COMPLETENESS_PROFILE_CAPABILITY,
    GET_PARTY_EVALUATION_JOB_CAPABILITY,
    GET_FINDING_CAPABILITY,
    LIST_FINDINGS_BY_PARTY_CAPABILITY,
    LIST_ASSIGNED_FINDINGS_CAPABILITY,
    GET_PARTY_COMPLETENESS_RESULT_CAPABILITY,
];

const DEFAULT_PAGE_SIZE: u32 = 50;
const MAXIMUM_PAGE_SIZE: u32 = 200;
const MAXIMUM_VISIBILITY_SCAN_RECORDS: usize = 10_000;

#[derive(Clone)]
pub struct DataQualityQueryAdapter {
    store: PostgresDataStore,
    cursor_codec: CursorCodec,
    visibility: Arc<dyn QueryVisibilityAuthorizer>,
    page_policy: PageSizePolicy,
}

impl std::fmt::Debug for DataQualityQueryAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DataQualityQueryAdapter")
            .field("store", &self.store)
            .field("cursor_codec", &self.cursor_codec)
            .field("visibility", &"dyn QueryVisibilityAuthorizer")
            .field("page_policy", &self.page_policy)
            .finish()
    }
}
