//! Tenant-independent decisions for the pre-Deal inbound interest lifecycle.
use crm_module_sdk::{ErrorCategory, RecordId, SdkError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeadStatus {
    New,
    Qualified,
    Rejected,
    Converted,
}

impl LeadStatus {
    pub const fn can_decide(self) -> bool {
        matches!(self, Self::New)
    }

    pub const fn can_convert(self) -> bool {
        matches!(self, Self::Qualified)
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LeadDecision {
    Qualify,
    Reject,
}

impl LeadDecision {
    pub const fn status(self) -> LeadStatus {
        match self {
            Self::Qualify => LeadStatus::Qualified,
            Self::Reject => LeadStatus::Rejected,
        }
    }

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Qualify => "qualified",
            Self::Reject => "rejected",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Lead {
    pub id: RecordId,
    pub original_party_id: RecordId,
    pub name: String,
    pub status: LeadStatus,
    pub reason: String,
    pub deal_id: Option<RecordId>,
    pub version: i64,
}

fn invalid(code: &'static str) -> SdkError {
    SdkError::new(
        code,
        ErrorCategory::InvalidArgument,
        false,
        "Invalid lead input.",
    )
}

fn conflict(code: &'static str) -> SdkError {
    SdkError::new(
        code,
        ErrorCategory::Conflict,
        false,
        "Lead state has changed.",
    )
}

fn text(value: &str, max: usize, code: &'static str) -> Result<String, SdkError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().count() > max || trimmed.chars().any(char::is_control)
    {
        return Err(invalid(code));
    }
    Ok(trimmed.to_owned())
}

impl Lead {
    pub fn create(id: RecordId, original_party_id: RecordId, name: &str) -> Result<Self, SdkError> {
        Ok(Self {
            id,
            original_party_id,
            name: text(name, 200, "SALES_LEAD_NAME_INVALID")?,
            status: LeadStatus::New,
            reason: String::new(),
            deal_id: None,
            version: 1,
        })
    }

    pub fn decide(
        &self,
        expected: i64,
        decision: LeadDecision,
        reason: &str,
    ) -> Result<Self, SdkError> {
        self.check_version(expected)?;
        if !self.status.can_decide() {
            return Err(conflict("SALES_LEAD_DECISION_NOT_ALLOWED"));
        }
        Ok(Self {
            status: decision.status(),
            reason: text(reason, 500, "SALES_LEAD_REASON_REQUIRED")?,
            version: 2,
            ..self.clone()
        })
    }

    pub fn check_conversion(&self, expected: i64) -> Result<(), SdkError> {
        self.check_version(expected)?;
        if !self.status.can_convert() {
            return Err(conflict("SALES_LEAD_CONVERSION_NOT_ALLOWED"));
        }
        Ok(())
    }

    fn check_version(&self, expected: i64) -> Result<(), SdkError> {
        if self.version != expected {
            return Err(conflict("SALES_LEAD_VERSION_CONFLICT"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decisions_require_reasons_and_a_current_new_lead() {
        let id = |value: &str| RecordId::try_new(value).unwrap();
        let new = Lead::create(id("lead"), id("party"), "  Referral ").unwrap();
        assert_eq!(new.name, "Referral");
        assert!(new.check_conversion(1).is_err());
        assert!(new.decide(1, LeadDecision::Qualify, " ").is_err());
        assert!(new.decide(2, LeadDecision::Qualify, "Budget").is_err());

        let qualified = new.decide(1, LeadDecision::Qualify, " Budget ").unwrap();
        assert_eq!(qualified.reason, "Budget");
        assert!(qualified.check_conversion(2).is_ok());
        assert!(qualified.check_conversion(1).is_err());
        assert!(
            qualified
                .decide(2, LeadDecision::Reject, "Changed")
                .is_err()
        );
        let rejected = new.decide(1, LeadDecision::Reject, "No fit").unwrap();
        assert!(rejected.check_conversion(2).is_err());
    }
}
