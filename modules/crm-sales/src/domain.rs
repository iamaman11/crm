use crm_core_contracts::{BasisPoints, CalendarDate, Money, PageRequest, Patch};
use crm_module_sdk::{
    ActorId, ErrorCategory, FieldName, FieldViolation, RecordId, ResourceRef, SdkError,
};

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
    pub pipeline_id: PipelineId,
    pub stage_id: StageId,
    pub ordinal: u16,
}

impl DealStage {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DealStatus {
    Open,
    Won,
    Lost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DealCloseOutcome {
    pub status: DealStatus,
    pub reason_code: ReasonCode,
    pub closed_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deal {
    pub deal_id: RecordId,
    pub name: String,
    pub owner: DealOwner,
    pub account: Option<ResourceRef>,
    pub primary_contact: Option<ResourceRef>,
    pub stage: DealStage,
    pub status: DealStatus,
    pub amount: Option<Money>,
    pub expected_close_date: Option<CalendarDate>,
    pub probability: BasisPoints,
    pub close_outcome: Option<DealCloseOutcome>,
    pub created_at_unix_nanos: i64,
    pub updated_at_unix_nanos: i64,
    pub version: i64,
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
    pub probability: BasisPoints,
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
    pub probability: Patch<BasisPoints>,
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
    pub target_status: DealStatus,
    pub close_reason_code: Option<ReasonCode>,
    pub occurred_at_unix_nanos: i64,
    pub policy: StageTransitionPolicy,
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
            close_outcome: None,
            created_at_unix_nanos: command.occurred_at_unix_nanos,
            updated_at_unix_nanos: command.occurred_at_unix_nanos,
            version: 1,
        })
    }

    pub fn apply_update(&mut self, command: UpdateDeal) -> Result<(), SdkError> {
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
        if self.stage.stage_id == command.target_stage.stage_id
            || self.stage.ordinal == command.target_stage.ordinal
        {
            return Err(invalid(
                "SALES_STAGE_UNCHANGED",
                "deal.target_stage",
                "target stage must differ from the current stage",
            ));
        }
        if command.target_stage.ordinal < self.stage.ordinal && !command.policy.allow_regression {
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

        self.stage = command.target_stage;
        self.status = command.target_status;
        self.close_outcome = close_outcome;
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
    {
        return Err(invalid(
            "SALES_RESOURCE_REFERENCE_INVALID",
            field,
            "resource reference type and id must be non-empty and contain no control characters",
        ));
    }
    Ok(())
}

fn validate_optional_amount(amount: Option<&Money>) -> Result<(), SdkError> {
    if amount.is_some_and(|amount| amount.minor_units < 0) {
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
            probability: BasisPoints::try_new(2_500).unwrap(),
            occurred_at_unix_nanos: 10,
        })
        .unwrap()
    }

    #[test]
    fn creates_exact_money_open_deal() {
        let deal = open_deal();
        assert_eq!(deal.version, 1);
        assert_eq!(deal.status, DealStatus::Open);
        assert_eq!(deal.amount.unwrap().minor_units, 1_250_000);
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
            probability: Patch::Set(BasisPoints::try_new(5_000).unwrap()),
            occurred_at_unix_nanos: 20,
        })
        .unwrap();
        assert!(deal.account.is_none());
        assert!(deal.expected_close_date.is_none());
        assert_eq!(deal.probability.get(), 5_000);
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
                occurred_at_unix_nanos: 21,
            })
            .unwrap_err();
        assert_eq!(error.code, "SALES_DEAL_VERSION_CONFLICT");
    }

    #[test]
    fn closes_only_with_reason_and_never_advances_after_close() {
        let mut deal = open_deal();
        let missing_reason = deal
            .advance_stage(AdvanceDealStage {
                expected_version: 1,
                target_stage: stage("closed_won", 4),
                target_status: DealStatus::Won,
                close_reason_code: None,
                occurred_at_unix_nanos: 30,
                policy: StageTransitionPolicy::default(),
            })
            .unwrap_err();
        assert_eq!(missing_reason.code, "SALES_CLOSE_REASON_REQUIRED");

        deal.advance_stage(AdvanceDealStage {
            expected_version: 1,
            target_stage: stage("closed_won", 4),
            target_status: DealStatus::Won,
            close_reason_code: Some(ReasonCode::try_new("customer_selected_us").unwrap()),
            occurred_at_unix_nanos: 30,
            policy: StageTransitionPolicy::default(),
        })
        .unwrap();
        assert_eq!(deal.status, DealStatus::Won);

        let error = deal
            .advance_stage(AdvanceDealStage {
                expected_version: 2,
                target_stage: stage("negotiation", 3),
                target_status: DealStatus::Open,
                close_reason_code: None,
                occurred_at_unix_nanos: 40,
                policy: StageTransitionPolicy {
                    allow_regression: true,
                    allow_skip: true,
                },
            })
            .unwrap_err();
        assert_eq!(error.code, "SALES_DEAL_ALREADY_CLOSED");
    }

    #[test]
    fn regression_requires_explicit_policy() {
        let mut deal = open_deal();
        deal.advance_stage(AdvanceDealStage {
            expected_version: 1,
            target_stage: stage("proposal", 3),
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
