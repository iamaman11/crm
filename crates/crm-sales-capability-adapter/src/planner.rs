#![forbid(unsafe_code)]

use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest, CapabilityRisk};
use crm_core_contracts::{BasisPoints, Patch};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, RecordId,
    RecordSnapshot, ResourceRef, SdkError, TenantId,
};
use crm_proto_contracts::crm::{core::v1 as core, sales::v1 as wire};
use crm_sales::{
    AdvanceDealStage, CreateDeal, DEAL_STATE_MAXIMUM_BYTES, DEAL_STATE_RETENTION_POLICY_ID,
    DEAL_STATE_SCHEMA_ID, DEAL_STATE_SCHEMA_VERSION, Deal, DealCloseOutcome, DealOwner, DealStage,
    DealStatus, PipelineId, ReasonCode, StageId, StageTransitionPolicy, TeamId, UpdateDeal,
    deal_state_descriptor_hash, decode_deal_state, encode_deal_state,
};

pub const MODULE_ID: &str = "crm.sales";
pub const RECORD_TYPE: &str = "sales.deal";

pub const CREATE_CAPABILITY: &str = "sales.deal.create";
pub const UPDATE_CAPABILITY: &str = "sales.deal.update";
pub const ADVANCE_CAPABILITY: &str = "sales.deal.advance_stage";

const CREATE_REQUEST_SCHEMA: &str = "crm.sales.v1.CreateDealRequest";
const CREATE_RESPONSE_SCHEMA: &str = "crm.sales.v1.CreateDealResponse";
const UPDATE_REQUEST_SCHEMA: &str = "crm.sales.v1.UpdateDealRequest";
const UPDATE_RESPONSE_SCHEMA: &str = "crm.sales.v1.UpdateDealResponse";
const ADVANCE_REQUEST_SCHEMA: &str = "crm.sales.v1.AdvanceStageRequest";
const ADVANCE_RESPONSE_SCHEMA: &str = "crm.sales.v1.AdvanceStageResponse";
const CREATED_EVENT_SCHEMA: &str = "crm.sales.v1.DealCreatedEvent";
const UPDATED_EVENT_SCHEMA: &str = "crm.sales.v1.DealUpdatedEvent";
const STAGE_CHANGED_EVENT_SCHEMA: &str = "crm.sales.v1.DealStageChangedEvent";

#[derive(Debug, Default, Clone, Copy)]
pub struct SalesDealCapabilityPlanner;

pub fn capability_definition(capability_id: &str) -> Result<CapabilityDefinition, SdkError> {
    let (input_schema, output_schema) = match capability_id {
        CREATE_CAPABILITY => (CREATE_REQUEST_SCHEMA, CREATE_RESPONSE_SCHEMA),
        UPDATE_CAPABILITY => (UPDATE_REQUEST_SCHEMA, UPDATE_RESPONSE_SCHEMA),
        ADVANCE_CAPABILITY => (ADVANCE_REQUEST_SCHEMA, ADVANCE_RESPONSE_SCHEMA),
        _ => return Err(unsupported_capability()),
    };

    Ok(CapabilityDefinition {
        capability_id: support::configured_identifier(CapabilityId::try_new(capability_id))?,
        capability_version: support::configured_identifier(CapabilityVersion::try_new(
            support::CONTRACT_VERSION,
        ))?,
        owner_module_id: support::configured_identifier(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            input_schema,
            vec![DataClass::Confidential],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            output_schema,
            vec![DataClass::Confidential],
        )?),
        risk: CapabilityRisk::Medium,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: capability_id.to_owned(),
        rate_limit_policy_id: None,
    })
}

