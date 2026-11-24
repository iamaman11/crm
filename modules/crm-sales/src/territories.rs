//! Sales-owned commercial responsibility, independent of Account identity.
use crate::SalesOwnerScope;
use crm_module_sdk::{ErrorCategory, RecordId, SdkError};
use serde::{Deserialize, Serialize};

fn invalid(code: &'static str) -> SdkError {
    SdkError::new(
        code,
        ErrorCategory::InvalidArgument,
        false,
        "Invalid sales coverage.",
    )
}

fn conflict(code: &'static str) -> SdkError {
    SdkError::new(
        code,
        ErrorCategory::Conflict,
        false,
        "Sales coverage conflict.",
    )
}

pub(crate) fn check_id(id: &str) -> Result<(), SdkError> {
    if id != id.trim()
        || id.contains(['/', '\\', '%'])
        || id.chars().any(char::is_control)
        || RecordId::try_new(id.to_owned()).is_err()
    {
        return Err(invalid("SALES_COVERAGE_ID_INVALID"));
    }
    Ok(())
}

pub(crate) fn next_version(previous: Option<i64>, expected: Option<i64>) -> Result<i64, SdkError> {
    if previous != expected {
        return Err(conflict("SALES_COVERAGE_VERSION_CONFLICT"));
    }
    previous
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| conflict("SALES_COVERAGE_VERSION_OVERFLOW"))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SalesTerritory {
    pub territory_id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub default_owner: SalesOwnerScope,
    pub archived: bool,
    pub version: i64,
}

impl SalesTerritory {
    pub fn validate(&self) -> Result<(), SdkError> {
        check_id(&self.territory_id)?;
        if self.name.is_empty()
            || self.name != self.name.trim()
            || self.name.len() > 240
            || self.name.chars().any(char::is_control)
        {
            return Err(invalid("SALES_TERRITORY_NAME_INVALID"));
        }
        if let Some(parent) = &self.parent_id {
            check_id(parent)?;
            if parent == &self.territory_id {
                return Err(invalid("SALES_TERRITORY_CYCLE"));
            }
        }
        self.default_owner.validate()?;
        if self.version <= 0 {
            return Err(invalid("SALES_COVERAGE_VERSION_INVALID"));
        }
        Ok(())
    }

    pub fn revise(mut self, old: Option<&Self>, expected: Option<i64>) -> Result<Self, SdkError> {
        self.version = next_version(old.map(|value| value.version), expected)?;
        if let Some(old) = old {
            if old.territory_id != self.territory_id {
                return Err(invalid("SALES_COVERAGE_ID_INVALID"));
            }
            if self.archived
                && (self.parent_id != old.parent_id
                    || self.default_owner != old.default_owner
                    || self.name != old.name)
            {
                return Err(invalid("SALES_COVERAGE_ARCHIVE_ONLY"));
            }
        } else if self.archived {
            return Err(invalid("SALES_COVERAGE_CREATE_ARCHIVED"));
        }
        self.validate()?;
        Ok(self)
    }

    pub fn decode(state: &str) -> Result<Self, SdkError> {
        let value: Self =
            serde_json::from_str(state).map_err(|_| invalid("SALES_COVERAGE_STATE_INVALID"))?;
        value.validate()?;
        Ok(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SalesOverlayRole {
    Specialist,
    AccountSupport,
    ExecutiveSponsor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SalesAccountOverlay {
    pub role: SalesOverlayRole,
    pub owner: SalesOwnerScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SalesAccountAssignment {
    pub account_id: String,
    pub territory_id: Option<String>,
    pub primary_owner: SalesOwnerScope,
    pub overlays: Vec<SalesAccountOverlay>,
    pub archived: bool,
    pub version: i64,
}

impl SalesAccountAssignment {
    pub fn validate(&self) -> Result<(), SdkError> {
        check_id(&self.account_id)?;
        if let Some(id) = &self.territory_id {
            check_id(id)?;
        }
        self.primary_owner.validate()?;
        // Bound the number of owner eligibility binds in a single D1 write.
        if self.overlays.len() > 40 || self.version <= 0 {
            return Err(invalid("SALES_ACCOUNT_ASSIGNMENT_INVALID"));
        }
        for (index, item) in self.overlays.iter().enumerate() {
            item.owner.validate()?;
            if item.owner == self.primary_owner || self.overlays[..index].contains(item) {
                return Err(invalid("SALES_ACCOUNT_OVERLAY_DUPLICATE"));
            }
        }
        Ok(())
    }

    pub fn revise(mut self, old: Option<&Self>, expected: Option<i64>) -> Result<Self, SdkError> {
        self.version = next_version(old.map(|value| value.version), expected)?;
        if let Some(old) = old {
            if old.account_id != self.account_id {
                return Err(invalid("SALES_COVERAGE_ID_INVALID"));
            }
            if self.archived
                && (self.territory_id != old.territory_id
                    || self.primary_owner != old.primary_owner
                    || self.overlays != old.overlays)
            {
                return Err(invalid("SALES_COVERAGE_ARCHIVE_ONLY"));
            }
        } else if self.archived {
            return Err(invalid("SALES_COVERAGE_CREATE_ARCHIVED"));
        }
        self.validate()?;
        Ok(self)
    }

    pub fn decode(state: &str) -> Result<Self, SdkError> {
        let value: Self =
            serde_json::from_str(state).map_err(|_| invalid("SALES_COVERAGE_STATE_INVALID"))?;
        value.validate()?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn territory_revisions_and_self_parent() {
        let draft = SalesTerritory {
            territory_id: "north".into(),
            name: "North".into(),
            parent_id: None,
            default_owner: SalesOwnerScope::Team("team-a".into()),
            archived: false,
            version: 1,
        };
        let first = draft.clone().revise(None, None).unwrap();
        assert_eq!(first.version, 1);
        assert_eq!(
            draft.clone().revise(Some(&first), Some(1)).unwrap().version,
            2
        );
        assert!(draft.clone().revise(Some(&first), Some(2)).is_err());
        let mut cycle = draft;
        cycle.parent_id = Some("north".into());
        assert!(cycle.revise(Some(&first), Some(1)).is_err());
    }

    #[test]
    fn assignment_overlays_and_archive_retention() {
        let mut draft = SalesAccountAssignment {
            account_id: "account-a".into(),
            territory_id: Some("north".into()),
            primary_owner: SalesOwnerScope::Actor("a@example.test".into()),
            overlays: vec![SalesAccountOverlay {
                role: SalesOverlayRole::Specialist,
                owner: SalesOwnerScope::Team("specialists".into()),
            }],
            archived: false,
            version: 1,
        };
        let first = draft.clone().revise(None, None).unwrap();
        assert_eq!(
            SalesAccountAssignment::decode(&serde_json::to_string(&first).unwrap()).unwrap(),
            first
        );
        draft.overlays.push(draft.overlays[0].clone());
        assert!(draft.clone().revise(Some(&first), Some(1)).is_err());
        draft.overlays.pop();
        draft.archived = true;
        assert_eq!(
            draft.clone().revise(Some(&first), Some(1)).unwrap().version,
            2
        );
        draft.overlays.clear();
        assert!(draft.revise(Some(&first), Some(1)).is_err());
    }
}
