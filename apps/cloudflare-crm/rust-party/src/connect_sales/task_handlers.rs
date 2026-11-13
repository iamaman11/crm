//! Generated Connect trait for the existing TaskV2Service Rust domain owner.
//! Access, tenant, and the single native router are defined by the parent.
use super::*;

impl task_wire::TaskService for TaskV2Service {
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
                priority,
                due_at_unix_nanos: due,
                reminder_at_unix_nanos: reminder,
            },
            &key,
            &self.d1()?,
            &self.tenant,
            &self.actor,
            &self.request_id,
        )
        .await
        .map_err(|_| ConnectError::internal("Task D1 create failed"))?
        .map_err(Self::failed)?;
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
        let saved = activity_tasks::update_typed(
            activity_tasks::UpdateInput {
                expected_version: version.unwrap_or_default(),
                subject,
                description,
                priority,
                due_at_unix_nanos,
            },
            &key,
            &self.d1()?,
            &self.tenant,
            &id,
            &self.request_id,
        )
        .await
        .map_err(|_| ConnectError::internal("Task D1 update failed"))?
        .map_err(Self::failed)?;
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
            &self.tenant,
            &id,
            &self.request_id,
        )
        .await
        .map_err(|_| ConnectError::internal("Task D1 complete failed"))?
        .map_err(Self::failed)?;
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
        let id = Self::valid_id(req.task_id)?;
        let task = activity_tasks::stored(&self.d1()?, &self.tenant, &id)
            .await
            .map_err(|_| ConnectError::internal("Task D1 get failed"))?
            .ok_or_else(|| ConnectError::not_found("ACTIVITIES_TASK_NOT_FOUND"))?;
        RpcResponse::ok(task_wire::GetTaskResponse {
            task: Some(Self::convert(&task, &self.persisted_account(&id).await?)?).into(),
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
            Self::valid_id(after)?;
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
        let (list, next) = activity_tasks::list_page_typed(
            &self.d1()?,
            &self.tenant,
            after,
            size as usize,
            status,
            account,
            priority,
        )
        .await
        .map_err(|_| ConnectError::internal("Task D1 list failed"))?;
        let mut tasks = Vec::with_capacity(list.len());
        for (task, account_id) in &list {
            tasks.push(Self::convert(task, account_id)?);
        }
        RpcResponse::ok(task_wire::ListTasksResponse {
            tasks,
            page_info: Some(core_v2::PageInfo {
                next_page_token: next.unwrap_or_default(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }
}