impl TransactionalAggregatePlanner for SalesDealCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        let (deal_id, presence) = match definition.capability_id.as_str() {
            CREATE_CAPABILITY => {
                let command: wire::CreateDealRequest =
                    support::decode_request(request, MODULE_ID, CREATE_REQUEST_SCHEMA)?;
                (command.deal_id, AggregatePresence::MustBeAbsent)
            }
            UPDATE_CAPABILITY => {
                let command: wire::UpdateDealRequest =
                    support::decode_request(request, MODULE_ID, UPDATE_REQUEST_SCHEMA)?;
                (command.deal_id, AggregatePresence::MustExist)
            }
            ADVANCE_CAPABILITY => {
                let command: wire::AdvanceStageRequest =
                    support::decode_request(request, MODULE_ID, ADVANCE_REQUEST_SCHEMA)?;
                (command.deal_id, AggregatePresence::MustExist)
            }
            _ => return Err(unsupported_capability()),
        };

        Ok(AggregateTarget {
            reference: support::record_ref(RECORD_TYPE, &deal_id, "deal.deal_id")?,
            presence,
        })
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        ensure_definition(definition, request)?;
        match definition.capability_id.as_str() {
            CREATE_CAPABILITY => plan_create(definition, request, current),
            UPDATE_CAPABILITY => plan_update(definition, request, current),
            ADVANCE_CAPABILITY => plan_advance(definition, request, current),
            _ => Err(unsupported_capability()),
        }
    }
}

fn plan_create(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    if current.is_some() {
        return Err(invalid_plan());
    }
    let command: wire::CreateDealRequest =
        support::decode_request(request, MODULE_ID, CREATE_REQUEST_SCHEMA)?;
    let tenant = &request.context.execution.tenant_id;
    let deal = Deal::create(CreateDeal {
        deal_id: support::input_identifier(RecordId::try_new(command.deal_id), "deal.deal_id")?,
        name: command.name,
        owner: owner_from_wire(required(command.owner, "deal.owner")?, "deal.owner")?,
        account: optional_resource(command.account, tenant, "deal.account")?,
        primary_contact: optional_resource(
            command.primary_contact,
            tenant,
            "deal.primary_contact",
        )?,
        stage: stage_from_wire(required(command.stage, "deal.stage")?, "deal.stage")?,
        amount: command
            .amount
            .map(|value| support::wire_money_to_domain(value, "deal.amount"))
            .transpose()?,
        expected_close_date: command
            .expected_close_date
            .map(|value| support::wire_date_to_domain(value, "deal.expected_close_date"))
            .transpose()?,
        probability: probability(
            command.probability_basis_points,
            "deal.probability_basis_points",
        )?,
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })?;

    let aggregate = support::record_ref(RECORD_TYPE, deal.deal_id().as_str(), "deal.deal_id")?;
    let public_deal = deal_to_wire(&deal, tenant);
    let output = support::protobuf_payload(
        MODULE_ID,
        CREATE_RESPONSE_SCHEMA,
        DataClass::Confidential,
        &wire::CreateDealResponse {
            deal: Some(public_deal.clone()),
        },
    )?;
    let event = support::event_evidence(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: "sales.deal.created",
            event_schema_id: CREATED_EVENT_SCHEMA,
            aggregate_version: deal.version(),
            previous_version: None,
        },
        &wire::DealCreatedEvent {
            deal: Some(public_deal),
        },
    )?;

    mutation_plan(
        definition,
        request,
        aggregate.clone(),
        RecordMutation::Create {
            reference: aggregate,
            payload: persisted_payload(&deal)?,
        },
        event,
        output,
    )
}

fn plan_update(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let current = current.ok_or_else(invalid_plan)?;
    let command: wire::UpdateDealRequest =
        support::decode_request(request, MODULE_ID, UPDATE_REQUEST_SCHEMA)?;
    ensure_target(&command.deal_id, current)?;
    let tenant = &request.context.execution.tenant_id;
    let mut deal = deal_from_snapshot(current)?;
    let before = deal.clone();
    deal.apply_update(UpdateDeal {
        expected_version: command.expected_version,
        name: string_patch(command.name, "deal.name")?,
        owner: owner_patch(command.owner, "deal.owner")?,
        account: resource_patch(command.account, tenant, "deal.account")?,
        primary_contact: resource_patch(command.primary_contact, tenant, "deal.primary_contact")?,
        amount: money_patch(command.amount, "deal.amount")?,
        expected_close_date: date_patch(command.expected_close_date, "deal.expected_close_date")?,
        probability: probability_patch(
            command.probability_basis_points,
            "deal.probability_basis_points",
        )?,
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })?;

    let aggregate = current.reference.clone();
    let public_deal = deal_to_wire(&deal, tenant);
    let output = support::protobuf_payload(
        MODULE_ID,
        UPDATE_RESPONSE_SCHEMA,
        DataClass::Confidential,
        &wire::UpdateDealResponse {
            deal: Some(public_deal.clone()),
        },
    )?;
    let event = support::event_evidence(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: "sales.deal.updated",
            event_schema_id: UPDATED_EVENT_SCHEMA,
            aggregate_version: deal.version(),
            previous_version: Some(current.version),
        },
        &wire::DealUpdatedEvent {
            deal: Some(public_deal),
            changed_fields: changed_fields(&before, &deal),
        },
    )?;

    mutation_plan(
        definition,
        request,
        aggregate.clone(),
        RecordMutation::Update {
            reference: aggregate,
            expected_version: current.version,
            payload: persisted_payload(&deal)?,
        },
        event,
        output,
    )
}

