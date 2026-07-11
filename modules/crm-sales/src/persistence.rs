use crate::domain::{
    Deal, DealCloseOutcomeSnapshot, DealOwner, DealSnapshot, DealStage, DealStatus, PipelineId,
    ReasonCode, StageId, TeamId,
};
use crm_core_contracts::{BasisPoints, CalendarDate, CurrencyCode, Money};
use crm_module_sdk::{ActorId, ErrorCategory, RecordId, ResourceRef, SdkError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const DEAL_STATE_SCHEMA_ID: &str = "crm.sales.deal.state";
pub const DEAL_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const DEAL_STATE_MAXIMUM_BYTES: u64 = 256 * 1024;
pub const DEAL_STATE_RETENTION_POLICY_ID: &str = "crm.sales.business_record";
const DEAL_STATE_DESCRIPTOR: &[u8] = b"crm.sales.deal.state/v1:deal_id,name,owner,account,primary_contact,stage,status,amount,expected_close_date,probability_basis_points,close_outcome,created_at_unix_nanos,updated_at_unix_nanos,version";

pub fn deal_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(DEAL_STATE_DESCRIPTOR).into()
}

pub fn encode_deal_state(deal: &Deal) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(&DealStateV1::from(deal.snapshot()))
        .map_err(|error| persisted_error(format!("deal state serialization failed: {error}")))?;
    validate_size(&bytes)?;
    Ok(bytes)
}

