//! Immutable commercial facts owned by the original Sales aggregate.
use crate::{Deal, DealOwner, DealStatus, SalesOwnerScope};
use crm_core_contracts::CurrencyCode;
use crm_module_sdk::{ErrorCategory, SdkError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommercialDimension {
    pub id: String,
    pub name: String,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommercialAmount {
    // Preserve exact integer across D1 and JavaScript.
    pub minor_units: String,
    pub currency_code: String,
}

#[derive(Debug, Clone, Default)]
pub struct CloseCreditContext {
    pub teams: Vec<CommercialDimension>,
    pub territory: Option<CommercialDimension>,
    pub assignment_version: Option<i64>,
    pub catalog_version: Option<i64>,
}

/// Explicit close credit intent. Default keeps existing automatic rules.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloseCreditSelection {
    pub team_id: Option<String>,
    pub leave_team_unallocated: bool,
    pub territory_id: Option<String>,
}

impl CloseCreditContext {
    /// Resolve against versioned evidence supplied by storage. No caller
    /// may invent a team membership or override an active account territory.
    pub fn select(
        mut self,
        owner: &DealOwner,
        selection: &CloseCreditSelection,
        explicit_territory: Option<CommercialDimension>,
    ) -> Result<Self, SdkError> {
        let reject = || {
            SdkError::new(
                "SALES_CLOSE_CREDIT_SELECTION_INVALID",
                ErrorCategory::InvalidArgument,
                false,
                "Select an eligible team and territory.",
            )
        };
        if selection.leave_team_unallocated && selection.team_id.is_some() {
            return Err(reject());
        }
        match owner {
            DealOwner::Team(id) => {
                if selection.leave_team_unallocated
                    || selection
                        .team_id
                        .as_ref()
                        .is_some_and(|selected| {
                            selected != id.as_str()
                                || !self.teams.iter().any(|team| &team.id == selected)
                        })
                {
                    return Err(reject());
                }
            }
            DealOwner::Actor(_) if selection.leave_team_unallocated => self.teams.clear(),
            DealOwner::Actor(id) if selection.team_id.is_some() => {
                if id.as_str().starts_with("service:") {
                    return Err(reject());
                }
                self.teams
                    .retain(|team| selection.team_id.as_ref() == Some(&team.id));
                if self.teams.len() != 1 {
                    return Err(reject());
                }
            }
            DealOwner::Actor(_) => {}
        }
        if let Some(id) = &selection.territory_id {
            if let Some(assigned) = &self.territory {
                if &assigned.id != id {
                    return Err(SdkError::new(
                        "SALES_CLOSE_CREDIT_TERRITORY_CONFLICT",
                        ErrorCategory::Conflict,
                        false,
                        "The account already has an assigned territory.",
                    ));
                }
            } else {
                self.territory = Some(
                    explicit_territory
                        .filter(|territory| &territory.id == id)
                        .ok_or_else(reject)?,
                );
            }
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommercialCloseFact {
    pub close_version: i64,
    pub outcome: DealStatus,
    pub closed_at_unix_nanos: i64,
    pub owner: SalesOwnerScope,
    pub account_id: Option<String>,
    pub pipeline_id: String,
    pub stage_id: String,
    pub amount: Option<CommercialAmount>,
    pub credited_actor: Option<String>,
    pub credited_team: Option<CommercialDimension>,
    pub credited_territory: Option<CommercialDimension>,
    pub assignment_version: Option<i64>,
    pub catalog_version: Option<i64>,
}

fn invalid() -> SdkError {
    SdkError::new(
        "SALES_COMMERCIAL_FACT_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "Invalid historical Sales attribution.",
    )
}

impl CommercialCloseFact {
    pub fn capture(
        deal: &Deal,
        outcome: DealStatus,
        closed_at_unix_nanos: i64,
        context: CloseCreditContext,
    ) -> Result<Self, SdkError> {
        let owner = match deal.owner() {
            DealOwner::Actor(id) => SalesOwnerScope::Actor(id.as_str().to_owned()),
            DealOwner::Team(id) => SalesOwnerScope::Team(id.as_str().to_owned()),
        };
        let credited_actor = match &owner {
            SalesOwnerScope::Actor(id) if !id.starts_with("service:") => Some(id.clone()),
            _ => None,
        };
        let credited_team = match &owner {
            SalesOwnerScope::Team(id) => context.teams.into_iter().find(|team| &team.id == id),
            SalesOwnerScope::Actor(_) if credited_actor.is_some() && context.teams.len() == 1 => {
                context.teams.into_iter().next()
            }
            _ => None,
        };
        let value = Self {
            close_version: deal.version().checked_add(1).ok_or_else(invalid)?,
            outcome,
            closed_at_unix_nanos,
            owner,
            account_id: deal.account().map(|account| account.resource_id.clone()),
            pipeline_id: deal.stage().pipeline_id().as_str().to_owned(),
            stage_id: deal.stage().stage_id().as_str().to_owned(),
            amount: deal.amount().map(|money| CommercialAmount {
                minor_units: money.minor_units().to_string(),
                currency_code: money.currency().as_str().to_owned(),
            }),
            credited_actor,
            credited_team,
            credited_territory: context.territory,
            assignment_version: context.assignment_version,
            catalog_version: context.catalog_version,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), SdkError> {
        self.owner.validate()?;
        if self.close_version < 2
            || self.outcome == DealStatus::Open
            || self.closed_at_unix_nanos < 0
            || self.pipeline_id.is_empty()
            || self.stage_id.is_empty()
            || self.assignment_version.is_some_and(|version| version < 1)
            || self.catalog_version.is_some_and(|version| version < 1)
        {
            return Err(invalid());
        }
        if let Some(amount) = &self.amount {
            let units = amount.minor_units.parse::<i128>().map_err(|_| invalid())?;
            if units < 0 || units.to_string() != amount.minor_units {
                return Err(invalid());
            }
            CurrencyCode::try_new(amount.currency_code.clone()).map_err(|_| invalid())?;
        }
        for dimension in [&self.credited_team, &self.credited_territory]
            .into_iter()
            .flatten()
        {
            if dimension.id.is_empty() || dimension.name.is_empty() || dimension.version < 1 {
                return Err(invalid());
            }
        }
        match &self.owner {
            SalesOwnerScope::Actor(id) if !id.starts_with("service:") => {
                if self.credited_actor.as_ref() != Some(id) {
                    return Err(invalid());
                }
            }
            SalesOwnerScope::Actor(_) => {
                if self.credited_actor.is_some() || self.credited_team.is_some() {
                    return Err(invalid());
                }
            }
            SalesOwnerScope::Team(id) => {
                if self.credited_actor.is_some()
                    || self
                        .credited_team
                        .as_ref()
                        .is_some_and(|team| &team.id != id)
                {
                    return Err(invalid());
                }
            }
        }
        Ok(())
    }

    /// Won close books a positive amount; reopen reverses the same fact.
    /// Lost deals book zero; a missing won amount stays unknown.
    pub fn booked_minor_units(&self, reversal: bool) -> Result<Option<i128>, SdkError> {
        self.validate()?;
        if self.outcome == DealStatus::Lost {
            return Ok(Some(0));
        }
        self.amount
            .as_ref()
            .map(|amount| {
                let value = amount.minor_units.parse::<i128>().map_err(|_| invalid())?;
                if reversal {
                    value.checked_neg().ok_or_else(invalid)
                } else {
                    Ok(value)
                }
            })
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_module_sdk::ActorId;

    fn dimension(id: &str) -> CommercialDimension {
        CommercialDimension {
            id: id.into(),
            name: id.into(),
            version: 1,
        }
    }

    #[test]
    fn explicit_credit_respects_membership_and_assigned_territory() {
        let owner = DealOwner::Actor(ActorId::try_new("actor-1").unwrap());
        let context = CloseCreditContext {
            teams: vec![dimension("team-a"), dimension("team-b")],
            territory: Some(dimension("territory-a")),
            ..Default::default()
        };
        let chosen = CloseCreditSelection {
            team_id: Some("team-b".into()),
            ..Default::default()
        };
        assert_eq!(
            context
                .clone()
                .select(&owner, &chosen, None)
                .unwrap()
                .teams,
            vec![dimension("team-b")]
        );
        assert!(
            context
                .clone()
                .select(
                    &owner,
                    &CloseCreditSelection {
                        leave_team_unallocated: true,
                        ..Default::default()
                    },
                    None,
                )
                .unwrap()
                .teams
                .is_empty()
        );
        assert!(
            context
                .clone()
                .select(
                    &owner,
                    &CloseCreditSelection {
                        team_id: Some("unrelated".into()),
                        ..Default::default()
                    },
                    None,
                )
                .is_err()
        );
        assert!(
            context
                .select(
                    &owner,
                    &CloseCreditSelection {
                        territory_id: Some("territory-b".into()),
                        ..Default::default()
                    },
                    Some(dimension("territory-b")),
                )
                .is_err()
        );
    }
}