fn plan_advance(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let current = current.ok_or_else(invalid_plan)?;
    let command: wire::AdvanceStageRequest =
        support::decode_request(request, MODULE_ID, ADVANCE_REQUEST_SCHEMA)?;
    ensure_target(&command.deal_id, current)?;
    let tenant = &request.context.execution.tenant_id;
    let mut deal = deal_from_snapshot(current)?;
    let previous_stage = deal.stage().clone();
    deal.advance_stage(AdvanceDealStage {
        expected_version: command.expected_version,
        target_stage: stage_from_wire(
            required(command.target_stage, "deal.target_stage")?,
            "deal.target_stage",
        )?,
        target_status: status_from_wire(command.target_status, "deal.target_status")?,
        close_reason_code: command
            .close_reason_code
            .map(ReasonCode::try_new)
            .transpose()?,
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
        policy: command
            .policy
            .map(|policy| StageTransitionPolicy {
                allow_regression: policy.allow_regression,
                allow_skip: policy.allow_skip,
            })
            .unwrap_or_default(),
    })?;

    let aggregate = current.reference.clone();
    let public_deal = deal_to_wire(&deal, tenant);
    let output = support::protobuf_payload(
        MODULE_ID,
        ADVANCE_RESPONSE_SCHEMA,
        DataClass::Confidential,
        &wire::AdvanceStageResponse {
            deal: Some(public_deal),
        },
    )?;
    let event = support::event_evidence(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: "sales.deal.stage_changed",
            event_schema_id: STAGE_CHANGED_EVENT_SCHEMA,
            aggregate_version: deal.version(),
            previous_version: Some(current.version),
        },
        &wire::DealStageChangedEvent {
            deal_id: deal.deal_id().as_str().to_owned(),
            previous_stage: Some(stage_to_wire(&previous_stage)),
            current_stage: Some(stage_to_wire(deal.stage())),
            status: status_to_wire(deal.status()),
            close_outcome: deal.close_outcome().map(close_outcome_to_wire),
            version: deal.version(),
        },
    )?;

    mutation_plan(
        definition,
        request,
        aggregate.clone(),
        RecordMutation::Update {
            reference: aggregate,
            expected_version: current.version,
            payload: persisted_payload(&deal)?,
        },
        event,
        output,
    )
}

fn mutation_plan(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    aggregate: crm_module_sdk::RecordRef,
    mutation: RecordMutation,
    event: crm_core_data::EventEvidence,
    output: crm_module_sdk::TypedPayload,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let audit = support::audit_intent(
        request,
        &aggregate,
        event.aggregate_version,
        definition.capability_id.as_str(),
        &output.bytes,
    )?;
    Ok(CapabilityBatchExecutionPlan {
        batch: BatchMutationPlan {
            context: request.context.clone(),
            records: vec![mutation],
            relationships: Vec::new(),
            events: vec![event],
            idempotency: support::capability_idempotency(definition, request)?,
            audits: vec![audit],
        },
        output: Some(output),
    })
}

fn persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: DEAL_STATE_SCHEMA_ID,
        schema_version: DEAL_STATE_SCHEMA_VERSION,
        descriptor_hash: deal_state_descriptor_hash(),
        maximum_size_bytes: DEAL_STATE_MAXIMUM_BYTES,
        retention_policy_id: DEAL_STATE_RETENTION_POLICY_ID,
    }
}