pub fn decode_deal_state(bytes: &[u8]) -> Result<Deal, SdkError> {
    validate_size(bytes)?;
    let state: DealStateV1 = serde_json::from_slice(bytes)
        .map_err(|error| persisted_error(format!("deal state JSON is invalid: {error}")))?;
    let snapshot = state.try_into()?;
    Deal::rehydrate(snapshot)
        .map_err(|error| persisted_error(format!("{}: {}", error.code, error.safe_message)))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DealStateV1 {
    deal_id: String,
    name: String,
    owner: OwnerState,
    account: Option<ResourceState>,
    primary_contact: Option<ResourceState>,
    stage: StageState,
    status: StatusState,
    amount: Option<MoneyState>,
    expected_close_date: Option<DateState>,
    probability_basis_points: u16,
    close_outcome: Option<CloseOutcomeState>,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum OwnerState {
    Actor { actor_id: String },
    Team { team_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceState {
    resource_type: String,
    resource_id: String,
    version: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StageState {
    pipeline_id: String,
    stage_id: String,
    ordinal: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StatusState {
    Open,
    Won,
    Lost,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MoneyState {
    minor_units: String,
    currency: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DateState {
    year: i32,
    month: u8,
    day: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CloseOutcomeState {
    status: StatusState,
    reason_code: String,
    closed_at_unix_nanos: i64,
}

impl From<DealSnapshot> for DealStateV1 {
    fn from(value: DealSnapshot) -> Self {
        Self {
            deal_id: value.deal_id.to_string(),
            name: value.name,
            owner: value.owner.into(),
            account: value.account.map(Into::into),
            primary_contact: value.primary_contact.map(Into::into),
            stage: value.stage.into(),
            status: value.status.into(),
            amount: value.amount.map(Into::into),
            expected_close_date: value.expected_close_date.map(Into::into),
            probability_basis_points: value.probability.get(),
            close_outcome: value.close_outcome.map(Into::into),
            created_at_unix_nanos: value.created_at_unix_nanos,
            updated_at_unix_nanos: value.updated_at_unix_nanos,
            version: value.version,
        }
    }
}

impl TryFrom<DealStateV1> for DealSnapshot {
    type Error = SdkError;

    fn try_from(value: DealStateV1) -> Result<Self, Self::Error> {
        Ok(Self {
            deal_id: RecordId::try_new(value.deal_id).map_err(identifier_error)?,
            name: value.name,
            owner: value.owner.try_into()?,
            account: value.account.map(Into::into),
            primary_contact: value.primary_contact.map(Into::into),
            stage: value.stage.try_into()?,
            status: value.status.into(),
            amount: value.amount.map(TryInto::try_into).transpose()?,
            expected_close_date: value
                .expected_close_date
                .map(TryInto::try_into)
                .transpose()?,
            probability: BasisPoints::try_new(value.probability_basis_points)
                .map_err(contract_error)?,
            close_outcome: value.close_outcome.map(TryInto::try_into).transpose()?,
            created_at_unix_nanos: value.created_at_unix_nanos,
            updated_at_unix_nanos: value.updated_at_unix_nanos,
            version: value.version,
        })
    }
}

impl From<DealOwner> for OwnerState {
    fn from(value: DealOwner) -> Self {
        match value {
            DealOwner::Actor(actor_id) => Self::Actor {
                actor_id: actor_id.to_string(),
            },
            DealOwner::Team(team_id) => Self::Team {
                team_id: team_id.as_str().to_owned(),
            },
        }
    }
}

impl TryFrom<OwnerState> for DealOwner {
    type Error = SdkError;

    fn try_from(value: OwnerState) -> Result<Self, Self::Error> {
        match value {
            OwnerState::Actor { actor_id } => Ok(Self::Actor(
                ActorId::try_new(actor_id).map_err(identifier_error)?,
            )),
            OwnerState::Team { team_id } => Ok(Self::Team(TeamId::try_new(team_id)?)),
        }
    }
}

impl From<ResourceRef> for ResourceState {
    fn from(value: ResourceRef) -> Self {
        Self {
            resource_type: value.resource_type,
            resource_id: value.resource_id,
            version: value.version,
        }
    }
}

impl From<ResourceState> for ResourceRef {
    fn from(value: ResourceState) -> Self {
        Self {
            resource_type: value.resource_type,
            resource_id: value.resource_id,
            version: value.version,
        }
    }
}

impl From<DealStage> for StageState {
    fn from(value: DealStage) -> Self {
        Self {
            pipeline_id: value.pipeline_id().as_str().to_owned(),
            stage_id: value.stage_id().as_str().to_owned(),
            ordinal: value.ordinal(),
        }
    }
}

impl TryFrom<StageState> for DealStage {
    type Error = SdkError;

    fn try_from(value: StageState) -> Result<Self, Self::Error> {
        DealStage::try_new(
            PipelineId::try_new(value.pipeline_id)?,
            StageId::try_new(value.stage_id)?,
            value.ordinal,
        )
    }
}

impl From<DealStatus> for StatusState {
    fn from(value: DealStatus) -> Self {
        match value {
            DealStatus::Open => Self::Open,
            DealStatus::Won => Self::Won,
            DealStatus::Lost => Self::Lost,
        }
    }
}

impl From<StatusState> for DealStatus {
    fn from(value: StatusState) -> Self {
        match value {
            StatusState::Open => Self::Open,
            StatusState::Won => Self::Won,
            StatusState::Lost => Self::Lost,
        }
    }
}

impl From<Money> for MoneyState {
    fn from(value: Money) -> Self {
        Self {
            minor_units: value.minor_units().to_string(),
            currency: value.currency().as_str().to_owned(),
        }
    }
}

impl TryFrom<MoneyState> for Money {
    type Error = SdkError;

    fn try_from(value: MoneyState) -> Result<Self, Self::Error> {
        let minor_units = value
            .minor_units
            .parse::<i128>()
            .map_err(|_| persisted_error("money minor units must be a base-10 i128 string"))?;
        Ok(Money::new(
            minor_units,
            CurrencyCode::try_new(value.currency).map_err(contract_error)?,
        ))
    }
}

impl From<CalendarDate> for DateState {
    fn from(value: CalendarDate) -> Self {
        Self {
            year: value.year(),
            month: value.month(),
            day: value.day(),
        }
    }
}

impl TryFrom<DateState> for CalendarDate {
    type Error = SdkError;

    fn try_from(value: DateState) -> Result<Self, Self::Error> {
        CalendarDate::try_new(value.year, value.month, value.day).map_err(contract_error)
    }
}

impl From<DealCloseOutcomeSnapshot> for CloseOutcomeState {
    fn from(value: DealCloseOutcomeSnapshot) -> Self {
        Self {
            status: value.status.into(),
            reason_code: value.reason_code.as_str().to_owned(),
            closed_at_unix_nanos: value.closed_at_unix_nanos,
        }
    }
}

impl TryFrom<CloseOutcomeState> for DealCloseOutcomeSnapshot {
    type Error = SdkError;

    fn try_from(value: CloseOutcomeState) -> Result<Self, Self::Error> {
        Ok(Self {
            status: value.status.into(),
            reason_code: ReasonCode::try_new(value.reason_code)?,
            closed_at_unix_nanos: value.closed_at_unix_nanos,
        })
    }
}

fn identifier_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    persisted_error(error.to_string())
}

fn contract_error(error: crm_core_contracts::ContractError) -> SdkError {
    persisted_error(format!("{}: {}", error.code, error.message))
}

fn validate_size(bytes: &[u8]) -> Result<(), SdkError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > DEAL_STATE_MAXIMUM_BYTES {
        return Err(persisted_error(format!(
            "deal state exceeds the maximum of {} bytes",
            DEAL_STATE_MAXIMUM_BYTES
        )));
    }
    Ok(())
}

fn persisted_error(message: impl Into<String>) -> SdkError {
    SdkError::new(
        "SALES_DEAL_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The persisted deal state is invalid.",
    )
    .with_internal_reference(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AdvanceDealStage, CreateDeal, StageTransitionPolicy};

    fn deal() -> Deal {
        Deal::create(CreateDeal {
            deal_id: RecordId::try_new("deal-persisted-1").unwrap(),
            name: "Enterprise expansion".to_owned(),
            owner: DealOwner::Actor(ActorId::try_new("actor-1").unwrap()),
            account: Some(ResourceRef {
                resource_type: "crm.account".to_owned(),
                resource_id: "account-1".to_owned(),
                version: Some(2),
            }),
            primary_contact: None,
            stage: DealStage::try_new(
                PipelineId::try_new("pipeline.enterprise").unwrap(),
                StageId::try_new("qualification").unwrap(),
                1,
            )
            .unwrap(),
            amount: Some(Money::new(
                9_999_999_999_999_999_999_i128,
                CurrencyCode::try_new("USD").unwrap(),
            )),
            expected_close_date: Some(CalendarDate::try_new(2027, 2, 28).unwrap()),
            probability: BasisPoints::try_new(4_250).unwrap(),
            occurred_at_unix_nanos: 10,
        })
        .unwrap()
    }

    #[test]
    fn round_trip_preserves_exact_state_and_schema_hash() {
        let mut value = deal();
        value
            .advance_stage(AdvanceDealStage {
                expected_version: 1,
                target_stage: DealStage::try_new(
                    PipelineId::try_new("pipeline.enterprise").unwrap(),
                    StageId::try_new("closed_won").unwrap(),
                    4,
                )
                .unwrap(),
                target_status: DealStatus::Won,
                close_reason_code: Some(ReasonCode::try_new("selected").unwrap()),
                occurred_at_unix_nanos: 30,
                policy: StageTransitionPolicy::default(),
            })
            .unwrap();

        let bytes = encode_deal_state(&value).unwrap();
        let decoded = decode_deal_state(&bytes).unwrap();

        assert_eq!(decoded, value);
        assert_eq!(
            decoded.amount().unwrap().minor_units(),
            9_999_999_999_999_999_999_i128
        );
        assert_ne!(deal_state_descriptor_hash(), [0; 32]);
    }

    #[test]
    fn rejects_unknown_fields_and_invalid_closed_state() {
        let mut json: serde_json::Value =
            serde_json::from_slice(&encode_deal_state(&deal()).unwrap()).unwrap();
        json["unknown"] = serde_json::json!(true);
        assert_eq!(
            decode_deal_state(&serde_json::to_vec(&json).unwrap())
                .unwrap_err()
                .code,
            "SALES_DEAL_PERSISTED_STATE_INVALID"
        );

        let mut invalid: serde_json::Value =
            serde_json::from_slice(&encode_deal_state(&deal()).unwrap()).unwrap();
        invalid["status"] = serde_json::json!("won");
        assert_eq!(
            decode_deal_state(&serde_json::to_vec(&invalid).unwrap())
                .unwrap_err()
                .code,
            "SALES_DEAL_PERSISTED_STATE_INVALID"
        );
    }
}
