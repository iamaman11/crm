//! One generated Connect service for ORIGINAL crm.activities automation.
//! Policy and tenant come only from the central signed-Access router.
//! This first vertical supports admin rule publication and explicitly signed
//! manual Interaction→Task execution. Production cron remains disabled.
use super::*;
use crate::activity_automation as automation;

impl auto_wire::AutomationService for AutomationV2Service {
    #[worker::send]
    async fn put_automation_rule(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, auto_wire::PutAutomationRuleRequest>,
    ) -> ServiceResult<auto_wire::PutAutomationRuleResponse> {
        let input = req.to_owned_message();
        let create = input
            .precondition
            .as_option()
            .and_then(|p| p.expected_version)
            .is_none();
        let (key, expected) = wire_precondition(input.precondition.as_option(), create)?;
        if input.trigger != auto_wire::AutomationTrigger::AUTOMATION_TRIGGER_INTERACTION_CREATED {
            return Err(ConnectError::invalid_argument(
                "ACTIVITIES_AUTOMATION_TRIGGER_NOT_ENABLED",
            ));
        }
        let row = automation::RuleInput {
            rule_id: input.rule_id,
            trigger: "interaction_created".into(),
            enabled: input.enabled,
            task_subject: input.task_subject,
            assignee_email: input.assignee_email,
            due_after_seconds: input.due_after_seconds,
        };
        let saved = automation::put(
            &self.d1()?,
            &self.actor,
            row,
            &key,
            expected,
            &self.request_id,
        )
        .await?;
        RpcResponse::ok(auto_wire::PutAutomationRuleResponse {
            rule: Some(Self::rule(&saved.rule)?).into(),
            receipt: Some(core_v2::MutationReceipt {
                request_id: self.request_id.clone(),
                resource_version: saved.rule.version,
                replayed: saved.replayed,
                original_request_id: saved.original_request_id,
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn get_automation_rule(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, auto_wire::GetAutomationRuleRequest>,
    ) -> ServiceResult<auto_wire::GetAutomationRuleResponse> {
        let input = req.to_owned_message();
        let id = automation::rule_id(&input.rule_id)?;
        let db = self.d1()?;
        let rule = automation::get(&db, &self.actor.tenant, id.as_str())
            .await?
            .ok_or_else(|| ConnectError::not_found("ACTIVITIES_AUTOMATION_RULE_NOT_FOUND"))?;
        let confirmed_mutation = match receipt_key(input.receipt_idempotency_key.as_deref())? {
            None => None,
            Some(key) => {
                let stored =
                    crud_receipt(&db, &self.actor.tenant, key, CrudReceiptOwner::Automation)
                        .await
                        .map_err(|_| ConnectError::unavailable("AUTOMATION_RECEIPT_READ_FAILED"))?;
                crud_commit_evidence(stored, id.as_str(), &self.request_id, |state| {
                    let saved: automation::RuleRow = serde_json::from_str(state).map_err(|_| {
                        worker::Error::RustError("Automation receipt invalid".into())
                    })?;
                    Ok((saved.rule_id, saved.version))
                })
                .map_err(|_| ConnectError::unavailable("AUTOMATION_RECEIPT_DECODE_FAILED"))?
            }
        };
        RpcResponse::ok(auto_wire::GetAutomationRuleResponse {
            rule: Some(Self::rule(&rule)?).into(),
            confirmed_mutation: confirmed_mutation.into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn list_automation_rules(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, auto_wire::ListAutomationRulesRequest>,
    ) -> ServiceResult<auto_wire::ListAutomationRulesResponse> {
        let size = if req.page.page_size == 0 {
            25
        } else {
            req.page.page_size
        };
        if size > 100 {
            return Err(ConnectError::invalid_argument("page_size exceeds 100"));
        }
        let after = req.page.page_token.as_str();
        if !after.is_empty() {
            automation::rule_id(after)?;
        }
        let mut rows =
            automation::list_rules(&self.d1()?, &self.actor.tenant, after, size + 1).await?;
        let next = if rows.len() > size as usize {
            rows.truncate(size as usize);
            rows.last()
                .map(|row| row.rule_id.clone())
                .unwrap_or_default()
        } else {
            String::new()
        };
        let rules = rows
            .iter()
            .map(Self::rule)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        RpcResponse::ok(auto_wire::ListAutomationRulesResponse {
            rules,
            page_info: Some(core_v2::PageInfo {
                next_page_token: next,
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn run_interaction_automation(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, auto_wire::RunInteractionAutomationRequest>,
    ) -> ServiceResult<auto_wire::RunInteractionAutomationResponse> {
        let input = req.to_owned_message();
        let (key, _) = wire_precondition(input.precondition.as_option(), true)?;
        let saved = automation::execute_interaction(
            &self.d1()?,
            &self.actor,
            &input.rule_id,
            &input.interaction_id,
            input.expected_rule_version,
            &key,
            &self.request_id,
        )
        .await?;
        RpcResponse::ok(auto_wire::RunInteractionAutomationResponse {
            run: Some(Self::run(&saved.run)?).into(),
            receipt: Some(core_v2::MutationReceipt {
                request_id: self.request_id.clone(),
                resource_version: 1,
                replayed: saved.replayed,
                original_request_id: saved.task_original_request_id,
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn get_automation_run(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, auto_wire::GetAutomationRunRequest>,
    ) -> ServiceResult<auto_wire::GetAutomationRunResponse> {
        let input = req.to_owned_message();
        let db = self.d1()?;
        let run = automation::get_run(
            &db,
            &self.actor.tenant,
            &input.rule_id,
            &input.interaction_id,
        )
        .await?
        .ok_or_else(|| ConnectError::not_found("ACTIVITIES_AUTOMATION_RUN_NOT_FOUND"))?;
        let confirmed_mutation = match receipt_key(input.receipt_idempotency_key.as_deref())? {
            None => None,
            Some(key) if automation::confirmed(&db, &self.actor.tenant, &run, key).await? => {
                Some(core_v2::MutationCommitEvidence {
                    request_id: self.request_id.clone(),
                    original_request_id: run.task_request_id.clone().unwrap_or_default(),
                    resource_version: 1,
                    ..Default::default()
                })
            }
            Some(_) => None,
        };
        RpcResponse::ok(auto_wire::GetAutomationRunResponse {
            run: Some(Self::run(&run)?).into(),
            confirmed_mutation: confirmed_mutation.into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn list_automation_runs(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, auto_wire::ListAutomationRunsRequest>,
    ) -> ServiceResult<auto_wire::ListAutomationRunsResponse> {
        let input = req.to_owned_message();
        if !input.rule_id.is_empty() {
            automation::rule_id(&input.rule_id)?;
        }
        let after = if input.page.page_token.is_empty() {
            0
        } else {
            input
                .page
                .page_token
                .parse::<i64>()
                .map_err(|_| ConnectError::invalid_argument("INVALID_AUTOMATION_CURSOR"))?
        };
        if after < 0 {
            return Err(ConnectError::invalid_argument("INVALID_AUTOMATION_CURSOR"));
        }
        let size = if input.page.page_size == 0 {
            25
        } else {
            input.page.page_size
        };
        if size > 100 {
            return Err(ConnectError::invalid_argument("page_size exceeds 100"));
        }
        let mut rows = automation::list_runs(
            &self.d1()?,
            &self.actor.tenant,
            &input.rule_id,
            after,
            size + 1,
        )
        .await?;
        let next = if rows.len() > size as usize {
            rows.truncate(size as usize);
            rows.last()
                .map(|run| run.seq.to_string())
                .unwrap_or_default()
        } else {
            String::new()
        };
        let runs = rows
            .iter()
            .map(Self::run)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        RpcResponse::ok(auto_wire::ListAutomationRunsResponse {
            runs,
            page_info: Some(core_v2::PageInfo {
                next_page_token: next,
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }
}

impl AutomationV2Service {
    fn d1(&self) -> std::result::Result<D1Database, ConnectError> {
        self.env
            .d1("DB")
            .map_err(|_| ConnectError::unavailable("D1 unavailable"))
    }
    fn rule(
        row: &automation::RuleRow,
    ) -> std::result::Result<auto_wire::AutomationRule, ConnectError> {
        let trigger = match row.trigger.as_str() {
            "interaction_created" => {
                auto_wire::AutomationTrigger::AUTOMATION_TRIGGER_INTERACTION_CREATED
            }
            _ => {
                return Err(ConnectError::internal(
                    "ACTIVITIES_AUTOMATION_TRIGGER_INVALID",
                ));
            }
        };
        Ok(auto_wire::AutomationRule {
            rule_id: row.rule_id.clone(),
            version: row.version,
            trigger: trigger.into(),
            enabled: row.enabled == 1,
            task_subject: row.task_subject.clone(),
            assignee_email: row.assignee_email.clone(),
            due_after_seconds: row.due_after_seconds,
            ..Default::default()
        })
    }
    fn run(
        row: &automation::RunRow,
    ) -> std::result::Result<auto_wire::AutomationRun, ConnectError> {
        let status = match row.status.as_str() {
            "pending" => auto_wire::AutomationRunStatus::AUTOMATION_RUN_STATUS_PENDING,
            "committed" => auto_wire::AutomationRunStatus::AUTOMATION_RUN_STATUS_COMMITTED,
            _ => {
                return Err(ConnectError::internal(
                    "ACTIVITIES_AUTOMATION_RUN_STATUS_INVALID",
                ));
            }
        };
        Ok(auto_wire::AutomationRun {
            rule_id: row.rule_id.clone(),
            rule_version: row.rule_version,
            source_event_id: row.source_event_id.clone(),
            interaction_id: row.interaction_id.clone(),
            task_id: row.task_id.clone(),
            status: status.into(),
            requested_by: row.requested_by.clone(),
            original_request_id: row.original_request_id.clone(),
            task_request_id: row.task_request_id.clone().unwrap_or_default(),
            created_at_unix_millis: row.created_at_unix_millis,
            completed_at_unix_millis: row.completed_at_unix_millis.unwrap_or_default(),
            ..Default::default()
        })
    }
}