fn persisted_payload(deal: &Deal) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload(persisted_contract(), encode_deal_state(deal)?)
}

fn deal_from_snapshot(snapshot: &RecordSnapshot) -> Result<Deal, SdkError> {
    let deal = decode_deal_state(support::persisted_json_bytes(
        snapshot,
        persisted_contract(),
    )?)?;
    if deal.deal_id() != &snapshot.reference.record_id || deal.version() != snapshot.version {
        return Err(support::stored_data_error(
            "SALES_PERSISTED_DEAL_IDENTITY_INVALID",
        ));
    }
    Ok(deal)
}

fn deal_to_wire(deal: &Deal, tenant: &TenantId) -> wire::Deal {
    let legacy_amount = deal
        .amount()
        .and_then(|value| i64::try_from(value.minor_units()).ok());
    let legacy_currency = legacy_amount
        .and_then(|_| deal.amount())
        .map(|value| value.currency().as_str().to_owned())
        .unwrap_or_default();
    let legacy_owner = match deal.owner() {
        DealOwner::Actor(actor) => actor.as_str().to_owned(),
        DealOwner::Team(team) => team.as_str().to_owned(),
    };

    wire::Deal {
        deal_id: deal.deal_id().as_str().to_owned(),
        tenant_id: tenant.as_str().to_owned(),
        name: deal.name().to_owned(),
        stage: deal.stage().stage_id().as_str().to_owned(),
        amount_minor: legacy_amount.unwrap_or_default(),
        currency: legacy_currency,
        owner_id: legacy_owner,
        version: deal.version(),
        stage_details: Some(stage_to_wire(deal.stage())),
        amount: deal.amount().map(support::domain_money_to_wire),
        owner: Some(owner_to_wire(deal.owner())),
        account: deal
            .account()
            .map(|value| support::domain_resource_to_wire(value, tenant)),
        primary_contact: deal
            .primary_contact()
            .map(|value| support::domain_resource_to_wire(value, tenant)),
        expected_close_date: deal.expected_close_date().map(support::domain_date_to_wire),
        probability_basis_points: u32::from(deal.probability().get()),
        status: status_to_wire(deal.status()),
        close_outcome: deal.close_outcome().map(close_outcome_to_wire),
        created_at: Some(support::nanos_to_wire_time(deal.created_at_unix_nanos())),
        updated_at: Some(support::nanos_to_wire_time(deal.updated_at_unix_nanos())),
    }
}

fn owner_from_wire(
    value: core::ActorOrTeamOwner,
    field: &'static str,
) -> Result<DealOwner, SdkError> {
    use core::actor_or_team_owner::Owner;
    match value.owner {
        Some(Owner::ActorId(value)) => Ok(DealOwner::Actor(support::input_identifier(
            ActorId::try_new(value),
            field,
        )?)),
        Some(Owner::TeamId(value)) => Ok(DealOwner::Team(TeamId::try_new(value)?)),
        None => Err(SdkError::invalid_argument(field, "owner is required")),
    }
}

fn owner_to_wire(value: &DealOwner) -> core::ActorOrTeamOwner {
    use core::actor_or_team_owner::Owner;
    core::ActorOrTeamOwner {
        owner: Some(match value {
            DealOwner::Actor(actor) => Owner::ActorId(actor.as_str().to_owned()),
            DealOwner::Team(team) => Owner::TeamId(team.as_str().to_owned()),
        }),
    }
}

fn stage_from_wire(value: wire::DealStage, field: &'static str) -> Result<DealStage, SdkError> {
    DealStage::try_new(
        PipelineId::try_new(value.pipeline_id)?,
        StageId::try_new(value.stage_id)?,
        u16::try_from(value.ordinal)
            .map_err(|_| SdkError::invalid_argument(field, "stage ordinal is out of range"))?,
    )
}

fn stage_to_wire(value: &DealStage) -> wire::DealStage {
    wire::DealStage {
        pipeline_id: value.pipeline_id().as_str().to_owned(),
        stage_id: value.stage_id().as_str().to_owned(),
        ordinal: u32::from(value.ordinal()),
    }
}

