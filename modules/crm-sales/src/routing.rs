//! Typed Sales routing. The domain matches rules and selects a target;
//! the D1 adapter supplies authoritative tenant-scoped facts and guards them.
use crate::{SalesAccountAssignment, SalesOwnerScope, SalesTerritory};
use crm_core_contracts::CurrencyCode;
use crm_module_sdk::{ErrorCategory, SdkError};
use serde::{Deserialize, Serialize};

fn invalid(code: &'static str) -> SdkError {
    SdkError::new(code, ErrorCategory::InvalidArgument, false, "Invalid sales routing.")
}

fn conflict(code: &'static str) -> SdkError {
    SdkError::new(code, ErrorCategory::Conflict, false, "Sales routing conflict.")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SalesDispatchMode {
    Direct,
    LeastOpenDeals,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SalesCapacityOverflow {
    Reject,
    TeamQueue,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SalesRoutingCriteria {
    pub territory_id: Option<String>,
    pub pipeline_id: Option<String>,
    pub stage_id: Option<String>,
    pub currency_code: Option<String>,
    pub minimum_minor_units: Option<i64>,
    pub maximum_minor_units: Option<i64>,
}

impl SalesRoutingCriteria {
    pub fn validate(&self) -> Result<(), SdkError> {
        for id in [&self.territory_id, &self.pipeline_id, &self.stage_id]
            .into_iter()
            .flatten()
        {
            crate::territories::check_id(id)?;
        }
        if self.stage_id.is_some() && self.pipeline_id.is_none() {
            return Err(invalid("SALES_ROUTING_STAGE_REQUIRES_PIPELINE"));
        }
        if let Some(currency) = &self.currency_code {
            CurrencyCode::try_new(currency.clone())
                .map_err(|_| invalid("SALES_ROUTING_CURRENCY_INVALID"))?;
        } else if self.minimum_minor_units.is_some() || self.maximum_minor_units.is_some() {
            return Err(invalid("SALES_ROUTING_CURRENCY_REQUIRED"));
        }
        if self.minimum_minor_units.is_some_and(|n| n < 0)
            || self.maximum_minor_units.is_some_and(|n| n < 0)
            || matches!((self.minimum_minor_units, self.maximum_minor_units), (Some(a), Some(b)) if a > b)
        {
            return Err(invalid("SALES_ROUTING_AMOUNT_RANGE_INVALID"));
        }
        Ok(())
    }

    pub fn matches(&self, facts: &SalesRoutingFacts, territory: Option<&str>) -> bool {
        self.territory_id.as_deref().is_none_or(|id| Some(id) == territory)
            && self.pipeline_id.as_deref().is_none_or(|id| id == facts.pipeline_id)
            && self.stage_id.as_deref().is_none_or(|id| id == facts.stage_id)
            && self.currency_code.as_deref().is_none_or(|code| Some(code) == facts.currency_code.as_deref())
            && self.minimum_minor_units.is_none_or(|min| facts.amount_minor_units.is_some_and(|n| n >= min))
            && self.maximum_minor_units.is_none_or(|max| facts.amount_minor_units.is_some_and(|n| n <= max))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SalesRoutingRule {
    pub rule_id: String,
    pub name: String,
    pub priority: u32,
    pub criteria: SalesRoutingCriteria,
    pub target: SalesOwnerScope,
    pub dispatch: SalesDispatchMode,
    pub maximum_open_deals: Option<u32>,
    pub overflow: SalesCapacityOverflow,
    pub archived: bool,
    pub version: i64,
}

impl SalesRoutingRule {
    pub fn validate(&self) -> Result<(), SdkError> {
        crate::territories::check_id(&self.rule_id)?;
        if self.name.is_empty()
            || self.name != self.name.trim()
            || self.name.len() > 240
            || self.name.chars().any(char::is_control)
            || self.priority == 0
            || self.version <= 0
        {
            return Err(invalid("SALES_ROUTING_RULE_INVALID"));
        }
        self.criteria.validate()?;
        self.target.validate()?;
        if self.dispatch == SalesDispatchMode::Direct
            && (self.maximum_open_deals.is_some() || self.overflow != SalesCapacityOverflow::Reject)
            || self.dispatch == SalesDispatchMode::LeastOpenDeals
                && !matches!(self.target, SalesOwnerScope::Team(_))
            || self.maximum_open_deals == Some(0)
            || self.overflow == SalesCapacityOverflow::TeamQueue && self.maximum_open_deals.is_none()
        {
            return Err(invalid("SALES_ROUTING_DISPATCH_INVALID"));
        }
        Ok(())
    }

    pub fn revise(mut self, old: Option<&Self>, expected: Option<i64>) -> Result<Self, SdkError> {
        self.version = crate::territories::next_version(old.map(|v| v.version), expected)?;
        if let Some(old) = old {
            if old.rule_id != self.rule_id {
                return Err(invalid("SALES_ROUTING_RULE_ID_INVALID"));
            }
            if self.archived {
                let mut retained = old.clone();
                retained.archived = true;
                retained.version = self.version;
                if retained != self {
                    return Err(invalid("SALES_ROUTING_ARCHIVE_ONLY"));
                }
            }
        } else if self.archived {
            return Err(invalid("SALES_ROUTING_CREATE_ARCHIVED"));
        }
        self.validate()?;
        Ok(self)
    }

    pub fn decode(state: &str) -> Result<Self, SdkError> {
        let value: Self = serde_json::from_str(state)
            .map_err(|_| invalid("SALES_ROUTING_STATE_INVALID"))?;
        value.validate()?;
        Ok(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SalesRoutingFacts {
    pub account_id: String,
    pub pipeline_id: String,
    pub stage_id: String,
    pub territory_id: Option<String>,
    pub currency_code: Option<String>,
    pub amount_minor_units: Option<i64>,
}

impl SalesRoutingFacts {
    pub fn validate(&self) -> Result<(), SdkError> {
        for id in [&self.account_id, &self.pipeline_id, &self.stage_id] {
            crate::territories::check_id(id)?;
        }
        if let Some(id) = &self.territory_id {
            crate::territories::check_id(id)?;
        }
        match (&self.currency_code, self.amount_minor_units) {
            (None, None) => Ok(()),
            (Some(code), Some(n)) if n >= 0 => CurrencyCode::try_new(code.clone())
                .map(|_| ())
                .map_err(|_| invalid("SALES_ROUTING_CURRENCY_INVALID")),
            _ => Err(invalid("SALES_ROUTING_AMOUNT_INVALID")),
        }
    }

    pub fn effective_territory<'a>(
        &'a self,
        assignment: Option<&'a SalesAccountAssignment>,
    ) -> Result<Option<&'a str>, SdkError> {
        let assigned = assignment
            .filter(|a| !a.archived)
            .and_then(|a| a.territory_id.as_deref());
        if matches!((self.territory_id.as_deref(), assigned), (Some(a), Some(b)) if a != b) {
            return Err(conflict("SALES_ROUTING_TERRITORY_CONFLICT"));
        }
        Ok(self.territory_id.as_deref().or(assigned))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SalesRoutingSource {
    Rule,
    Account,
    Territory,
    Manual,
    Creator,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SalesRoutingDecision {
    pub source: SalesRoutingSource,
    pub selected_owner: SalesOwnerScope,
    pub target: SalesOwnerScope,
    pub rule_id: Option<String>,
    pub rule_version: Option<i64>,
    pub rule_name: Option<String>,
    pub territory_id: Option<String>,
    pub territory_version: Option<i64>,
    pub territory_name: Option<String>,
    pub assignment_version: Option<i64>,
    pub dispatch: SalesDispatchMode,
    pub selected_open_deals: Option<u64>,
    pub maximum_open_deals: Option<u32>,
    pub capacity_overflow: bool,
}

impl SalesRoutingDecision {
    pub fn explicit(owner: SalesOwnerScope, source: SalesRoutingSource) -> Self {
        Self {
            source,
            selected_owner: owner.clone(),
            target: owner,
            rule_id: None,
            rule_version: None,
            rule_name: None,
            territory_id: None,
            territory_version: None,
            territory_name: None,
            assignment_version: None,
            dispatch: SalesDispatchMode::Direct,
            selected_open_deals: None,
            maximum_open_deals: None,
            capacity_overflow: false,
        }
    }

    pub fn validate(&self) -> Result<(), SdkError> {
        self.selected_owner.validate()?;
        self.target.validate()?;
        let rule = self.rule_id.is_some()
            && self.rule_version.is_some_and(|v| v > 0)
            && self.rule_name.as_ref().is_some_and(|v| !v.is_empty());
        if let Some(id) = &self.rule_id {
            crate::territories::check_id(id)?;
        }
        if let Some(id) = &self.territory_id {
            crate::territories::check_id(id)?;
        }
        if (self.source == SalesRoutingSource::Rule) != rule
            || self.source != SalesRoutingSource::Rule
                && (self.rule_id.is_some() || self.rule_version.is_some() || self.rule_name.is_some())
            || self.territory_id.is_some() != self.territory_version.is_some_and(|v| v > 0)
            || self.territory_id.is_some() != self.territory_name.as_ref().is_some_and(|v| !v.is_empty())
            || self.assignment_version.is_some_and(|v| v <= 0)
            || self.source == SalesRoutingSource::Account && self.assignment_version.is_none()
            || self.source == SalesRoutingSource::Territory && self.territory_id.is_none()
            || self.source != SalesRoutingSource::Rule && self.dispatch != SalesDispatchMode::Direct
            || matches!(self.source, SalesRoutingSource::Manual | SalesRoutingSource::Creator)
                && (self.assignment_version.is_some() || self.territory_id.is_some())
            || self.source == SalesRoutingSource::Creator
                && !matches!(self.selected_owner, SalesOwnerScope::Actor(_))
            || self.maximum_open_deals == Some(0)
            || self.dispatch == SalesDispatchMode::Direct
                && (self.target != self.selected_owner
                    || self.capacity_overflow
                    || self.selected_open_deals.is_some()
                    || self.maximum_open_deals.is_some())
            || self.dispatch == SalesDispatchMode::LeastOpenDeals
                && (!matches!(self.target, SalesOwnerScope::Team(_))
                    || if self.capacity_overflow {
                        self.target != self.selected_owner || self.selected_open_deals.is_some()
                            || self.maximum_open_deals.is_none()
                    } else {
                        !matches!(self.selected_owner, SalesOwnerScope::Actor(_))
                            || self.selected_open_deals.is_none()
                            || self.maximum_open_deals.zip(self.selected_open_deals)
                                .is_some_and(|(max, n)| n >= u64::from(max))
                    })
        {
            return Err(invalid("SALES_ROUTING_DECISION_INVALID"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SalesRoutingCandidate {
    pub email: String,
    pub open_deals: u64,
}

pub struct SalesRoutingSelection {
    pub decision: SalesRoutingDecision,
    pub overflow: SalesCapacityOverflow,
}

impl SalesRoutingSelection {
    // Eligibility and counts are authoritative inputs from Access and D1.
    pub fn dispatch(
        mut self,
        eligible: bool,
        candidates: &[SalesRoutingCandidate],
    ) -> Result<SalesRoutingDecision, SdkError> {
        if !eligible {
            return Err(conflict("SALES_ROUTING_TARGET_NOT_ELIGIBLE"));
        }
        if self.decision.dispatch == SalesDispatchMode::LeastOpenDeals {
            let candidate = candidates
                .iter()
                .filter(|c| self.decision.maximum_open_deals.is_none_or(|max| c.open_deals < u64::from(max)))
                .min_by(|a, b| (a.open_deals, &a.email).cmp(&(b.open_deals, &b.email)));
            if let Some(candidate) = candidate {
                self.decision.selected_owner = SalesOwnerScope::Actor(candidate.email.clone());
                self.decision.selected_open_deals = Some(candidate.open_deals);
            } else if self.overflow == SalesCapacityOverflow::TeamQueue && !candidates.is_empty() {
                self.decision.capacity_overflow = true;
            } else {
                return Err(conflict("SALES_ROUTING_CAPACITY_EXHAUSTED"));
            }
        }
        self.decision.validate()?;
        Ok(self.decision)
    }
}

/// Select first rule by unique priority, then Account, then Territory.
/// A matching but unavailable target is rejected by dispatch, never skipped.
pub fn select_sales_route(
    facts: &SalesRoutingFacts,
    rules: &[SalesRoutingRule],
    assignment: Option<&SalesAccountAssignment>,
    territory: Option<&SalesTerritory>,
) -> Result<SalesRoutingSelection, SdkError> {
    facts.validate()?;
    let assignment = assignment.filter(|a| !a.archived);
    let effective = facts.effective_territory(assignment)?;
    if effective.is_some() && !territory.is_some_and(|t| !t.archived && Some(t.territory_id.as_str()) == effective) {
        return Err(conflict("SALES_ROUTING_TERRITORY_UNAVAILABLE"));
    }
    let rule = rules.iter().filter(|r| !r.archived && r.criteria.matches(facts, effective))
        .min_by_key(|r| r.priority);
    if let Some(rule) = rule {
        if rules.iter().filter(|r| !r.archived && r.priority == rule.priority).count() != 1 {
            return Err(conflict("SALES_ROUTING_PRIORITY_CONFLICT"));
        }
    }
    let (target, source) = if let Some(rule) = rule {
        (rule.target.clone(), SalesRoutingSource::Rule)
    } else if let Some(a) = assignment {
        (a.primary_owner.clone(), SalesRoutingSource::Account)
    } else if let Some(t) = territory {
        (t.default_owner.clone(), SalesRoutingSource::Territory)
    } else {
        return Err(conflict("SALES_ROUTING_NO_MATCH"));
    };
    let mut decision = SalesRoutingDecision::explicit(target, source);
    decision.territory_id = territory.map(|t| t.territory_id.clone());
    decision.territory_version = territory.map(|t| t.version);
    decision.territory_name = territory.map(|t| t.name.clone());
    decision.assignment_version = assignment.map(|a| a.version);
    let mut overflow = SalesCapacityOverflow::Reject;
    if let Some(rule) = rule {
        decision.rule_id = Some(rule.rule_id.clone());
        decision.rule_version = Some(rule.version);
        decision.rule_name = Some(rule.name.clone());
        decision.dispatch = rule.dispatch;
        decision.maximum_open_deals = rule.maximum_open_deals;
        overflow = rule.overflow;
    }
    Ok(SalesRoutingSelection { decision, overflow })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> SalesRoutingFacts {
        SalesRoutingFacts {
            account_id: "account".into(),
            pipeline_id: "pipeline".into(),
            stage_id: "stage".into(),
            territory_id: None,
            currency_code: Some("EUR".into()),
            amount_minor_units: Some(100),
        }
    }

    fn rule(priority: u32) -> SalesRoutingRule {
        SalesRoutingRule {
            rule_id: format!("rule-{priority}"),
            name: format!("Rule {priority}"),
            priority,
            criteria: SalesRoutingCriteria {
                pipeline_id: Some("pipeline".into()),
                currency_code: Some("EUR".into()),
                minimum_minor_units: Some(100),
                maximum_minor_units: Some(100),
                ..Default::default()
            },
            target: SalesOwnerScope::Team("team".into()),
            dispatch: SalesDispatchMode::LeastOpenDeals,
            maximum_open_deals: Some(2),
            overflow: SalesCapacityOverflow::Reject,
            archived: false,
            version: 1,
        }
    }

    #[test]
    fn priority_currency_and_capacity() {
        let rules = [rule(20), rule(10)];
        for rule in &rules {
            rule.validate().unwrap();
        }
        let candidates = [
            SalesRoutingCandidate { email: "b@example.test".into(), open_deals: 1 },
            SalesRoutingCandidate { email: "a@example.test".into(), open_deals: 1 },
        ];
        let chosen = select_sales_route(&facts(), &rules, None, None).unwrap()
            .dispatch(true, &candidates).unwrap();
        assert_eq!(chosen.rule_id.as_deref(), Some("rule-10"));
        assert_eq!(chosen.selected_owner, SalesOwnerScope::Actor("a@example.test".into()));
        let mut full = candidates;
        for c in &mut full { c.open_deals = 2; }
        assert!(select_sales_route(&facts(), &rules, None, None).unwrap()
            .dispatch(true, &full).is_err());
        let mut overflow = rule(1);
        overflow.overflow = SalesCapacityOverflow::TeamQueue;
        assert!(select_sales_route(&facts(), &[overflow], None, None).unwrap()
            .dispatch(true, &full).unwrap().capacity_overflow);
        let mut wrong = facts();
        wrong.currency_code = Some("USD".into());
        assert!(select_sales_route(&wrong, &rules, None, None).is_err());
        assert!(select_sales_route(&facts(), &rules, None, None).unwrap()
            .dispatch(false, &[]).is_err());
    }
}
