//! One generated Connect service for ORIGINAL crm.activities automation.
//! Policy and tenant come only from the central signed-Access router.
//! One typed rule service for Deal/Task scheduled sources and retained signed
//! manual Interaction→Task execution.
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
        let trigger = match input.trigger {
            buffa::EnumValue::Known(auto_wire::AutomationTrigger::AUTOMATION_TRIGGER_INTERACTION_CREATED) => {
                "interaction_created"
            }
            buffa::EnumValue::Known(auto_wire::AutomationTrigger::AUTOMATION_TRIGGER_TASK_OVERDUE) => "task_overdue",
            buffa::EnumValue::Known(auto_wire::AutomationTrigger::AUTOMATION_TRIGGER_DEAL_CREATED) => "deal_created",
            _ => {
                return Err(ConnectError::invalid_argument(
                    "ACTIVITIES_AUTOMATION_TRIGGER_NOT_ENABLED",
                ));
            }
        };
        let row = automation::RuleInput {
            rule_id: input.rule_id,
            trigger: trigger.into(),
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
        let after = req.page.page_token;
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
                original_request_id: saved.original_request_id,
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn cancel_automation_run(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, auto_wire::CancelAutomationRunRequest>,
    ) -> ServiceResult<auto_wire::CancelAutomationRunResponse> {
        let input = req.to_owned_message();
        let (key, _) = wire_precondition(input.precondition.as_option(), true)?;
        let (source_type, source_id) =
            Self::source_identity(input.source_type, &input.source_id, &input.interaction_id)?;
        let saved = automation::cancel(
            &self.d1()?,
            &self.actor,
            &input.rule_id,
            &source_type,
            &source_id,
            input.expected_rule_version,
            &key,
            &self.request_id,
        )
        .await?;
        RpcResponse::ok(auto_wire::CancelAutomationRunResponse {
            run: Some(Self::run(&saved.run)?).into(),
            receipt: Some(core_v2::MutationReceipt {
                request_id: self.request_id.clone(),
                resource_version: 1,
                replayed: saved.replayed,
                original_request_id: saved.original_request_id,
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
        let (source_type, source_id) =
            Self::source_identity(input.source_type, &input.source_id, &input.interaction_id)?;
        let run = automation::get_run(
            &db,
            &self.actor.tenant,
            &input.rule_id,
            &source_type,
            &source_id,
            input.expected_rule_version,
        )
        .await?
        .ok_or_else(|| ConnectError::not_found("ACTIVITIES_AUTOMATION_RUN_NOT_FOUND"))?;
        let confirmed_mutation = match receipt_key(input.receipt_idempotency_key.as_deref())? {
            None => None,
            Some(key) => automation::confirmed_request(&db, &self.actor.tenant, &run, key)
                .await?
                .map(|original_request_id| core_v2::MutationCommitEvidence {
                    request_id: self.request_id.clone(),
                    original_request_id,
                    resource_version: 1,
                    ..Default::default()
                }),
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
    fn source_identity(
        source_type: buffa::EnumValue<auto_wire::AutomationTrigger>,
        source_id: &str,
        legacy_interaction_id: &str,
    ) -> std::result::Result<(String, String), ConnectError> {
        if source_id.is_empty() {
            if legacy_interaction_id.is_empty() {
                return Err(ConnectError::invalid_argument(
                    "ACTIVITIES_AUTOMATION_SOURCE_ID_REQUIRED",
                ));
            }
            return Ok((
                "interaction_created".into(),
                legacy_interaction_id.to_owned(),
            ));
        }
        let kind = match source_type {
            buffa::EnumValue::Known(auto_wire::AutomationTrigger::AUTOMATION_TRIGGER_INTERACTION_CREATED) => {
                "interaction_created"
            }
            buffa::EnumValue::Known(auto_wire::AutomationTrigger::AUTOMATION_TRIGGER_TASK_OVERDUE) => "task_overdue",
            buffa::EnumValue::Known(auto_wire::AutomationTrigger::AUTOMATION_TRIGGER_DEAL_CREATED) => "deal_created",
            _ => {
                return Err(ConnectError::invalid_argument(
                    "ACTIVITIES_AUTOMATION_SOURCE_TYPE_REQUIRED",
                ));
            }
        };
        Ok((kind.into(), source_id.to_owned()))
    }
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
            "task_overdue" => auto_wire::AutomationTrigger::AUTOMATION_TRIGGER_TASK_OVERDUE,
            "deal_created" => auto_wire::AutomationTrigger::AUTOMATION_TRIGGER_DEAL_CREATED,
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
            "pending" if row.cancel_request_id.is_some() => {
                auto_wire::AutomationRunStatus::AUTOMATION_RUN_STATUS_CANCELLED
            }
            "pending" => auto_wire::AutomationRunStatus::AUTOMATION_RUN_STATUS_PENDING,
            "committed" => auto_wire::AutomationRunStatus::AUTOMATION_RUN_STATUS_COMMITTED,
            _ => {
                return Err(ConnectError::internal(
                    "ACTIVITIES_AUTOMATION_RUN_STATUS_INVALID",
                ));
            }
        };
        use auto_wire::AutomationRunDisposition as WireDisposition;
        use automation::RunDisposition;
        let disposition = match row.disposition()? {
            RunDisposition::Committed => WireDisposition::AUTOMATION_RUN_DISPOSITION_COMMITTED,
            RunDisposition::RetryReady => WireDisposition::AUTOMATION_RUN_DISPOSITION_RETRY_READY,
            RunDisposition::WaitingAssignee => {
                WireDisposition::AUTOMATION_RUN_DISPOSITION_WAITING_ASSIGNEE
            }
            RunDisposition::RuleSuperseded => {
                WireDisposition::AUTOMATION_RUN_DISPOSITION_RULE_SUPERSEDED
            }
            RunDisposition::ReconcileReceipt => {
                WireDisposition::AUTOMATION_RUN_DISPOSITION_RECONCILE_RECEIPT
            }
            RunDisposition::Executing => WireDisposition::AUTOMATION_RUN_DISPOSITION_EXECUTING,
            RunDisposition::Backoff => WireDisposition::AUTOMATION_RUN_DISPOSITION_BACKOFF,
            RunDisposition::AttemptsExhausted => {
                WireDisposition::AUTOMATION_RUN_DISPOSITION_ATTEMPTS_EXHAUSTED
            }
            RunDisposition::Cancelled => WireDisposition::AUTOMATION_RUN_DISPOSITION_CANCELLED,
            RunDisposition::SourceStale => WireDisposition::AUTOMATION_RUN_DISPOSITION_SOURCE_STALE,
        };
        let source_type = match row.source_type.as_str() {
            "interaction_created" => {
                auto_wire::AutomationTrigger::AUTOMATION_TRIGGER_INTERACTION_CREATED
            }
            "task_overdue" => auto_wire::AutomationTrigger::AUTOMATION_TRIGGER_TASK_OVERDUE,
            "deal_created" => auto_wire::AutomationTrigger::AUTOMATION_TRIGGER_DEAL_CREATED,
            _ => {
                return Err(ConnectError::internal(
                    "ACTIVITIES_AUTOMATION_TRIGGER_INVALID",
                ));
            }
        };
        Ok(auto_wire::AutomationRun {
            rule_id: row.rule_id.clone(),
            rule_version: row.rule_version,
            source_event_id: row.source_event_id.clone(),
            interaction_id: row.interaction_id.clone(),
            source_type: source_type.into(),
            source_id: row.source_id.clone(),
            source_version: row.source_version,
            task_id: row.task_id.clone(),
            status: status.into(),
            requested_by: row.requested_by.clone(),
            original_request_id: row.original_request_id.clone(),
            task_request_id: row.task_request_id.clone().unwrap_or_default(),
            created_at_unix_millis: row.created_at_unix_millis,
            completed_at_unix_millis: row.completed_at_unix_millis.unwrap_or_default(),
            disposition: disposition.into(),
            attempt_count: row.attempt_count,
            attempt_limit: automation::MAX_ATTEMPTS,
            next_attempt_at_unix_millis: row.next_attempt_at_unix_millis,
            lease_until_unix_millis: row.lease_until_unix_millis,
            can_cancel: row.can_cancel(),
            cancelled_by: row.cancelled_by.clone().unwrap_or_default(),
            cancel_request_id: row.cancel_request_id.clone().unwrap_or_default(),
            cancelled_at_unix_millis: row.cancelled_at_unix_millis.unwrap_or_default(),
            ..Default::default()
        })
    }
}