fn status_from_wire(value: i32, field: &'static str) -> Result<DealStatus, SdkError> {
    match wire::DealStatus::try_from(value).ok() {
        Some(wire::DealStatus::Open) => Ok(DealStatus::Open),
        Some(wire::DealStatus::Won) => Ok(DealStatus::Won),
        Some(wire::DealStatus::Lost) => Ok(DealStatus::Lost),
        _ => Err(SdkError::invalid_argument(field, "deal status is required")),
    }
}

fn status_to_wire(value: DealStatus) -> i32 {
    match value {
        DealStatus::Open => wire::DealStatus::Open as i32,
        DealStatus::Won => wire::DealStatus::Won as i32,
        DealStatus::Lost => wire::DealStatus::Lost as i32,
    }
}

fn close_outcome_to_wire(value: &DealCloseOutcome) -> wire::DealCloseOutcome {
    wire::DealCloseOutcome {
        status: status_to_wire(value.status()),
        reason_code: value.reason_code().as_str().to_owned(),
        closed_at: Some(support::nanos_to_wire_time(value.closed_at_unix_nanos())),
    }
}

fn optional_resource(
    value: Option<core::ResourceRef>,
    tenant: &TenantId,
    field: &'static str,
) -> Result<Option<ResourceRef>, SdkError> {
    value
        .map(|value| support::wire_resource_to_domain(value, tenant, field))
        .transpose()
}

fn string_patch(
    value: Option<core::StringPatch>,
    field: &'static str,
) -> Result<Patch<String>, SdkError> {
    use core::string_patch::Operation;
    let Some(patch) = value else {
        return Ok(Patch::Keep);
    };
    match patch.operation {
        Some(Operation::Set(value)) => Ok(Patch::Set(value)),
        Some(Operation::Clear(true)) => Ok(Patch::Clear),
        Some(Operation::Clear(false)) => {
            Err(SdkError::invalid_argument(field, "clear must be true"))
        }
        None => Err(SdkError::invalid_argument(
            field,
            "a present patch must select exactly one operation",
        )),
    }
}

fn owner_patch(
    value: Option<core::ActorOrTeamOwnerPatch>,
    field: &'static str,
) -> Result<Patch<DealOwner>, SdkError> {
    use core::actor_or_team_owner_patch::Operation;
    let Some(patch) = value else {
        return Ok(Patch::Keep);
    };
    match patch.operation {
        Some(Operation::Set(value)) => Ok(Patch::Set(owner_from_wire(value, field)?)),
        Some(Operation::Clear(true)) => Ok(Patch::Clear),
        Some(Operation::Clear(false)) => {
            Err(SdkError::invalid_argument(field, "clear must be true"))
        }
        None => Err(SdkError::invalid_argument(
            field,
            "a present patch must select exactly one operation",
        )),
    }
}

fn resource_patch(
    value: Option<core::ResourceRefPatch>,
    tenant: &TenantId,
    field: &'static str,
) -> Result<Patch<ResourceRef>, SdkError> {
    use core::resource_ref_patch::Operation;
    let Some(patch) = value else {
        return Ok(Patch::Keep);
    };
    match patch.operation {
        Some(Operation::Set(value)) => Ok(Patch::Set(support::wire_resource_to_domain(
            value, tenant, field,
        )?)),
        Some(Operation::Clear(true)) => Ok(Patch::Clear),
        Some(Operation::Clear(false)) => {
            Err(SdkError::invalid_argument(field, "clear must be true"))
        }
        None => Err(SdkError::invalid_argument(
            field,
            "a present patch must select exactly one operation",
        )),
    }
}

fn money_patch(
    value: Option<core::ExactMoneyPatch>,
    field: &'static str,
) -> Result<Patch<crm_core_contracts::Money>, SdkError> {
    use core::exact_money_patch::Operation;
    let Some(patch) = value else {
        return Ok(Patch::Keep);
    };
    match patch.operation {
        Some(Operation::Set(value)) => Ok(Patch::Set(support::wire_money_to_domain(value, field)?)),
        Some(Operation::Clear(true)) => Ok(Patch::Clear),
        Some(Operation::Clear(false)) => {
            Err(SdkError::invalid_argument(field, "clear must be true"))
        }
        None => Err(SdkError::invalid_argument(
            field,
            "a present patch must select exactly one operation",
        )),
    }
}

