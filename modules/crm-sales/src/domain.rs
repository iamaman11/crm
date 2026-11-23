use crm_core_contracts::{BasisPoints, CalendarDate, Money, PageRequest, Patch};
use crm_module_sdk::{
    ActorId, ErrorCategory, FieldName, FieldViolation, RecordId, ResourceRef, SdkError,
};
use serde::{Deserialize, Serialize};

const MAX_NAME_BYTES: usize = 240;
const MAX_IDENTIFIER_BYTES: usize = 180;
const MAX_REASON_BYTES: usize = 240;

macro_rules! domain_identifier {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
                let value = value.into();
                validate_identifier($field, &value)?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

domain_identifier!(PipelineId, "deal.pipeline_id");
domain_identifier!(StageId, "deal.stage_id");
domain_identifier!(TeamId, "deal.owner.team_id");
domain_identifier!(ReasonCode, "deal.close_reason_code");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DealOwner {
    Actor(ActorId),
    Team(TeamId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DealStage {
    pipeline_id: PipelineId,
    stage_id: StageId,
    ordinal: u16,
}

impl DealStage {
    pub fn pipeline_id(&self) -> &PipelineId {
        &self.pipeline_id
    }

    pub fn stage_id(&self) -> &StageId {
        &self.stage_id
    }

    pub const fn ordinal(&self) -> u16 {
        self.ordinal
    }

    pub fn try_new(
        pipeline_id: PipelineId,
        stage_id: StageId,
        ordinal: u16,
    ) -> Result<Self, SdkError> {
        if ordinal == 0 {
            return Err(invalid(
                "SALES_STAGE_ORDINAL_INVALID",
                "deal.stage.ordinal",
                "stage ordinal must be greater than zero",
            ));
        }
        Ok(Self {
            pipeline_id,
            stage_id,
            ordinal,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DealStatus {
    Open,
    Won,
    Lost,
}

/// An exact probability with explicit provenance, not an inferred value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbabilityEstimate {
    Manual(BasisPoints),
    StageDefault(BasisPoints),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbabilitySource {
    #[default]
    Manual,
    StageDefault,
}

impl ProbabilityEstimate {
    pub const fn from_parts(source: ProbabilitySource, value: BasisPoints) -> Self {
        match source {
            ProbabilitySource::Manual => Self::Manual(value),
            ProbabilitySource::StageDefault => Self::StageDefault(value),
        }
    }

    pub const fn basis_points(self) -> BasisPoints {
        match self {
            Self::Manual(value) | Self::StageDefault(value) => value,
        }
    }

    pub const fn source(self) -> ProbabilitySource {
        match self {
            Self::Manual(_) => ProbabilitySource::Manual,
            Self::StageDefault(_) => ProbabilitySource::StageDefault,
        }
    }
}

/// Explicit operator assessment; closed Deals retain it for reopening but
/// do not enter prospective forecast totals.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForecastCategory {
    #[default]
    Pipeline,
    BestCase,
    Commit,
    Omitted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DealCloseOutcome {
    status: DealStatus,
    reason_code: ReasonCode,
    closed_at_unix_nanos: i64,
}

impl DealCloseOutcome {
    pub const fn status(&self) -> DealStatus {
        self.status
    }

    pub fn reason_code(&self) -> &ReasonCode {
        &self.reason_code
    }

    pub const fn closed_at_unix_nanos(&self) -> i64 {
        self.closed_at_unix_nanos
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deal {
    deal_id: RecordId,
    name: String,
    owner: DealOwner,
    account: Option<ResourceRef>,
    primary_contact: Option<ResourceRef>,
    stage: DealStage,
    status: DealStatus,
    amount: Option<Money>,
    expected_close_date: Option<CalendarDate>,
    probability: ProbabilityEstimate,
    forecast_category: ForecastCategory,
    close_outcome: Option<DealCloseOutcome>,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DealSnapshot {
    pub deal_id: RecordId,
    pub name: String,
    pub owner: DealOwner,
    pub account: Option<ResourceRef>,
    pub primary_contact: Option<ResourceRef>,
    pub stage: DealStage,
    pub status: DealStatus,
    pub amount: Option<Money>,
    pub expected_close_date: Option<CalendarDate>,
    pub probability: ProbabilityEstimate,
    pub forecast_category: ForecastCategory,
    pub close_outcome: Option<DealCloseOutcomeSnapshot>,
    pub created_at_unix_nanos: i64,
    pub updated_at_unix_nanos: i64,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DealCloseOutcomeSnapshot {
    pub status: DealStatus,
    pub reason_code: ReasonCode,
    pub closed_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateDeal {
    pub deal_id: RecordId,
    pub name: String,
    pub owner: DealOwner,
    pub account: Option<ResourceRef>,
    pub primary_contact: Option<ResourceRef>,
    pub stage: DealStage,
    pub amount: Option<Money>,
    pub expected_close_date: Option<CalendarDate>,
    pub probability: ProbabilityEstimate,
    pub forecast_category: ForecastCategory,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateDeal {
    pub expected_version: i64,
    pub name: Patch<String>,
    pub owner: Patch<DealOwner>,
    pub account: Patch<ResourceRef>,
    pub primary_contact: Patch<ResourceRef>,
    pub amount: Patch<Money>,
    pub expected_close_date: Patch<CalendarDate>,
    pub probability: Patch<ProbabilityEstimate>,
    pub forecast_category: Option<ForecastCategory>,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageTransitionPolicy {
    pub allow_regression: bool,
    pub allow_skip: bool,
}

impl Default for StageTransitionPolicy {
    fn default() -> Self {
        Self {
            allow_regression: false,
            allow_skip: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvanceDealStage {
    pub expected_version: i64,
    pub target_stage: DealStage,
    pub target_stage_probability: Option<BasisPoints>,
    pub target_status: DealStatus,
    pub close_reason_code: Option<ReasonCode>,
    pub occurred_at_unix_nanos: i64,
    pub policy: StageTransitionPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloseDeal {
    pub expected_version: i64,
    pub outcome: DealStatus,
    pub reason_code: ReasonCode,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReopenDeal {
    pub expected_version: i64,
    pub reason_code: ReasonCode,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DealSort {
    UpdatedAtDescending,
    ExpectedCloseDateAscending,
    AmountDescending,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DealListQuery {
    pub page: PageRequest,
    pub owner: Option<DealOwner>,
    pub pipeline_id: Option<PipelineId>,
    pub status: Option<DealStatus>,
    pub sort: DealSort,
}

impl Deal {
    pub fn deal_id(&self) -> &RecordId {
        &self.deal_id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn owner(&self) -> &DealOwner {
        &self.owner
    }
    pub fn account(&self) -> Option<&ResourceRef> {
        self.account.as_ref()
    }
    pub fn primary_contact(&self) -> Option<&ResourceRef> {
        self.primary_contact.as_ref()
    }
    pub fn stage(&self) -> &DealStage {
        &self.stage
    }
    pub const fn status(&self) -> DealStatus {
        self.status
    }
    pub fn amount(&self) -> Option<&Money> {
        self.amount.as_ref()
    }
    pub const fn expected_close_date(&self) -> Option<CalendarDate> {
        self.expected_close_date
    }
    pub const fn probability(&self) -> BasisPoints {
        self.probability.basis_points()
    }
    pub const fn probability_source(&self) -> ProbabilitySource {
        self.probability.source()
    }
    pub const fn forecast_category(&self) -> ForecastCategory {
        self.forecast_category
    }
    pub fn close_outcome(&self) -> Option<&DealCloseOutcome> {
        self.close_outcome.as_ref()
    }
    pub const fn created_at_unix_nanos(&self) -> i64 {
        self.created_at_unix_nanos
    }
    pub const fn updated_at_unix_nanos(&self) -> i64 {
        self.updated_at_unix_nanos
    }
    pub const fn version(&self) -> i64 {
        self.version
    }

    pub fn rehydrate(snapshot: DealSnapshot) -> Result<Self, SdkError> {
        validate_name(&snapshot.name)?;
        validate_optional_resource("deal.account", snapshot.account.as_ref())?;
        validate_optional_resource("deal.primary_contact", snapshot.primary_contact.as_ref())?;
        validate_optional_amount(snapshot.amount.as_ref())?;
        validate_timestamp("deal.created_at_unix_nanos", snapshot.created_at_unix_nanos)?;
        validate_timestamp("deal.updated_at_unix_nanos", snapshot.updated_at_unix_nanos)?;
        if snapshot.updated_at_unix_nanos < snapshot.created_at_unix_nanos {
            return Err(invalid(
                "SALES_DEAL_PERSISTED_TIME_INVALID",
                "deal.updated_at_unix_nanos",
                "updated time cannot precede created time",
            ));
        }
        if snapshot.version <= 0 {
            return Err(invalid(
                "SALES_DEAL_PERSISTED_VERSION_INVALID",
                "deal.version",
                "persisted deal version must be positive",
            ));
        }

        let close_outcome = match (snapshot.status, snapshot.close_outcome) {
            (DealStatus::Open, None) => None,
            (DealStatus::Open, Some(_)) => {
                return Err(invalid(
                    "SALES_OPEN_DEAL_CLOSE_OUTCOME_FORBIDDEN",
                    "deal.close_outcome",
                    "an open deal cannot contain a close outcome",
                ));
            }
            (DealStatus::Won | DealStatus::Lost, None) => {
                return Err(invalid(
                    "SALES_CLOSED_DEAL_OUTCOME_REQUIRED",
                    "deal.close_outcome",
                    "a closed deal requires a close outcome",
                ));
            }
            (status, Some(outcome)) => {
                if outcome.status != status {
                    return Err(invalid(
                        "SALES_CLOSE_OUTCOME_STATUS_MISMATCH",
                        "deal.close_outcome.status",
                        "close outcome status must match the deal status",
                    ));
                }
                validate_timestamp(
                    "deal.close_outcome.closed_at_unix_nanos",
                    outcome.closed_at_unix_nanos,
                )?;
                if outcome.closed_at_unix_nanos < snapshot.created_at_unix_nanos
                    || outcome.closed_at_unix_nanos > snapshot.updated_at_unix_nanos
                {
                    return Err(invalid(
                        "SALES_CLOSE_OUTCOME_TIME_INVALID",
                        "deal.close_outcome.closed_at_unix_nanos",
                        "close time must fall between created and updated time",
                    ));
                }
                Some(DealCloseOutcome {
                    status: outcome.status,
                    reason_code: outcome.reason_code,
                    closed_at_unix_nanos: outcome.closed_at_unix_nanos,
                })
            }
        };

        Ok(Self {
            deal_id: snapshot.deal_id,
            name: snapshot.name,
            owner: snapshot.owner,
            account: snapshot.account,
            primary_contact: snapshot.primary_contact,
            stage: snapshot.stage,
            status: snapshot.status,
            amount: snapshot.amount,
            expected_close_date: snapshot.expected_close_date,
            probability: snapshot.probability,
            forecast_category: snapshot.forecast_category,
            close_outcome,
            created_at_unix_nanos: snapshot.created_at_unix_nanos,
            updated_at_unix_nanos: snapshot.updated_at_unix_nanos,
            version: snapshot.version,
        })
    }

    pub fn snapshot(&self) -> DealSnapshot {
        DealSnapshot {
            deal_id: self.deal_id.clone(),
            name: self.name.clone(),
            owner: self.owner.clone(),
            account: self.account.clone(),
            primary_contact: self.primary_contact.clone(),
            stage: self.stage.clone(),
            status: self.status,
            amount: self.amount.clone(),
            expected_close_date: self.expected_close_date,
            probability: self.probability,
            forecast_category: self.forecast_category,
            close_outcome: self
                .close_outcome
                .as_ref()
                .map(|outcome| DealCloseOutcomeSnapshot {
                    status: outcome.status,
                    reason_code: outcome.reason_code.clone(),
                    closed_at_unix_nanos: outcome.closed_at_unix_nanos,
                }),
            created_at_unix_nanos: self.created_at_unix_nanos,
            updated_at_unix_nanos: self.updated_at_unix_nanos,
            version: self.version,
        }
    }

    pub fn create(command: CreateDeal) -> Result<Self, SdkError> {
        validate_name(&command.name)?;
        validate_timestamp(
            "deal.occurred_at_unix_nanos",
            command.occurred_at_unix_nanos,
        )?;
        validate_optional_resource("deal.account", command.account.as_ref())?;
        validate_optional_resource("deal.primary_contact", command.primary_contact.as_ref())?;
        validate_optional_amount(command.amount.as_ref())?;

        Ok(Self {
            deal_id: command.deal_id,
            name: command.name,
            owner: command.owner,
            account: command.account,
            primary_contact: command.primary_contact,
            stage: command.stage,
            status: DealStatus::Open,
            amount: command.amount,
            expected_close_date: command.expected_close_date,
            probability: command.probability,
            forecast_category: command.forecast_category,
            close_outcome: None,
            created_at_unix_nanos: command.occurred_at_unix_nanos,
            updated_at_unix_nanos: command.occurred_at_unix_nanos,
            version: 1,
        })
    }

    pub fn apply_update(&mut self, command: UpdateDeal) -> Result<(), SdkError> {
        self.require_version(command.expected_version)?;
        if self.status != DealStatus::Open {
            return Err(conflict(
                "SALES_DEAL_ALREADY_CLOSED",
                "a closed deal cannot be edited before explicit reopening",
            ));
        }
        // Validate every patch against a candidate before changing authoritative
        // aggregate state. A later invalid patch must not leave an earlier edit.
        let mut candidate = self.clone();
        candidate.apply_update_checked(command)?;
        *self = candidate;
        Ok(())
    }

    fn apply_update_checked(&mut self, command: UpdateDeal) -> Result<(), SdkError> {
        self.require_version(command.expected_version)?;
        self.require_monotonic_time(command.occurred_at_unix_nanos)?;

        match command.name {
            Patch::Keep => {}
            Patch::Set(name) => {
                validate_name(&name)?;
                self.name = name;
            }
            Patch::Clear => {
                return Err(invalid(
                    "SALES_DEAL_NAME_REQUIRED",
                    "deal.name",
                    "deal name cannot be cleared",
                ));
            }
        }

        match command.owner {
            Patch::Keep => {}
            Patch::Set(owner) => self.owner = owner,
            Patch::Clear => {
                return Err(invalid(
                    "SALES_DEAL_OWNER_REQUIRED",
                    "deal.owner",
                    "deal owner cannot be cleared",
                ));
            }
        }

        apply_optional_resource_patch("deal.account", &mut self.account, command.account)?;
        apply_optional_resource_patch(
            "deal.primary_contact",
            &mut self.primary_contact,
            command.primary_contact,
        )?;

        match command.amount {
            Patch::Keep => {}
            Patch::Set(amount) => {
                validate_optional_amount(Some(&amount))?;
                self.amount = Some(amount);
            }
            Patch::Clear => self.amount = None,
        }

        match command.expected_close_date {
            Patch::Keep => {}
            Patch::Set(date) => self.expected_close_date = Some(date),
            Patch::Clear => self.expected_close_date = None,
        }

        match command.probability {
            Patch::Keep => {}
            Patch::Set(probability) => self.probability = probability,
            Patch::Clear => {
                return Err(invalid(
                    "SALES_DEAL_PROBABILITY_REQUIRED",
                    "deal.probability",
                    "deal probability cannot be cleared",
                ));
            }
        }

        if let Some(category) = command.forecast_category {
            self.forecast_category = category;
        }
        self.updated_at_unix_nanos = command.occurred_at_unix_nanos;
        self.version += 1;
        Ok(())
    }

    pub fn advance_stage(&mut self, command: AdvanceDealStage) -> Result<(), SdkError> {
        self.require_version(command.expected_version)?;
        self.require_monotonic_time(command.occurred_at_unix_nanos)?;

        if self.status != DealStatus::Open {
            return Err(conflict(
                "SALES_DEAL_ALREADY_CLOSED",
                "a closed deal cannot change stage",
            ));
        }
        if self.stage.pipeline_id != command.target_stage.pipeline_id {
            return Err(invalid(
                "SALES_PIPELINE_CHANGE_FORBIDDEN",
                "deal.target_stage.pipeline_id",
                "stage advancement cannot change the deal pipeline",
            ));
        }
        // Domain operations are distinct. The stage transition cannot imply
        // a terminal outcome; callers must use CloseDeal.
        if command.target_status != DealStatus::Open {
            return Err(invalid(
                "SALES_CLOSE_SEPARATE_ACTION_REQUIRED",
                "deal.target_status",
                "use CloseDeal to finish the current stage",
            ));
        }
        // A terminal outcome is independent of pipeline progression: closing
        // must preserve the actual current stage, not invent a synthetic one.
        // Open-stage transitions still require a distinct stage and obey policy.
        if command.target_status == DealStatus::Open {
            if self.stage.stage_id == command.target_stage.stage_id
                || self.stage.ordinal == command.target_stage.ordinal
            {
                return Err(invalid(
                    "SALES_STAGE_UNCHANGED",
                    "deal.target_stage",
                    "target stage must differ from the current stage",
                ));
            }
            if command.target_stage.ordinal < self.stage.ordinal && !command.policy.allow_regression
            {
                return Err(conflict(
                    "SALES_STAGE_REGRESSION_FORBIDDEN",
                    "stage regression requires an explicit transition policy",
                ));
            }
            if command.target_stage.ordinal > self.stage.ordinal.saturating_add(1)
                && !command.policy.allow_skip
            {
                return Err(conflict(
                    "SALES_STAGE_SKIP_FORBIDDEN",
                    "skipping intermediate stages is not permitted by the transition policy",
                ));
            }
        } else if self.stage.stage_id != command.target_stage.stage_id
            || self.stage.ordinal != command.target_stage.ordinal
        {
            return Err(invalid(
                "SALES_CLOSE_STAGE_MISMATCH",
                "deal.target_stage",
                "closing a deal must preserve the current stage",
            ));
        }

        let close_outcome = match command.target_status {
            DealStatus::Open => {
                if command.close_reason_code.is_some() {
                    return Err(invalid(
                        "SALES_OPEN_DEAL_CLOSE_REASON_FORBIDDEN",
                        "deal.close_reason_code",
                        "an open deal cannot have a close reason",
                    ));
                }
                None
            }
            DealStatus::Won | DealStatus::Lost => {
                let reason_code = command.close_reason_code.ok_or_else(|| {
                    invalid(
                        "SALES_CLOSE_REASON_REQUIRED",
                        "deal.close_reason_code",
                        "won and lost deals require a close reason code",
                    )
                })?;
                Some(DealCloseOutcome {
                    status: command.target_status,
                    reason_code,
                    closed_at_unix_nanos: command.occurred_at_unix_nanos,
                })
            }
        };

        let probability = match self.probability {
            ProbabilityEstimate::Manual(_) => self.probability,
            ProbabilityEstimate::StageDefault(_) => {
                let value = command.target_stage_probability.ok_or_else(|| {
                    invalid(
                        "SALES_STAGE_PROBABILITY_REQUIRED",
                        "deal.target_stage",
                        "stage-derived probability requires the authoritative target default",
                    )
                })?;
                ProbabilityEstimate::StageDefault(value)
            }
        };
        self.stage = command.target_stage;
        self.probability = probability;
        self.status = command.target_status;
        self.close_outcome = close_outcome;
        self.updated_at_unix_nanos = command.occurred_at_unix_nanos;
        self.version += 1;
        Ok(())
    }

    pub fn close(&mut self, command: CloseDeal) -> Result<(), SdkError> {
        self.require_version(command.expected_version)?;
        self.require_monotonic_time(command.occurred_at_unix_nanos)?;
        if self.status != DealStatus::Open {
            return Err(conflict(
                "SALES_DEAL_ALREADY_CLOSED",
                "a closed deal requires explicit reopening",
            ));
        }
        if !matches!(command.outcome, DealStatus::Won | DealStatus::Lost) {
            return Err(invalid(
                "SALES_CLOSE_OUTCOME_REQUIRED",
                "deal.outcome",
                "close outcome must be Won or Lost",
            ));
        }
        self.status = command.outcome;
        self.close_outcome = Some(DealCloseOutcome {
            status: command.outcome,
            reason_code: command.reason_code,
            closed_at_unix_nanos: command.occurred_at_unix_nanos,
        });
        self.updated_at_unix_nanos = command.occurred_at_unix_nanos;
        self.version += 1;
        Ok(())
    }

    pub fn reopen(&mut self, command: ReopenDeal) -> Result<(), SdkError> {
        self.reopen_with_owner(command, None)
    }

    /// Reopen and optionally reassign in one domain version, not a second
    /// UpdateDeal mutation. The application owner validates live eligibility.
    pub fn reopen_with_owner(
        &mut self,
        command: ReopenDeal,
        owner: Option<DealOwner>,
    ) -> Result<(), SdkError> {
        self.require_version(command.expected_version)?;
        self.require_monotonic_time(command.occurred_at_unix_nanos)?;
        if self.status == DealStatus::Open {
            return Err(conflict(
                "SALES_DEAL_ALREADY_OPEN",
                "only closed deals can reopen",
            ));
        }
        // ReasonCode is a validated, bounded domain identifier; the D1 event
        // persists it with the same CAS/audit/receipt mutation.
        let _reason = command.reason_code;
        if let Some(owner) = owner {
            self.owner = owner;
        }
        self.status = DealStatus::Open;
        self.close_outcome = None;
        self.updated_at_unix_nanos = command.occurred_at_unix_nanos;
        self.version += 1;
        Ok(())
    }

    fn require_version(&self, expected_version: i64) -> Result<(), SdkError> {
        if expected_version != self.version {
            return Err(conflict(
                "SALES_DEAL_VERSION_CONFLICT",
                format!(
                    "expected deal version {expected_version}, found {}",
                    self.version
                ),
            ));
        }
        if self.version == i64::MAX {
            return Err(conflict(
                "SALES_DEAL_VERSION_EXHAUSTED",
                "deal version cannot be incremented",
            ));
        }
        Ok(())
    }

    fn require_monotonic_time(&self, occurred_at_unix_nanos: i64) -> Result<(), SdkError> {
        validate_timestamp("deal.occurred_at_unix_nanos", occurred_at_unix_nanos)?;
        if occurred_at_unix_nanos < self.updated_at_unix_nanos {
            return Err(invalid(
                "SALES_DEAL_TIME_REGRESSION",
                "deal.occurred_at_unix_nanos",
                "deal mutation time cannot precede the previous mutation",
            ));
        }
        Ok(())
    }
}

fn apply_optional_resource_patch(
    field: &'static str,
    target: &mut Option<ResourceRef>,
    patch: Patch<ResourceRef>,
) -> Result<(), SdkError> {
    match patch {
        Patch::Keep => {}
        Patch::Set(reference) => {
            validate_resource(field, &reference)?;
            *target = Some(reference);
        }
        Patch::Clear => *target = None,
    }
    Ok(())
}

fn validate_optional_resource(
    field: &'static str,
    reference: Option<&ResourceRef>,
) -> Result<(), SdkError> {
    if let Some(reference) = reference {
        validate_resource(field, reference)?;
    }
    Ok(())
}

fn validate_resource(field: &'static str, reference: &ResourceRef) -> Result<(), SdkError> {
    if reference.resource_type.is_empty()
        || reference.resource_id.is_empty()
        || reference.resource_type.chars().any(char::is_control)
        || reference.resource_id.chars().any(char::is_control)
        || reference.version.is_some_and(|version| version <= 0)
    {
        return Err(invalid(
            "SALES_RESOURCE_REFERENCE_INVALID",
            field,
            "resource reference type and id must be non-empty, contain no control characters, and any version must be positive",
        ));
    }
    Ok(())
}

fn validate_optional_amount(amount: Option<&Money>) -> Result<(), SdkError> {
    if amount.is_some_and(|amount| amount.minor_units() < 0) {
        return Err(invalid(
            "SALES_DEAL_AMOUNT_NEGATIVE",
            "deal.amount.minor_units",
            "deal amount must not be negative",
        ));
    }
    Ok(())
}

fn validate_name(name: &str) -> Result<(), SdkError> {
    let trimmed = name.trim();
    if trimmed.is_empty() || name.len() > MAX_NAME_BYTES || name.chars().any(char::is_control) {
        return Err(invalid(
            "SALES_DEAL_NAME_INVALID",
            "deal.name",
            format!(
                "deal name must be non-empty, contain no control characters and not exceed {MAX_NAME_BYTES} bytes"
            ),
        ));
    }
    Ok(())
}

fn validate_identifier(field: &'static str, value: &str) -> Result<(), SdkError> {
    if value.is_empty()
        || value.len() > MAX_IDENTIFIER_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':'))
    {
        return Err(invalid(
            "SALES_IDENTIFIER_INVALID",
            field,
            format!(
                "identifier must use ASCII letters, digits, '.', '_', '-' or ':' and not exceed {MAX_IDENTIFIER_BYTES} bytes"
            ),
        ));
    }
    if field == "deal.close_reason_code" && value.len() > MAX_REASON_BYTES {
        return Err(invalid(
            "SALES_CLOSE_REASON_TOO_LONG",
            field,
            format!("close reason must not exceed {MAX_REASON_BYTES} bytes"),
        ));
    }
    Ok(())
}

fn validate_timestamp(field: &'static str, value: i64) -> Result<(), SdkError> {
    if value < 0 {
        return Err(invalid(
            "SALES_TIMESTAMP_INVALID",
            field,
            "timestamp must not be negative",
        ));
    }
    Ok(())
}

fn invalid(code: &'static str, field: &'static str, safe_message: impl Into<String>) -> SdkError {
    let safe_message = safe_message.into();
    let mut error = SdkError::new(
        code,
        ErrorCategory::InvalidArgument,
        false,
        "The deal request contains invalid data.",
    );
    error.field_violations.push(FieldViolation {
        field: FieldName::try_new(field).expect("static field path must be valid"),
        code: code.to_owned(),
        safe_message,
    });
    error
}

fn conflict(code: &'static str, safe_message: impl Into<String>) -> SdkError {
    SdkError::new(code, ErrorCategory::Conflict, false, safe_message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_core_contracts::{CurrencyCode, PageSize};

    fn stage(id: &str, ordinal: u16) -> DealStage {
        DealStage::try_new(
            PipelineId::try_new("pipeline.enterprise").unwrap(),
            StageId::try_new(id).unwrap(),
            ordinal,
        )
        .unwrap()
    }

    fn open_deal() -> Deal {
        Deal::create(CreateDeal {
            deal_id: RecordId::try_new("deal-1").unwrap(),
            name: "Enterprise renewal".to_owned(),
            owner: DealOwner::Actor(ActorId::try_new("actor-1").unwrap()),
            account: Some(ResourceRef {
                resource_type: "crm.account".to_owned(),
                resource_id: "account-1".to_owned(),
                version: Some(3),
            }),
            primary_contact: None,
            stage: stage("qualification", 1),
            amount: Some(Money::new(1_250_000, CurrencyCode::try_new("USD").unwrap())),
            expected_close_date: Some(CalendarDate::try_new(2026, 12, 31).unwrap()),
            probability: ProbabilityEstimate::Manual(BasisPoints::try_new(2_500).unwrap()),
            forecast_category: ForecastCategory::Pipeline,
            occurred_at_unix_nanos: 10,
        })
        .unwrap()
    }

    use proptest::prelude::*;

    proptest! {
        #![proptest_config(proptest::test_runner::Config {
            cases: 256,
            rng_seed: proptest::test_runner::RngSeed::Fixed(471_2026),
            ..proptest::test_runner::Config::default()
        })]

        #[test]
        fn property_failed_later_patch_and_stale_replay_are_atomic(
            label in "[A-Za-z0-9]{1,32}", units in 1i64..=i64::MAX,
            stale in 2i64..=i64::MAX, moment in 10i64..=i64::MAX,
        ) {
            let mut deal = open_deal();
            let before = deal.clone();
            let command = UpdateDeal {
                expected_version: 1,
                name: Patch::Set(label),
                owner: Patch::Keep,
                account: Patch::Clear,
                primary_contact: Patch::Keep,
                amount: Patch::Set(Money::new(-i128::from(units), CurrencyCode::try_new("USD").unwrap())),
                expected_close_date: Patch::Keep,
                probability: Patch::Keep,
                forecast_category: None,
                occurred_at_unix_nanos: moment,
            };
            prop_assert!(deal.apply_update(command.clone()).is_err());
            prop_assert_eq!(&deal, &before);
            let valid = UpdateDeal {
                amount: Patch::Set(Money::new(i128::from(units), CurrencyCode::try_new("USD").unwrap())),
                ..command
            };
            let stale_result = deal.apply_update(UpdateDeal { expected_version: stale, ..valid.clone() });
            prop_assert!(stale_result.is_err());
            prop_assert_eq!(&deal, &before);
            let mut exhausted_snapshot = before.snapshot();
            exhausted_snapshot.version = i64::MAX;
            let mut exhausted = Deal::rehydrate(exhausted_snapshot).unwrap();
            let exhausted_before = exhausted.clone();
            let error = exhausted.apply_update(UpdateDeal {
                expected_version: i64::MAX,
                ..valid.clone()
            }).unwrap_err();
            prop_assert_eq!(error.code, "SALES_DEAL_VERSION_EXHAUSTED");
            prop_assert_eq!(exhausted, exhausted_before);
            deal.apply_update(valid.clone()).unwrap();
            prop_assert_eq!(deal.version(), 2);
            let committed = deal.clone();
            prop_assert!(deal.apply_update(valid).is_err());
            prop_assert_eq!(&deal, &committed);
            let bytes = crate::encode_deal_state(&deal).unwrap();
            prop_assert_eq!(crate::decode_deal_state(&bytes).unwrap(), committed);
        }
    }

    #[test]
    fn creates_exact_money_open_deal() {
        let deal = open_deal();
        assert_eq!(deal.version, 1);
        assert_eq!(deal.status, DealStatus::Open);
        assert_eq!(deal.amount().unwrap().minor_units(), 1_250_000);
    }

    #[test]
    fn forecast_source_and_stage_change_are_atomic() {
        let mut deal = open_deal();
        deal.apply_update(UpdateDeal {
            expected_version: 1,
            name: Patch::Keep,
            owner: Patch::Keep,
            account: Patch::Keep,
            primary_contact: Patch::Keep,
            amount: Patch::Keep,
            expected_close_date: Patch::Keep,
            probability: Patch::Set(ProbabilityEstimate::StageDefault(
                BasisPoints::try_new(1_000).unwrap(),
            )),
            forecast_category: Some(ForecastCategory::Commit),
            occurred_at_unix_nanos: 20,
        })
        .unwrap();
        let command = AdvanceDealStage {
            expected_version: 2,
            target_stage: stage("proposal", 2),
            target_stage_probability: None,
            target_status: DealStatus::Open,
            close_reason_code: None,
            occurred_at_unix_nanos: 30,
            policy: StageTransitionPolicy::default(),
        };
        let before = deal.clone();
        assert_eq!(
            deal.advance_stage(command.clone()).unwrap_err().code,
            "SALES_STAGE_PROBABILITY_REQUIRED"
        );
        assert_eq!(deal, before);
        deal.advance_stage(AdvanceDealStage {
            target_stage_probability: Some(BasisPoints::try_new(6_000).unwrap()),
            ..command
        })
        .unwrap();
        assert_eq!(deal.probability_source(), ProbabilitySource::StageDefault);
        assert_eq!(deal.probability().get(), 6_000);
        assert_eq!(deal.forecast_category(), ForecastCategory::Commit);
    }

    #[test]
    fn update_uses_explicit_clear_and_optimistic_version() {
        let mut deal = open_deal();
        deal.apply_update(UpdateDeal {
            expected_version: 1,
            name: Patch::Keep,
            owner: Patch::Keep,
            account: Patch::Clear,
            primary_contact: Patch::Keep,
            amount: Patch::Keep,
            expected_close_date: Patch::Clear,
            probability: Patch::Set(ProbabilityEstimate::Manual(
                BasisPoints::try_new(5_000).unwrap(),
            )),
            forecast_category: None,
            occurred_at_unix_nanos: 20,
        })
        .unwrap();
        assert!(deal.account.is_none());
        assert!(deal.expected_close_date.is_none());
        assert_eq!(deal.probability().get(), 5_000);
        assert_eq!(deal.version, 2);

        let error = deal
            .apply_update(UpdateDeal {
                expected_version: 1,
                name: Patch::Keep,
                owner: Patch::Keep,
                account: Patch::Keep,
                primary_contact: Patch::Keep,
                amount: Patch::Keep,
                expected_close_date: Patch::Keep,
                probability: Patch::Keep,
                forecast_category: None,
                occurred_at_unix_nanos: 21,
            })
            .unwrap_err();
        assert_eq!(error.code, "SALES_DEAL_VERSION_CONFLICT");
    }

    #[test]
    fn close_and_reopen_have_separate_versioned_commands() {
        let mut deal = open_deal();
        let stage = deal.stage().clone();
        assert_eq!(
            deal.advance_stage(AdvanceDealStage {
                expected_version: 1,
                target_stage: stage.clone(),
                target_stage_probability: None,
                target_status: DealStatus::Won,
                close_reason_code: Some(ReasonCode::try_new("won").unwrap()),
                occurred_at_unix_nanos: 30,
                policy: StageTransitionPolicy::default(),
            })
            .unwrap_err()
            .code,
            "SALES_CLOSE_SEPARATE_ACTION_REQUIRED"
        );

        assert_eq!(
            deal.close(CloseDeal {
                expected_version: 1,
                outcome: DealStatus::Open,
                reason_code: ReasonCode::try_new("won").unwrap(),
                occurred_at_unix_nanos: 30,
            })
            .unwrap_err()
            .code,
            "SALES_CLOSE_OUTCOME_REQUIRED"
        );
        assert_eq!(deal.version(), 1);
        deal.close(CloseDeal {
            expected_version: 1,
            outcome: DealStatus::Won,
            reason_code: ReasonCode::try_new("won").unwrap(),
            occurred_at_unix_nanos: 30,
        })
        .unwrap();
        assert_eq!(deal.status(), DealStatus::Won);
        assert_eq!(deal.stage(), &stage);
        assert_eq!(deal.version(), 2);
        assert_eq!(deal.close_outcome().unwrap().reason_code().as_str(), "won");
        assert_eq!(
            deal.close(CloseDeal {
                expected_version: 2,
                outcome: DealStatus::Lost,
                reason_code: ReasonCode::try_new("again").unwrap(),
                occurred_at_unix_nanos: 40,
            })
            .unwrap_err()
            .code,
            "SALES_DEAL_ALREADY_CLOSED"
        );
        assert_eq!(
            deal.reopen(ReopenDeal {
                expected_version: 1,
                reason_code: ReasonCode::try_new("revisit").unwrap(),
                occurred_at_unix_nanos: 40,
            })
            .unwrap_err()
            .code,
            "SALES_DEAL_VERSION_CONFLICT"
        );
        deal.reopen(ReopenDeal {
            expected_version: 2,
            reason_code: ReasonCode::try_new("revisit").unwrap(),
            occurred_at_unix_nanos: 40,
        })
        .unwrap();
        assert_eq!(deal.status(), DealStatus::Open);
        assert!(deal.close_outcome().is_none());
        assert_eq!(deal.stage(), &stage);
        assert_eq!(deal.version(), 3);
        assert_eq!(
            deal.reopen(ReopenDeal {
                expected_version: 3,
                reason_code: ReasonCode::try_new("again").unwrap(),
                occurred_at_unix_nanos: 50,
            })
            .unwrap_err()
            .code,
            "SALES_DEAL_ALREADY_OPEN"
        );
        assert_eq!(deal.version(), 3);
        deal.close(CloseDeal {
            expected_version: 3,
            outcome: DealStatus::Lost,
            reason_code: ReasonCode::try_new("lost").unwrap(),
            occurred_at_unix_nanos: 50,
        })
        .unwrap();
        assert_eq!(deal.status(), DealStatus::Lost);
        assert_eq!(deal.version(), 4);
    }

    #[test]
    fn terminal_edit_and_move_are_forbidden() {
        let mut deal = open_deal();
        deal.close(CloseDeal {
            expected_version: 1,
            outcome: DealStatus::Won,
            reason_code: ReasonCode::try_new("signed").unwrap(),
            occurred_at_unix_nanos: 30,
        })
        .unwrap();
        assert_eq!(
            deal.apply_update(UpdateDeal {
                expected_version: 2,
                name: Patch::Set("Forbidden".into()),
                owner: Patch::Keep,
                account: Patch::Keep,
                primary_contact: Patch::Keep,
                amount: Patch::Keep,
                expected_close_date: Patch::Keep,
                probability: Patch::Keep,
                forecast_category: None,
                occurred_at_unix_nanos: 40,
            })
            .unwrap_err()
            .code,
            "SALES_DEAL_ALREADY_CLOSED"
        );
        assert_eq!(
            deal.advance_stage(AdvanceDealStage {
                expected_version: 2,
                target_stage: stage("negotiation", 3),
                target_stage_probability: None,
                target_status: DealStatus::Open,
                close_reason_code: None,
                occurred_at_unix_nanos: 40,
                policy: StageTransitionPolicy::default(),
            })
            .unwrap_err()
            .code,
            "SALES_DEAL_ALREADY_CLOSED"
        );
        assert_eq!(deal.version(), 2);
    }

    #[test]
    fn invalid_late_patch_does_not_partially_change_deal() {
        let mut deal = open_deal();
        let previous_name = deal.name().to_owned();
        let previous_version = deal.version();
        let error = deal
            .apply_update(UpdateDeal {
                expected_version: previous_version,
                name: Patch::Set("Would corrupt earlier field".into()),
                owner: Patch::Keep,
                account: Patch::Keep,
                primary_contact: Patch::Keep,
                amount: Patch::Keep,
                expected_close_date: Patch::Keep,
                probability: Patch::Clear,
                forecast_category: None,
                occurred_at_unix_nanos: 20,
            })
            .unwrap_err();
        assert_eq!(error.code, "SALES_DEAL_PROBABILITY_REQUIRED");
        assert_eq!(deal.name(), previous_name);
        assert_eq!(deal.version(), previous_version);
    }

    #[test]
    fn regression_requires_explicit_policy() {
        let mut deal = open_deal();
        deal.advance_stage(AdvanceDealStage {
            expected_version: 1,
            target_stage: stage("proposal", 3),
            target_stage_probability: None,
            target_status: DealStatus::Open,
            close_reason_code: None,
            occurred_at_unix_nanos: 20,
            policy: StageTransitionPolicy::default(),
        })
        .unwrap();

        let error = deal
            .advance_stage(AdvanceDealStage {
                expected_version: 2,
                target_stage: stage("qualification", 1),
                target_stage_probability: None,
                target_status: DealStatus::Open,
                close_reason_code: None,
                occurred_at_unix_nanos: 30,
                policy: StageTransitionPolicy::default(),
            })
            .unwrap_err();
        assert_eq!(error.code, "SALES_STAGE_REGRESSION_FORBIDDEN");
    }

    #[test]
    fn list_query_uses_bounded_shared_pagination() {
        let query = DealListQuery {
            page: PageRequest {
                cursor: None,
                page_size: PageSize::try_new(100).unwrap(),
            },
            owner: None,
            pipeline_id: Some(PipelineId::try_new("pipeline.enterprise").unwrap()),
            status: Some(DealStatus::Open),
            sort: DealSort::UpdatedAtDescending,
        };
        assert_eq!(query.page.page_size.get(), 100);
    }
}
