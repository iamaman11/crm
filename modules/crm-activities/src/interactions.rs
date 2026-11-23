//! Immutable human work-history events belonging to the original Activities owner.
//! This is distinct from scheduled Tasks and from Sales stage-transition audit.
use crm_module_sdk::{ErrorCategory, RecordId, SdkError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionKind {
    Call,
    Meeting,
    Note,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Interaction {
    pub interaction_id: String,
    pub account_id: String,
    pub deal_id: Option<String>,
    pub kind: InteractionKind,
    pub text: String,
    pub author: String,
    pub occurred_at_nanos: i64,
    pub recorded_at_nanos: i64,
    pub version: i64,
}

pub struct CreateInteraction {
    pub interaction_id: String,
    pub account_id: String,
    pub deal_id: Option<String>,
    pub kind: InteractionKind,
    pub text: String,
    pub occurred_at_nanos: i64,
}

fn invalid(code: &'static str) -> SdkError {
    SdkError::new(code, ErrorCategory::InvalidArgument, false, "Invalid interaction")
}

pub fn validate_identifier(value: &str) -> Result<(), SdkError> {
    if value != value.trim() || value.contains('|') || value.contains('/') {
        return Err(invalid("ACTIVITIES_INTERACTION_ID_INVALID"));
    }
    RecordId::try_new(value.to_owned())
        .map(|_| ())
        .map_err(|_| invalid("ACTIVITIES_INTERACTION_ID_INVALID"))
}

impl Interaction {
    pub fn create(
        input: CreateInteraction,
        author: &str,
        recorded_at_nanos: i64,
    ) -> Result<Self, SdkError> {
        validate_identifier(&input.interaction_id)?;
        validate_identifier(&input.account_id)?;
        if let Some(deal) = &input.deal_id {
            validate_identifier(deal)?;
        }
        if input.text.trim().is_empty() || input.text.len() > 16_384 {
            return Err(invalid("ACTIVITIES_INTERACTION_TEXT_INVALID"));
        }
        if author.trim().is_empty() || author.len() > 256 {
            return Err(invalid("ACTIVITIES_INTERACTION_AUTHOR_INVALID"));
        }
        // D1 sorts a fixed-width, nonnegative nanosecond timestamp precisely
        // as a TEXT key, without floating-point loss of subsecond detail.
        if input.occurred_at_nanos < 0 || recorded_at_nanos <= 0 {
            return Err(invalid("ACTIVITIES_INTERACTION_TIME_INVALID"));
        }
        Ok(Self {
            interaction_id: input.interaction_id,
            account_id: input.account_id,
            deal_id: input.deal_id,
            kind: input.kind,
            text: input.text,
            author: author.to_owned(),
            occurred_at_nanos: input.occurred_at_nanos,
            recorded_at_nanos,
            version: 1,
        })
    }

    pub fn verify_stored(&self) -> Result<(), SdkError> {
        let input = CreateInteraction {
            interaction_id: self.interaction_id.clone(),
            account_id: self.account_id.clone(),
            deal_id: self.deal_id.clone(),
            kind: self.kind,
            text: self.text.clone(),
            occurred_at_nanos: self.occurred_at_nanos,
        };
        if *self != Self::create(input, &self.author, self.recorded_at_nanos)?
            || self.version != 1
        {
            return Err(invalid("ACTIVITIES_INTERACTION_STATE_INVALID"));
        }
        Ok(())
    }

    pub fn chronological_key(&self) -> (i64, &str) {
        (self.occurred_at_nanos, &self.interaction_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(occurred: i64) -> CreateInteraction {
        CreateInteraction {
            interaction_id: "interaction-1".into(),
            account_id: "account-1".into(),
            deal_id: None,
            kind: InteractionKind::Note,
            text: "Follow up with customer".into(),
            occurred_at_nanos: occurred,
        }
    }

    #[test]
    fn immutable_author_and_occurrence_are_distinct() {
        let saved = Interaction::create(event(100), "alice", 200).unwrap();
        assert_eq!(saved.author, "alice");
        assert_eq!(saved.occurred_at_nanos, 100);
        assert_eq!(saved.recorded_at_nanos, 200);
        saved.verify_stored().unwrap();
    }

    #[test]
    fn invalid_unrelated_identity_or_empty_note_is_rejected() {
        let mut input = event(100);
        input.deal_id = Some("bad/deal".into());
        assert!(Interaction::create(input, "alice", 200).is_err());
        let mut input = event(100);
        input.text = " ".into();
        assert!(Interaction::create(input, "alice", 200).is_err());
        assert!(Interaction::create(event(-1), "alice", 200).is_err());
        assert!(Interaction::create(event(100), " ", 200).is_err());
    }

    #[test]
    fn earlier_event_sorts_before_newer_recorded_event() {
        let older = Interaction::create(event(100), "alice", 500).unwrap();
        let newer = Interaction::create(event(400), "alice", 450).unwrap();
        assert!(older.chronological_key() < newer.chronological_key());
    }
}
