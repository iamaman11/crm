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
        let id = Self::valid_id(&req.task_id)?;
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
        let account = req.account_id.as_deref();
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
