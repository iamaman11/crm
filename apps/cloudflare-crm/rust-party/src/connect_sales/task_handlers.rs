//! Generated Connect trait for the existing TaskV2Service Rust domain owner.
//! Access, tenant, and the single native router are defined by the parent.
use super::*;

impl task_wire::TaskService for TaskV2Service {
    #[worker::send]
    async fn claim_task(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, task_wire::ClaimTaskRequest>,
    ) -> ServiceResult<task_wire::ClaimTaskResponse> {
        let input = req.to_owned_message();
        let id = Self::valid_id(&input.task_id)?;
        let (key, expected) = wire_precondition(input.precondition.as_option(), false)?;
        let saved = activity_tasks::claim_typed(
            expected.unwrap_or_default(),
            &key,
            &self.d1()?,
            &id,
            &self.actor,
            &self.request_id,
        )
        .await
        .map_err(|_| ConnectError::unavailable("ACTIVITIES_CLAIM_FAILED"))?
        .map_err(|e| e.connect("ClaimTask", &self.request_id))?;
        RpcResponse::ok(task_wire::ClaimTaskResponse {
            task: Some(Self::convert(
                &saved.task,
                &self.persisted_account(&id).await?,
            )?)
            .into(),
            receipt: Some(self.receipt(&saved)).into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn create_task(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, task_wire::CreateTaskRequest>,
    ) -> ServiceResult<task_wire::CreateTaskResponse> {
        let input = req.to_owned_message();
        let id = Self::valid_id(&input.task_id)?;
        let (key, _) = wire_precondition(input.precondition.as_option(), true)?;
        let priority = match input.priority {
            x if x == task_wire::TaskPriority::TASK_PRIORITY_LOW => activity_tasks::Priority::Low,
            x if x == task_wire::TaskPriority::TASK_PRIORITY_NORMAL => {
                activity_tasks::Priority::Normal
            }
            x if x == task_wire::TaskPriority::TASK_PRIORITY_HIGH => activity_tasks::Priority::High,
            x if x == task_wire::TaskPriority::TASK_PRIORITY_URGENT => {
                activity_tasks::Priority::Urgent
            }
            _ => return Err(ConnectError::invalid_argument("invalid TaskPriority")),
        };
        let due = input
            .due_at
            .as_option()
            .map(ContactPointV2Service::nanos)
            .transpose()?;
        let reminder = input
            .reminder_at
            .as_option()
            .map(ContactPointV2Service::nanos)
            .transpose()?;
        let saved = activity_tasks::create_typed(
            activity_tasks::CreateInput {
                id,
                subject: input.subject,
                description: input.description,
                account_id: input.account_id,
                deal_id: input.deal_id,
                assignee_email: input.assignee_email,
                owner_team_id: input.owner_team_id,
                priority,
                due_at_unix_nanos: due,
                reminder_at_unix_nanos: reminder,
            },
            &key,
            &self.d1()?,
            &self.actor.tenant,
            &self.actor.identity,
            &self.request_id,
        )
        .await
        .map_err(|_| ConnectError::internal("Task D1 create failed"))?
        .map_err(|failure| failure.connect("CreateTask", &self.request_id))?;
        RpcResponse::ok(task_wire::CreateTaskResponse {
            task: Some(Self::convert(
                &saved.task,
                &self
                    .persisted_account(saved.task.task_id().as_str())
                    .await?,
            )?)
            .into(),
            receipt: Some(self.receipt(&saved)).into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn update_task(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, task_wire::UpdateTaskRequest>,
    ) -> ServiceResult<task_wire::UpdateTaskResponse> {
        use activity_tasks::Change;
        let input = req.to_owned_message();
        let id = Self::valid_id(&input.task_id)?;
        let (key, version) = wire_precondition(input.precondition.as_option(), false)?;
        let string_patch = |patch: Option<&core_v2::StringPatch>| -> std::result::Result<Change<String>, ConnectError> {
            let Some(patch) = patch else { return Ok(Change::Keep); };
            match patch.operation.as_ref() {
                Some(core_v2::string_patch::Operation::Set(value)) => Ok(Change::Set(value.clone())),
                Some(core_v2::string_patch::Operation::Clear(true)) => Ok(Change::Clear),
                _ => Err(ConnectError::invalid_argument("invalid string patch")),
            }
        };
        let subject = string_patch(input.subject.as_option())?;
        let description = string_patch(input.description.as_option())?;
        // The same typed optional enum already serves ListTasks. Presence is
        // Keep/Set; Clear cannot apply to this mandatory domain property.
        let priority = match input.priority {
            None => Change::Keep,
            Some(x) if x == task_wire::TaskPriority::TASK_PRIORITY_LOW => {
                Change::Set(activity_tasks::Priority::Low)
            }
            Some(x) if x == task_wire::TaskPriority::TASK_PRIORITY_NORMAL => {
                Change::Set(activity_tasks::Priority::Normal)
            }
            Some(x) if x == task_wire::TaskPriority::TASK_PRIORITY_HIGH => {
                Change::Set(activity_tasks::Priority::High)
            }
            Some(x) if x == task_wire::TaskPriority::TASK_PRIORITY_URGENT => {
                Change::Set(activity_tasks::Priority::Urgent)
            }
            _ => return Err(ConnectError::invalid_argument("invalid TaskPriority patch")),
        };
        let due_at_unix_nanos = match input.due_at.as_option() {
            None => Change::Keep,
            Some(patch) => match patch.operation.as_ref() {
                Some(core_v2::timestamp_patch::Operation::Set(value)) => {
                    Change::Set(ContactPointV2Service::nanos(value)?)
                }
                Some(core_v2::timestamp_patch::Operation::Clear(true)) => Change::Clear,
                _ => return Err(ConnectError::invalid_argument("invalid due_at patch")),
            },
        };
        let owner = match input.assignee_email {
            None => Change::Keep,
            Some(email) => Change::Set(email),
        };
        let saved = activity_tasks::update_typed(
            activity_tasks::UpdateInput {
                expected_version: version.unwrap_or_default(),
                subject,
                description,
                owner,
                owner_team_id: input.owner_team_id,
                priority,
                due_at_unix_nanos,
            },
            &key,
            &self.d1()?,
            &self.actor.tenant,
            &id,
            &self.actor.identity,
            &self.request_id,
        )
        .await
        .map_err(|_| ConnectError::internal("Task D1 update failed"))?
        .map_err(|failure| failure.connect("UpdateTask", &self.request_id))?;
        RpcResponse::ok(task_wire::UpdateTaskResponse {
            task: Some(Self::convert(
                &saved.task,
                &self
                    .persisted_account(saved.task.task_id().as_str())
                    .await?,
            )?)
            .into(),
            receipt: Some(self.receipt(&saved)).into(),
            ..Default::default()
        })
    }
    // Existing signed Task owner, original domain CAS and receipt/audit trigger.
    #[worker::send]
    async fn schedule_reminder(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, task_wire::ScheduleReminderRequest>,
    ) -> ServiceResult<task_wire::ScheduleReminderResponse> {
        let input = req.to_owned_message();
        let id = Self::valid_id(&input.task_id)?;
        let (key, version) = wire_precondition(input.precondition.as_option(), false)?;
        let reminder_at = input
            .reminder_at
            .as_option()
            .ok_or_else(|| ConnectError::invalid_argument("reminder_at required"))?;
        let reminder_at_unix_nanos = ContactPointV2Service::nanos(reminder_at)?;
        let saved = activity_tasks::schedule_reminder_typed(
            activity_tasks::ReminderInput {
                expected_version: version.unwrap_or_default(),
                reminder_at_unix_nanos,
            },
            &key,
            &self.d1()?,
            &id,
            &self.actor,
            &self.request_id,
        )
        .await
        .map_err(|_| ConnectError::internal("Task D1 reminder failed"))?
        .map_err(|failure| failure.connect("ScheduleReminder", &self.request_id))?;
        RpcResponse::ok(task_wire::ScheduleReminderResponse {
            task: Some(Self::convert(
                &saved.task,
                &self
                    .persisted_account(saved.task.task_id().as_str())
                    .await?,
            )?)
            .into(),
            changed: saved.changed,
            receipt: if saved.changed || saved.replayed {
                Some(self.receipt(&saved)).into()
            } else {
                None.into()
            },
            ..Default::default()
        })
    }
    #[worker::send]
    async fn complete_task(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, task_wire::CompleteTaskRequest>,
    ) -> ServiceResult<task_wire::CompleteTaskResponse> {
        let input = req.to_owned_message();
        let id = Self::valid_id(&input.task_id)?;
        let (key, version) = wire_precondition(input.precondition.as_option(), false)?;
        let saved = activity_tasks::complete_typed(
            activity_tasks::CompleteInput {
                expected_version: version.unwrap_or_default(),
            },
            &key,
            &self.d1()?,
            &id,
            &self.actor,
            &self.request_id,
        )
        .await
        .map_err(|_| ConnectError::internal("Task D1 complete failed"))?
        .map_err(|failure| failure.connect("CompleteTask", &self.request_id))?;
        RpcResponse::ok(task_wire::CompleteTaskResponse {
            task: Some(Self::convert(
                &saved.task,
                &self
                    .persisted_account(saved.task.task_id().as_str())
                    .await?,
            )?)
            .into(),
            changed: saved.changed,
            receipt: if saved.changed || saved.replayed {
                Some(self.receipt(&saved)).into()
            } else {
                None.into()
            },
            ..Default::default()
        })
    }
    #[worker::send]
    async fn get_task(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, task_wire::GetTaskRequest>,
    ) -> ServiceResult<task_wire::GetTaskResponse> {
        let input = req.to_owned_message();
        let id = Self::valid_id(&input.task_id)?;
        let db = self.d1()?;
        let item = activity_tasks::stored(&db, &self.actor.tenant, &id)
            .await
            .map_err(|_| ConnectError::internal("TASK original D1 read failed"))?
            .ok_or_else(|| ConnectError::not_found("ACTIVITIES_TASK_NOT_FOUND"))?;
        let confirmed_mutation = match receipt_key(input.receipt_idempotency_key.as_deref())? {
            None => None,
            Some(key) => {
                let stored = crud_receipt(&db, &self.actor.tenant, key, CrudReceiptOwner::Task)
                    .await
                    .map_err(|_| ConnectError::unavailable("TASK_RECEIPT_READ_FAILED"))?;
                crud_commit_evidence(stored, &id, &self.request_id, |state| {
                    activity_tasks::decode(state)
                        .map(|row| (row.task_id().as_str().to_owned(), row.version()))
                })
                .map_err(|_| ConnectError::unavailable("TASK_RECEIPT_DECODE_FAILED"))?
            }
        };
        RpcResponse::ok(task_wire::GetTaskResponse {
            task: Some(Self::convert(&item, &self.persisted_account(&id).await?)?).into(),
            confirmed_mutation: confirmed_mutation.into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn list_tasks(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, task_wire::ListTasksRequest>,
    ) -> ServiceResult<task_wire::ListTasksResponse> {
        let size = if req.page.page_size == 0 {
            25
        } else {
            req.page.page_size
        };
        if size > 100 {
            return Err(ConnectError::invalid_argument("page_size exceeds 100"));
        }
        let after = &req.page.page_token;
        if !after.is_empty() {
            if req.sort_by_due {
                let (time, id) = after
                    .split_once('|')
                    .ok_or_else(|| ConnectError::invalid_argument("INVALID_TASK_DUE_CURSOR"))?;
                if time
                    .parse::<i64>()
                    .ok()
                    .filter(|value| *value >= 0)
                    .is_none()
                    || Self::valid_id(id).is_err()
                {
                    return Err(ConnectError::invalid_argument("INVALID_TASK_DUE_CURSOR"));
                }
            } else {
                Self::valid_id(after)?;
            }
        }
        // Generated ServiceRequest borrows TimestampView; use the standard
        // generated owned message for the existing UtcInstant validator.
        let owned = req.to_owned_message();
        let due_from = owned
            .due_from
            .as_option()
            .map(ContactPointV2Service::nanos)
            .transpose()?;
        let due_before = owned
            .due_before
            .as_option()
            .map(ContactPointV2Service::nanos)
            .transpose()?;
        if due_from.is_some_and(|value| value < 0)
            || due_before.is_some_and(|value| value < 0)
            || due_from
                .zip(due_before)
                .is_some_and(|(from, before)| from >= before)
        {
            return Err(ConnectError::invalid_argument("INVALID_TASK_DUE_RANGE"));
        }
        if req.without_due_only
            && (due_from.is_some() || due_before.is_some() || req.sort_by_due || req.overdue_only)
        {
            return Err(ConnectError::invalid_argument("INVALID_TASK_DUE_FILTERS"));
        }
        let status = match req.status {
            None => None,
            Some(x) if x == task_wire::TaskStatus::TASK_STATUS_OPEN => Some(DomainTaskStatus::Open),
            Some(x) if x == task_wire::TaskStatus::TASK_STATUS_COMPLETED => {
                Some(DomainTaskStatus::Completed)
            }
            _ => return Err(ConnectError::invalid_argument("invalid TaskStatus filter")),
        };
        let account = req.account_id;
        let priority = match req.priority {
            None => None,
            Some(x) if x == task_wire::TaskPriority::TASK_PRIORITY_LOW => {
                Some(DomainTaskPriority::Low)
            }
            Some(x) if x == task_wire::TaskPriority::TASK_PRIORITY_NORMAL => {
                Some(DomainTaskPriority::Normal)
            }
            Some(x) if x == task_wire::TaskPriority::TASK_PRIORITY_HIGH => {
                Some(DomainTaskPriority::High)
            }
            Some(x) if x == task_wire::TaskPriority::TASK_PRIORITY_URGENT => {
                Some(DomainTaskPriority::Urgent)
            }
            _ => {
                return Err(ConnectError::invalid_argument(
                    "invalid TaskPriority filter",
                ));
            }
        };
        if let Some(id) = account {
            crm_module_sdk::RecordId::try_new(id.to_owned())
                .map_err(|_| ConnectError::invalid_argument("invalid Account filter"))?;
        }
        let deal = req.deal_id;
        if let Some(id) = deal {
            Self::valid_id(id)?;
        }
        let search = wire_search(req.search)?;
        if (req.assigned_to_me && (req.assignee_email.is_some() || req.owner_team_id.is_some()))
            || (req.assignee_email.is_some() && req.owner_team_id.is_some())
        {
            return Err(ConnectError::invalid_argument("INVALID_ASSIGNEE_FILTERS"));
        }
        if let Some(team) = req.owner_team_id {
            access_teams::id(team).map_err(|e| e.connect("ListTasks", &self.request_id))?;
        }
        let owner = if req.assigned_to_me {
            Some(self.actor.identity.clone())
        } else {
            req.assignee_email
                .map(|v| {
                    access_members::email(v).map_err(|e| e.connect("ListTasks", &self.request_id))
                })
                .transpose()?
        };
        let (list, next) = activity_tasks::list_page_typed(
            &self.d1()?,
            &self.actor.tenant,
            after,
            size as usize,
            activity_tasks::TaskListFilters {
                status,
                account_id: account,
                priority,
                deal_id: deal,
                search,
                overdue_only: req.overdue_only,
                due_from,
                due_before,
                sort_by_due: req.sort_by_due,
                without_due_only: req.without_due_only,
                owner_identity: owner.as_deref(),
                owner_team_id: req.owner_team_id,
            },
        )
        .await
        .map_err(|_| ConnectError::internal("Task D1 list failed"))?;
        let mut response = task_wire::ListTasksResponse::default();
        for (task, account_id, name) in &list {
            response
                .account_names
                .insert(account_id.clone(), name.clone());
            response.tasks.push(Self::convert(task, account_id)?);
        }
        RpcResponse::ok(task_wire::ListTasksResponse {
            page_info: Some(core_v2::PageInfo {
                next_page_token: next.unwrap_or_default(),
                ..Default::default()
            })
            .into(),
            ..response
        })
    }
    #[worker::send]
    async fn list_task_workload(
        &self,
        _ctx: RequestContext,
        _req: ServiceRequest<'_, task_wire::ListTaskWorkloadRequest>,
    ) -> ServiceResult<task_wire::ListTaskWorkloadResponse> {
        let (buckets, as_of_unix_seconds) =
            activity_tasks::workload(&self.d1()?, &self.actor.tenant)
                .await
                .map_err(|_| ConnectError::internal("Task workload D1 failed"))?;
        let as_count = |n: i64| {
            u64::try_from(n).map_err(|_| ConnectError::internal("Invalid Task workload count"))
        };
        let mut converted = Vec::with_capacity(buckets.len());
        for b in buckets {
            converted.push(task_wire::TaskWorkloadBucket {
                assignee_identity: b.assignee_identity,
                owner_team_id: b.owner_team_id,
                open_count: as_count(b.open_count)?,
                completed_count: as_count(b.completed_count)?,
                overdue_open_count: as_count(b.overdue_open_count)?,
                due_next_7_days_count: as_count(b.due_next_7_days_count)?,
                ..Default::default()
            });
        }
        RpcResponse::ok(task_wire::ListTaskWorkloadResponse {
            buckets: converted,
            as_of_unix_seconds,
            ..Default::default()
        })
    }
}
