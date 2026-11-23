//! Tenant-scoped named Sales Deal filters. Business validation and CAS live in
//! original crm-sales; D1 and Connect are adapters, never parallel owners.
use crate::{DealOwner, DealStatus, PipelineId, TeamId};
use crm_module_sdk::{ActorId, ErrorCategory, RecordId, SdkError};
use serde::{Deserialize, Serialize};

// A saved query dimension, never a second owner of a Deal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SalesOwnerScope {
    Actor(String),
    Team(String),
}
impl SalesOwnerScope {
    pub fn domain_owner(&self) -> Result<DealOwner, SdkError> {
        match self {
            Self::Actor(value) => ActorId::try_new(value.as_str())
                .map(DealOwner::Actor)
                .map_err(|_| invalid("SALES_OWNER_INVALID")),
            Self::Team(value) => TeamId::try_new(value.as_str()).map(DealOwner::Team),
        }
    }
    pub fn validate(&self) -> Result<(), SdkError> {
        let value = match self {
            Self::Actor(value) | Self::Team(value) => value,
        };
        if value.is_empty()
            || value != value.trim()
            || value.contains(['/', '\\', '%'])
            || value.chars().any(char::is_control)
        {
            return Err(invalid("SALES_OWNER_INVALID"));
        }
        self.domain_owner().map(|_| ())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SalesDealView {
    pub view_id: String,
    pub name: String,
    pub pipeline_id: Option<String>,
    pub status: Option<DealStatus>,
    pub account_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_filter: Option<SalesOwnerScope>,
    pub version: i64,
    pub archived: bool,
}

fn failure(code: &'static str, category: ErrorCategory) -> SdkError {
    SdkError::new(code, category, false, "Invalid saved Sales view.")
}
fn invalid(code: &'static str) -> SdkError {
    failure(code, ErrorCategory::InvalidArgument)
}
fn valid_record_id(value: &str) -> bool {
    value == value.trim()
        && !value.contains('/')
        && !value.chars().any(char::is_control)
        && RecordId::try_new(value.to_owned()).is_ok()
}
fn validate(
    view_id: &str,
    name: &str,
    pipeline_id: &Option<String>,
    account_id: &Option<String>,
) -> Result<(), SdkError> {
    if !valid_record_id(view_id) {
        return Err(invalid("SALES_VIEW_ID_INVALID"));
    }
    if name.trim().is_empty()
        || name != name.trim()
        || name.len() > 240
        || name.chars().any(char::is_control)
    {
        return Err(invalid("SALES_VIEW_NAME_INVALID"));
    }
    if pipeline_id.as_ref().is_some_and(|id| {
        id != id.trim() || id.contains('/') || PipelineId::try_new(id.to_owned()).is_err()
    }) {
        return Err(invalid("SALES_VIEW_PIPELINE_ID_INVALID"));
    }
    if account_id.as_ref().is_some_and(|id| !valid_record_id(id)) {
        return Err(invalid("SALES_VIEW_ACCOUNT_ID_INVALID"));
    }
    Ok(())
}
impl SalesDealView {
    pub fn create(
        view_id: String,
        name: String,
        pipeline_id: Option<String>,
        status: Option<DealStatus>,
        account_id: Option<String>,
    ) -> Result<Self, SdkError> {
        validate(&view_id, &name, &pipeline_id, &account_id)?;
        Ok(Self {
            view_id,
            name,
            pipeline_id,
            status,
            account_id,
            owner_filter: None,
            version: 1,
            archived: false,
        })
    }

    pub fn reconfigure(
        &self,
        expected_version: i64,
        name: String,
        pipeline_id: Option<String>,
        status: Option<DealStatus>,
        account_id: Option<String>,
        archived: bool,
    ) -> Result<Self, SdkError> {
        if self.version != expected_version {
            return Err(failure(
                "SALES_VIEW_VERSION_CONFLICT",
                ErrorCategory::Conflict,
            ));
        }
        validate(&self.view_id, &name, &pipeline_id, &account_id)?;
        Ok(Self {
            view_id: self.view_id.clone(),
            name,
            pipeline_id,
            status,
            account_id,
            owner_filter: None,
            version: self
                .version
                .checked_add(1)
                .ok_or_else(|| failure("SALES_VIEW_VERSION_OVERFLOW", ErrorCategory::Conflict))?,
            archived,
        })
    }

    pub fn with_owner_filter(mut self, owner: Option<SalesOwnerScope>) -> Result<Self, SdkError> {
        if let Some(scope) = &owner {
            scope.validate()?;
        }
        self.owner_filter = owner;
        Ok(self)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, SdkError> {
        let value: Self =
            serde_json::from_slice(bytes).map_err(|_| invalid("SALES_VIEW_STATE_INVALID"))?;
        validate(
            &value.view_id,
            &value.name,
            &value.pipeline_id,
            &value.account_id,
        )
        .map_err(|_| invalid("SALES_VIEW_STATE_INVALID"))?;
        if let Some(owner) = &value.owner_filter {
            owner
                .validate()
                .map_err(|_| invalid("SALES_VIEW_STATE_INVALID"))?;
        }
        if value.version < 1 {
            return Err(invalid("SALES_VIEW_STATE_INVALID"));
        }
        Ok(value)
    }

    pub fn encode(&self) -> Result<String, SdkError> {
        serde_json::to_string(self).map_err(|_| invalid("SALES_VIEW_ENCODE_FAILED"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_views_validate_all_filters_and_exact_roundtrip() {
        let first = SalesDealView::create(
            "view-1".into(),
            "Открытые сделки".into(),
            Some("pipeline.enterprise".into()),
            Some(DealStatus::Open),
            Some("account-1".into()),
        )
        .unwrap();
        assert_eq!(
            SalesDealView::decode(first.encode().unwrap().as_bytes()).unwrap(),
            first
        );
        let second = first
            .reconfigure(
                1,
                "Только выигранные".into(),
                None,
                Some(DealStatus::Won),
                None,
                false,
            )
            .unwrap();
        assert_eq!(second.version, 2);
        assert_eq!(second.pipeline_id, None);
        assert_eq!(second.status, Some(DealStatus::Won));
        assert_eq!(
            second
                .reconfigure(1, second.name.clone(), None, None, None, true)
                .unwrap_err()
                .code,
            "SALES_VIEW_VERSION_CONFLICT"
        );
        let archived = second
            .reconfigure(2, second.name.clone(), None, None, None, true)
            .unwrap();
        assert!(archived.archived);
        assert_eq!(archived.version, 3);
    }

    #[test]
    fn invalid_views_do_not_enter_persistence() {
        assert_eq!(
            SalesDealView::create("bad/id".into(), "Название".into(), None, None, None)
                .unwrap_err()
                .code,
            "SALES_VIEW_ID_INVALID"
        );
        assert_eq!(
            SalesDealView::create("view-1".into(), " ".into(), None, None, None)
                .unwrap_err()
                .code,
            "SALES_VIEW_NAME_INVALID"
        );
        assert_eq!(
            SalesDealView::create(
                "view-1".into(),
                "Название".into(),
                Some("bad/id".into()),
                None,
                None
            )
            .unwrap_err()
            .code,
            "SALES_VIEW_PIPELINE_ID_INVALID"
        );
        assert_eq!(
            SalesDealView::create(
                "view-1".into(),
                "Название".into(),
                None,
                None,
                Some("bad/id".into())
            )
            .unwrap_err()
            .code,
            "SALES_VIEW_ACCOUNT_ID_INVALID"
        );
        assert_eq!(SalesDealView::decode(br#"{"view_id":"view-1","name":"x","pipeline_id":null,"status":"wrong","account_id":null,"version":1,"archived":false}"#)
            .unwrap_err().code, "SALES_VIEW_STATE_INVALID");
    }
}