fn date_patch(
    value: Option<core::CalendarDatePatch>,
    field: &'static str,
) -> Result<Patch<crm_core_contracts::CalendarDate>, SdkError> {
    use core::calendar_date_patch::Operation;
    let Some(patch) = value else {
        return Ok(Patch::Keep);
    };
    match patch.operation {
        Some(Operation::Set(value)) => Ok(Patch::Set(support::wire_date_to_domain(value, field)?)),
        Some(Operation::Clear(true)) => Ok(Patch::Clear),
        Some(Operation::Clear(false)) => {
            Err(SdkError::invalid_argument(field, "clear must be true"))
        }
        None => Err(SdkError::invalid_argument(
            field,
            "a present patch must select exactly one operation",
        )),
    }
}

fn probability_patch(
    value: Option<core::UInt32Patch>,
    field: &'static str,
) -> Result<Patch<BasisPoints>, SdkError> {
    use core::u_int32_patch::Operation;
    let Some(patch) = value else {
        return Ok(Patch::Keep);
    };
    match patch.operation {
        Some(Operation::Set(value)) => Ok(Patch::Set(probability(value, field)?)),
        Some(Operation::Clear(true)) => Ok(Patch::Clear),
        Some(Operation::Clear(false)) => {
            Err(SdkError::invalid_argument(field, "clear must be true"))
        }
        None => Err(SdkError::invalid_argument(
            field,
            "a present patch must select exactly one operation",
        )),
    }
}

fn probability(value: u32, field: &'static str) -> Result<BasisPoints, SdkError> {
    BasisPoints::try_new(
        u16::try_from(value)
            .map_err(|_| SdkError::invalid_argument(field, "basis points are out of range"))?,
    )
    .map_err(|error| SdkError::invalid_argument(field, error.to_string()))
}

fn changed_fields(before: &Deal, after: &Deal) -> Vec<String> {
    let mut fields = Vec::new();
    if before.name() != after.name() {
        fields.push("name".to_owned());
    }
    if before.owner() != after.owner() {
        fields.push("owner".to_owned());
    }
    if before.account() != after.account() {
        fields.push("account".to_owned());
    }
    if before.primary_contact() != after.primary_contact() {
        fields.push("primary_contact".to_owned());
    }
    if before.amount() != after.amount() {
        fields.push("amount".to_owned());
    }
    if before.expected_close_date() != after.expected_close_date() {
        fields.push("expected_close_date".to_owned());
    }
    if before.probability() != after.probability() {
        fields.push("probability_basis_points".to_owned());
    }
    fields
}

fn ensure_target(deal_id: &str, current: &RecordSnapshot) -> Result<(), SdkError> {
    if deal_id != current.reference.record_id.as_str() {
        return Err(invalid_plan());
    }
    Ok(())
}

fn required<T>(value: Option<T>, field: &'static str) -> Result<T, SdkError> {
    value.ok_or_else(|| SdkError::invalid_argument(field, "field is required"))
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if definition.owner_module_id.as_str() != MODULE_ID
        || request.context.module_id.as_str() != MODULE_ID
        || definition.capability_version.as_str() != support::CONTRACT_VERSION
        || request.context.execution.capability_id != definition.capability_id
        || request.context.execution.capability_version != definition.capability_version
    {
        return Err(invalid_plan());
    }
    Ok(())
}

fn unsupported_capability() -> SdkError {
    SdkError::new(
        "SALES_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The Sales capability is not configured.",
    )
}

fn invalid_plan() -> SdkError {
    SdkError::new(
        "SALES_MUTATION_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The Sales mutation could not be planned safely.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_module_sdk::{
        BusinessTransactionId, CausationId, CorrelationId, ExecutionContext, IdempotencyKey,
        ModuleExecutionContext, PayloadEncoding, RequestId, RetentionPolicyId, SchemaId,
        SchemaVersion, TraceId, TypedPayload,
    };
    use prost::Message;

    #[test]
    fn create_plan_is_deterministic_and_uses_internal_persisted_schema() {
        let definition = capability_definition(CREATE_CAPABILITY).unwrap();
        let request = create_request("tenant-a");
        let planner = SalesDealCapabilityPlanner;
        let first = planner.plan(&definition, &request, None).unwrap();
        let second = planner.plan(&definition, &request, None).unwrap();
        assert_eq!(first, second);
        let RecordMutation::Create { payload, .. } = &first.batch.records[0] else {
            panic!("expected create mutation");
        };
        assert_eq!(payload.schema_id.as_str(), DEAL_STATE_SCHEMA_ID);
        assert_eq!(payload.encoding, PayloadEncoding::Json);
        let persisted = decode_deal_state(&payload.bytes).unwrap();
        assert_eq!(persisted.version(), 1);
        assert_eq!(
            persisted.amount().unwrap().minor_units(),
            125_000_000_000_000_000_000_i128
        );
        assert_eq!(first.batch.events.len(), 1);
        assert_eq!(first.batch.audits.len(), 1);
    }

    #[test]
    fn cross_tenant_resource_is_rejected() {
        let definition = capability_definition(CREATE_CAPABILITY).unwrap();
        let request = create_request("tenant-b");
        let error = SalesDealCapabilityPlanner
            .plan(&definition, &request, None)
            .unwrap_err();
        assert_eq!(error.category, ErrorCategory::InvalidArgument);
    }

    fn create_request(resource_tenant: &str) -> CapabilityRequest {
        let definition = capability_definition(CREATE_CAPABILITY).unwrap();
        let command = wire::CreateDealRequest {
            deal_id: "deal-1".to_owned(),
            name: "Enterprise renewal".to_owned(),
            owner: Some(core::ActorOrTeamOwner {
                owner: Some(core::actor_or_team_owner::Owner::ActorId(
                    "actor-a".to_owned(),
                )),
            }),
            account: Some(core::ResourceRef {
                tenant_id: resource_tenant.to_owned(),
                resource_type: "customer.account".to_owned(),
                resource_id: "account-1".to_owned(),
                version: Some(1),
            }),
            primary_contact: None,
            stage: Some(wire::DealStage {
                pipeline_id: "pipeline.enterprise".to_owned(),
                stage_id: "qualification".to_owned(),
                ordinal: 1,
            }),
            amount: Some(core::ExactMoney {
                minor_units: "125000000000000000000".to_owned(),
                currency_code: "USD".to_owned(),
            }),
            expected_close_date: Some(core::CalendarDate {
                year: 2027,
                month: 12,
                day: 31,
            }),
            probability_basis_points: 2_500,
        };
        let bytes = command.encode_to_vec();
        CapabilityRequest {
            context: ModuleExecutionContext {
                module_id: ModuleId::try_new(MODULE_ID).unwrap(),
                execution: ExecutionContext {
                    tenant_id: TenantId::try_new("tenant-a").unwrap(),
                    actor_id: ActorId::try_new("actor-a").unwrap(),
                    request_id: RequestId::try_new("request-a").unwrap(),
                    correlation_id: CorrelationId::try_new("correlation-a").unwrap(),
                    causation_id: CausationId::try_new("causation-a").unwrap(),
                    trace_id: TraceId::try_new("trace-a").unwrap(),
                    capability_id: definition.capability_id.clone(),
                    capability_version: definition.capability_version.clone(),
                    idempotency_key: IdempotencyKey::try_new("idem-a").unwrap(),
                    business_transaction_id: BusinessTransactionId::try_new("tx-a").unwrap(),
                    schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                    request_started_at_unix_nanos: 100,
                },
            },
            input: TypedPayload {
                owner: ModuleId::try_new(MODULE_ID).unwrap(),
                schema_id: SchemaId::try_new(CREATE_REQUEST_SCHEMA).unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                descriptor_hash: support::message_descriptor_hash(CREATE_REQUEST_SCHEMA),
                data_class: DataClass::Confidential,
                encoding: PayloadEncoding::Protobuf,
                maximum_size_bytes: support::MAX_PROTOBUF_BYTES,
                retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
                bytes,
            },
            input_hash: [7; 32],
            approval: None,
        }
    }
}
