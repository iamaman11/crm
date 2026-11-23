//! Generated Connect trait for the existing SalesV2Service Rust domain owner.
//! Access, tenant, and the single native router are defined by the parent.
use super::*;
use crate::diagnostics;

impl sale_wire::DealService for SalesV2Service {
    #[worker::send]
    async fn create_territory(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::CreateTerritoryRequest>,
    ) -> ServiceResult<sale_wire::CreateTerritoryResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.territory_id)?;
        let (key, expected) = wire_precondition(input.precondition.as_option(), true)?;
        let cfg = input.configuration.as_option().ok_or_else(|| {
            ConnectError::invalid_argument("SALES_COVERAGE_CONFIGURATION_REQUIRED")
        })?;
        let record = Self::territory_draft(id, cfg)?;
        let db = self.d1()?;
        let saved = sales_territories::mutate(
            &sales_territories::Context {
                db: &db,
                tenant: &self.tenant,
                actor: &self.actor,
                request_id: &self.request_id,
            },
            record,
            &key,
            expected,
        )
        .await
        .map_err(|_| ConnectError::unavailable("SALES_COVERAGE_WRITE_FAILED"))?
        .map_err(|failure| failure.connect("CreateTerritory", &self.request_id))?;
        let receipt = self.coverage_receipt(&saved);
        RpcResponse::ok(sale_wire::CreateTerritoryResponse {
            territory: Some(Self::territory_wire(saved.record)?).into(),
            receipt: Some(receipt).into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn update_territory(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::UpdateTerritoryRequest>,
    ) -> ServiceResult<sale_wire::UpdateTerritoryResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.territory_id)?;
        let (key, expected) = wire_precondition(input.precondition.as_option(), false)?;
        let cfg = input.configuration.as_option().ok_or_else(|| {
            ConnectError::invalid_argument("SALES_COVERAGE_CONFIGURATION_REQUIRED")
        })?;
        let record = Self::territory_draft(id, cfg)?;
        let db = self.d1()?;
        let saved = sales_territories::mutate(
            &sales_territories::Context {
                db: &db,
                tenant: &self.tenant,
                actor: &self.actor,
                request_id: &self.request_id,
            },
            record,
            &key,
            expected,
        )
        .await
        .map_err(|_| ConnectError::unavailable("SALES_COVERAGE_WRITE_FAILED"))?
        .map_err(|failure| failure.connect("UpdateTerritory", &self.request_id))?;
        let receipt = self.coverage_receipt(&saved);
        RpcResponse::ok(sale_wire::UpdateTerritoryResponse {
            territory: Some(Self::territory_wire(saved.record)?).into(),
            receipt: Some(receipt).into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn get_territory(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::GetTerritoryRequest>,
    ) -> ServiceResult<sale_wire::GetTerritoryResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.territory_id)?;
        let db = self.d1()?;
        let kind = sales_territories::Kind::Territory;
        let record = sales_territories::get(&db, &self.tenant, kind, &id)
            .await
            .map_err(|_| ConnectError::internal("Sales coverage read failed"))?;
        let record = record.ok_or_else(|| ConnectError::not_found("SALES_TERRITORY_NOT_FOUND"))?;
        let confirmed_mutation = match receipt_key(input.receipt_idempotency_key.as_deref())? {
            None => None,
            Some(key) => {
                let row = crud_receipt(&db, &self.tenant, key, CrudReceiptOwner::Territory)
                    .await
                    .map_err(|_| ConnectError::unavailable("SALES_COVERAGE_RECEIPT_READ_FAILED"))?;
                crud_commit_evidence(row, &id, &self.request_id, |state| {
                    let record = kind.decode(state)?;
                    Ok((record.id().to_owned(), record.version()))
                })
                .map_err(|_| ConnectError::unavailable("SALES_COVERAGE_RECEIPT_DECODE_FAILED"))?
            }
        };
        RpcResponse::ok(sale_wire::GetTerritoryResponse {
            territory: Some(Self::territory_wire(record)?).into(),
            confirmed_mutation: confirmed_mutation.into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn list_territories(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::ListTerritoriesRequest>,
    ) -> ServiceResult<sale_wire::ListTerritoriesResponse> {
        let size = if req.page.page_size == 0 {
            25
        } else {
            req.page.page_size
        };
        if size > 100 {
            return Err(ConnectError::invalid_argument("page_size exceeds 100"));
        }
        if !req.page.page_token.is_empty() {
            Self::id(&req.page.page_token)?;
        }
        let (items, next) = sales_territories::list(
            &self.d1()?,
            &self.tenant,
            sales_territories::Kind::Territory,
            sales_territories::Filter {
                after: &req.page.page_token,
                limit: size as usize,
                include_archived: req.include_archived,
                territory_id: None,
                primary_owner: None,
            },
        )
        .await
        .map_err(|_| ConnectError::internal("Sales coverage list failed"))?;
        RpcResponse::ok(sale_wire::ListTerritoriesResponse {
            territories: items
                .into_iter()
                .map(Self::territory_wire)
                .collect::<std::result::Result<Vec<_>, _>>()?,
            page_info: Some(core_v2::PageInfo {
                next_page_token: next.unwrap_or_default(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn create_sales_account_assignment(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::CreateSalesAccountAssignmentRequest>,
    ) -> ServiceResult<sale_wire::CreateSalesAccountAssignmentResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.account_id)?;
        let (key, expected) = wire_precondition(input.precondition.as_option(), true)?;
        let cfg = input.configuration.as_option().ok_or_else(|| {
            ConnectError::invalid_argument("SALES_COVERAGE_CONFIGURATION_REQUIRED")
        })?;
        let record = Self::assignment_draft(id, cfg)?;
        let db = self.d1()?;
        let saved = sales_territories::mutate(
            &sales_territories::Context {
                db: &db,
                tenant: &self.tenant,
                actor: &self.actor,
                request_id: &self.request_id,
            },
            record,
            &key,
            expected,
        )
        .await
        .map_err(|_| ConnectError::unavailable("SALES_COVERAGE_WRITE_FAILED"))?
        .map_err(|failure| failure.connect("CreateSalesAccountAssignment", &self.request_id))?;
        let receipt = self.coverage_receipt(&saved);
        RpcResponse::ok(sale_wire::CreateSalesAccountAssignmentResponse {
            assignment: Some(Self::assignment_wire(saved.record)?).into(),
            receipt: Some(receipt).into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn update_sales_account_assignment(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::UpdateSalesAccountAssignmentRequest>,
    ) -> ServiceResult<sale_wire::UpdateSalesAccountAssignmentResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.account_id)?;
        let (key, expected) = wire_precondition(input.precondition.as_option(), false)?;
        let cfg = input.configuration.as_option().ok_or_else(|| {
            ConnectError::invalid_argument("SALES_COVERAGE_CONFIGURATION_REQUIRED")
        })?;
        let record = Self::assignment_draft(id, cfg)?;
        let db = self.d1()?;
        let saved = sales_territories::mutate(
            &sales_territories::Context {
                db: &db,
                tenant: &self.tenant,
                actor: &self.actor,
                request_id: &self.request_id,
            },
            record,
            &key,
            expected,
        )
        .await
        .map_err(|_| ConnectError::unavailable("SALES_COVERAGE_WRITE_FAILED"))?
        .map_err(|failure| failure.connect("UpdateSalesAccountAssignment", &self.request_id))?;
        let receipt = self.coverage_receipt(&saved);
        RpcResponse::ok(sale_wire::UpdateSalesAccountAssignmentResponse {
            assignment: Some(Self::assignment_wire(saved.record)?).into(),
            receipt: Some(receipt).into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn get_sales_account_assignment(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::GetSalesAccountAssignmentRequest>,
    ) -> ServiceResult<sale_wire::GetSalesAccountAssignmentResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.account_id)?;
        let db = self.d1()?;
        let kind = sales_territories::Kind::Assignment;
        if accounts::load(&db, &self.tenant, &id)
            .await
            .map_err(|_| ConnectError::internal("Account read failed"))?
            .is_none()
        {
            return Err(ConnectError::not_found("CUSTOMER_ACCOUNTS_NOT_FOUND"));
        }
        let record = sales_territories::get(&db, &self.tenant, kind, &id)
            .await
            .map_err(|_| ConnectError::internal("Sales coverage read failed"))?;
        let confirmed_mutation = match receipt_key(input.receipt_idempotency_key.as_deref())? {
            None => None,
            Some(key) => {
                let row = crud_receipt(&db, &self.tenant, key, CrudReceiptOwner::AccountAssignment)
                    .await
                    .map_err(|_| ConnectError::unavailable("SALES_COVERAGE_RECEIPT_READ_FAILED"))?;
                crud_commit_evidence(row, &id, &self.request_id, |state| {
                    let record = kind.decode(state)?;
                    Ok((record.id().to_owned(), record.version()))
                })
                .map_err(|_| ConnectError::unavailable("SALES_COVERAGE_RECEIPT_DECODE_FAILED"))?
            }
        };
        RpcResponse::ok(sale_wire::GetSalesAccountAssignmentResponse {
            assignment: record.map(Self::assignment_wire).transpose()?.into(),
            confirmed_mutation: confirmed_mutation.into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn list_sales_account_assignments(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::ListSalesAccountAssignmentsRequest>,
    ) -> ServiceResult<sale_wire::ListSalesAccountAssignmentsResponse> {
        let size = if req.page.page_size == 0 {
            25
        } else {
            req.page.page_size
        };
        if size > 100 {
            return Err(ConnectError::invalid_argument("page_size exceeds 100"));
        }
        if !req.page.page_token.is_empty() {
            Self::id(&req.page.page_token)?;
        }
        let after = req.page.page_token.to_owned();
        let include_archived = req.include_archived;
        let input = req.to_owned_message();
        let owner = Self::owner_scope(input.primary_owner_filter.as_option())?;
        if let Some(id) = &input.territory_id {
            Self::id(id)?;
        }
        let (items, next) = sales_territories::list(
            &self.d1()?,
            &self.tenant,
            sales_territories::Kind::Assignment,
            sales_territories::Filter {
                after: &after,
                limit: size as usize,
                include_archived,
                territory_id: input.territory_id.as_deref(),
                primary_owner: owner.as_ref(),
            },
        )
        .await
        .map_err(|_| ConnectError::internal("Sales coverage list failed"))?;
        RpcResponse::ok(sale_wire::ListSalesAccountAssignmentsResponse {
            assignments: items
                .into_iter()
                .map(Self::assignment_wire)
                .collect::<std::result::Result<Vec<_>, _>>()?,
            page_info: Some(core_v2::PageInfo {
                next_page_token: next.unwrap_or_default(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn create_sales_view(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::CreateSalesViewRequest>,
    ) -> ServiceResult<sale_wire::CreateSalesViewResponse> {
        let input = req.to_owned_message();
        Self::id(&input.view_id)?;
        let status = Self::view_status(input.status)?;
        if let Some(id) = &input.account_id {
            AccountV2Service::valid_id(id)?;
        }
        if let Some(id) = &input.pipeline_id {
            Self::pipeline_id(id)?;
        }
        let (key, _) = wire_precondition(input.precondition.as_option(), true)?;
        let saved = sales_views::create(
            sales_views::CreateInput {
                view_id: input.view_id,
                name: input.name,
                pipeline_id: input.pipeline_id,
                status,
                account_id: input.account_id,
                owner_filter: Self::owner_scope(input.owner_filter.as_option())?,
            },
            &key,
            &self.d1()?,
            &self.tenant,
            &self.actor,
            &self.request_id,
        )
        .await
        .map_err(|_| ConnectError::internal("Sales view D1 create failed"))?
        .map_err(|failure| failure.connect("CreateSalesView", &self.request_id))?;
        RpcResponse::ok(sale_wire::CreateSalesViewResponse {
            view: Some(Self::view(&saved.value)).into(),
            receipt: Some(self.view_receipt(&saved)).into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn update_sales_view(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::UpdateSalesViewRequest>,
    ) -> ServiceResult<sale_wire::UpdateSalesViewResponse> {
        let input = req.to_owned_message();
        Self::id(&input.view_id)?;
        let status = Self::view_status(input.status)?;
        if let Some(id) = &input.account_id {
            AccountV2Service::valid_id(id)?;
        }
        if let Some(id) = &input.pipeline_id {
            Self::pipeline_id(id)?;
        }
        let (key, version) = wire_precondition(input.precondition.as_option(), false)?;
        let saved = sales_views::update(
            sales_views::UpdateInput {
                view_id: input.view_id,
                expected_version: version.unwrap_or_default(),
                name: input.name,
                pipeline_id: input.pipeline_id,
                status,
                account_id: input.account_id,
                owner_filter: Self::owner_scope(input.owner_filter.as_option())?,
                archived: input.archived,
            },
            &key,
            &self.d1()?,
            &self.tenant,
            &self.actor,
            &self.request_id,
        )
        .await
        .map_err(|_| ConnectError::internal("Sales view D1 update failed"))?
        .map_err(|failure| failure.connect("UpdateSalesView", &self.request_id))?;
        RpcResponse::ok(sale_wire::UpdateSalesViewResponse {
            view: Some(Self::view(&saved.value)).into(),
            receipt: Some(self.view_receipt(&saved)).into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn get_sales_view(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::GetSalesViewRequest>,
    ) -> ServiceResult<sale_wire::GetSalesViewResponse> {
        let input = req.to_owned_message();
        Self::id(&input.view_id)?;
        let db = self.d1()?;
        let value = sales_views::get(&db, &self.tenant, &input.view_id)
            .await
            .map_err(|_| {
                diagnostics::read_failure(
                    &self.request_id,
                    "sales_view_read",
                    diagnostics::ReadFailureReason::InternalReadError,
                );
                ConnectError::internal("Sales view D1 read failed")
            })?
            .ok_or_else(|| ConnectError::not_found("SALES_VIEW_NOT_FOUND"))?;
        let confirmed_mutation = match receipt_key(input.receipt_idempotency_key.as_deref())? {
            None => None,
            Some(key) => {
                let row = crud_receipt(&db, &self.tenant, key, CrudReceiptOwner::SalesView)
                    .await
                    .map_err(|_| {
                        diagnostics::read_failure(
                            &self.request_id,
                            "sales_view_receipt_lookup",
                            diagnostics::ReadFailureReason::InternalReadError,
                        );
                        ConnectError::unavailable("SALES_VIEW_RECEIPT_READ_FAILED")
                    })?;
                crud_commit_evidence(row, &input.view_id, &self.request_id, |state| {
                    let view = crm_sales::SalesDealView::decode(state.as_bytes())
                        .map_err(|error| worker::Error::RustError(error.code))?;
                    Ok((view.view_id, view.version))
                })
                .map_err(|_| {
                    diagnostics::read_failure(
                        &self.request_id,
                        "sales_view_receipt_decode",
                        diagnostics::ReadFailureReason::SalesStateDecodeFailed,
                    );
                    ConnectError::unavailable("SALES_VIEW_RECEIPT_READ_FAILED")
                })?
            }
        };
        RpcResponse::ok(sale_wire::GetSalesViewResponse {
            view: Some(Self::view(&value)).into(),
            confirmed_mutation: confirmed_mutation.into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn list_sales_views(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::ListSalesViewsRequest>,
    ) -> ServiceResult<sale_wire::ListSalesViewsResponse> {
        let size = if req.page.page_size == 0 {
            25
        } else {
            req.page.page_size
        };
        if size > 100 {
            return Err(ConnectError::invalid_argument("page_size exceeds 100"));
        }
        let cursor = &req.page.page_token;
        if !cursor.is_empty() {
            Self::id(cursor)?;
        }
        let (views, next) = sales_views::list(
            &self.d1()?,
            &self.tenant,
            cursor,
            size as usize,
            req.include_archived,
        )
        .await
        .map_err(|_| ConnectError::internal("Sales view D1 list failed"))?;
        RpcResponse::ok(sale_wire::ListSalesViewsResponse {
            views: views.iter().map(Self::view).collect(),
            page_info: Some(core_v2::PageInfo {
                next_page_token: next.unwrap_or_default(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn create_pipeline(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::CreatePipelineRequest>,
    ) -> ServiceResult<sale_wire::CreatePipelineResponse> {
        let input = req.to_owned_message();
        Self::pipeline_id(&input.pipeline_id)?;
        let (key, _) = wire_precondition(input.precondition.as_option(), true)?;
        let saved = sales_pipelines::create(
            sales_pipelines::CreateInput {
                pipeline_id: input.pipeline_id,
                name: input.name,
                stages: Self::stage_inputs(&input.stages)?,
            },
            &key,
            &self.d1()?,
            &self.tenant,
            &self.actor,
            &self.request_id,
        )
        .await
        .map_err(|_| ConnectError::internal("Pipeline D1 create failed"))?
        .map_err(|failure| failure.connect("CreatePipeline", &self.request_id))?;
        RpcResponse::ok(sale_wire::CreatePipelineResponse {
            pipeline: Some(Self::pipeline(&saved.value)).into(),
            receipt: Some(self.pipeline_receipt(&saved)).into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn update_pipeline(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::UpdatePipelineRequest>,
    ) -> ServiceResult<sale_wire::UpdatePipelineResponse> {
        let input = req.to_owned_message();
        Self::pipeline_id(&input.pipeline_id)?;
        let (key, version) = wire_precondition(input.precondition.as_option(), false)?;
        let saved = sales_pipelines::update(
            sales_pipelines::UpdateInput {
                pipeline_id: input.pipeline_id,
                expected_version: version.unwrap_or_default(),
                name: input.name,
                stages: Self::stage_inputs(&input.stages)?,
                archived: input.archived,
            },
            &key,
            &self.d1()?,
            &self.tenant,
            &self.actor,
            &self.request_id,
        )
        .await
        .map_err(|_| ConnectError::internal("Pipeline D1 update failed"))?
        .map_err(|failure| failure.connect("UpdatePipeline", &self.request_id))?;
        RpcResponse::ok(sale_wire::UpdatePipelineResponse {
            pipeline: Some(Self::pipeline(&saved.value)).into(),
            receipt: Some(self.pipeline_receipt(&saved)).into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn get_pipeline(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::GetPipelineRequest>,
    ) -> ServiceResult<sale_wire::GetPipelineResponse> {
        let input = req.to_owned_message();
        Self::pipeline_id(&input.pipeline_id)?;
        let db = self.d1()?;
        let pipeline = sales_pipelines::get(&db, &self.tenant, &input.pipeline_id)
            .await
            .map_err(|_| {
                diagnostics::read_failure(
                    &self.request_id,
                    "sales_pipeline_read",
                    diagnostics::ReadFailureReason::InternalReadError,
                );
                ConnectError::internal("Pipeline D1 get failed")
            })?
            .ok_or_else(|| ConnectError::not_found("SALES_PIPELINE_NOT_FOUND"))?;
        let confirmed_mutation = match receipt_key(input.receipt_idempotency_key.as_deref())? {
            None => None,
            Some(key) => {
                let row = crud_receipt(&db, &self.tenant, key, CrudReceiptOwner::Pipeline)
                    .await
                    .map_err(|_| {
                        diagnostics::read_failure(
                            &self.request_id,
                            "sales_pipeline_receipt_lookup",
                            diagnostics::ReadFailureReason::InternalReadError,
                        );
                        ConnectError::unavailable("SALES_PIPELINE_RECEIPT_READ_FAILED")
                    })?;
                crud_commit_evidence(row, &input.pipeline_id, &self.request_id, |state| {
                    let pipeline = crm_sales::SalesPipeline::decode(state.as_bytes())
                        .map_err(|error| worker::Error::RustError(error.code))?;
                    Ok((pipeline.pipeline_id, pipeline.version))
                })
                .map_err(|_| {
                    diagnostics::read_failure(
                        &self.request_id,
                        "sales_pipeline_receipt_decode",
                        diagnostics::ReadFailureReason::SalesStateDecodeFailed,
                    );
                    ConnectError::unavailable("SALES_PIPELINE_RECEIPT_READ_FAILED")
                })?
            }
        };
        RpcResponse::ok(sale_wire::GetPipelineResponse {
            pipeline: Some(Self::pipeline(&pipeline)).into(),
            confirmed_mutation: confirmed_mutation.into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn list_pipelines(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::ListPipelinesRequest>,
    ) -> ServiceResult<sale_wire::ListPipelinesResponse> {
        let size = if req.page.page_size == 0 {
            25
        } else {
            req.page.page_size
        };
        if size > 100 {
            return Err(ConnectError::invalid_argument("page_size exceeds 100"));
        }
        let cursor = &req.page.page_token;
        if !cursor.is_empty() {
            Self::pipeline_id(cursor)?;
        }
        let (pipelines, next) = sales_pipelines::list(
            &self.d1()?,
            &self.tenant,
            cursor,
            size as usize,
            req.include_archived,
        )
        .await
        .map_err(|_| ConnectError::internal("Pipeline D1 list failed"))?;
        RpcResponse::ok(sale_wire::ListPipelinesResponse {
            pipelines: pipelines.iter().map(Self::pipeline).collect(),
            currencies: if cursor.is_empty() {
                crm_core_contracts::CurrencyCode::catalog()
                    .into_iter()
                    .map(|currency| core_v2::CurrencyDefinition {
                        currency_code: currency.as_str().to_owned(),
                        minor_unit_exponent: Some(u32::from(currency.exponent())),
                        ..Default::default()
                    })
                    .collect()
            } else {
                Vec::new()
            },
            page_info: Some(core_v2::PageInfo {
                next_page_token: next.unwrap_or_default(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn list_stage_deals(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::ListStageDealsRequest>,
    ) -> ServiceResult<sale_wire::ListStageDealsResponse> {
        Self::pipeline_id(req.pipeline_id)?;
        crm_sales::StageId::try_new(req.stage_id.to_owned())
            .map_err(|_| ConnectError::invalid_argument("SALES_STAGE_ID_INVALID"))?;
        let size = if req.page.page_size == 0 {
            25
        } else {
            req.page.page_size
        };
        if size > 100 {
            return Err(ConnectError::invalid_argument("page_size exceeds 100"));
        }
        let cursor = &req.page.page_token;
        if !cursor.is_empty() {
            Self::id(cursor)?;
        }
        let status = match req.status {
            None => None,
            Some(raw) => Some(match raw {
                x if x == sale_wire::DealStatus::DEAL_STATUS_OPEN => DomainDealStatus::Open,
                x if x == sale_wire::DealStatus::DEAL_STATUS_WON => DomainDealStatus::Won,
                x if x == sale_wire::DealStatus::DEAL_STATUS_LOST => DomainDealStatus::Lost,
                _ => return Err(ConnectError::invalid_argument("invalid DealStatus filter")),
            }),
        };
        if let Some(id) = req.account_id {
            AccountV2Service::valid_id(id)?;
        }
        let db = self.d1()?;
        let pipeline = sales_pipelines::get(&db, &self.tenant, req.pipeline_id)
            .await
            .map_err(|_| ConnectError::internal("Pipeline D1 read failed"))?
            .ok_or_else(|| ConnectError::not_found("SALES_PIPELINE_NOT_FOUND"))?;
        if pipeline.stage(req.stage_id).is_none() {
            return Err(ConnectError::not_found("SALES_PIPELINE_STAGE_NOT_FOUND"));
        }
        let input = req.to_owned_message();
        let owner_filter = Self::owner_scope(input.owner_filter.as_option())?;
        let (list, next, total_count, currency_totals) = sales_deals::list_stage_page(
            &db,
            &self.tenant,
            sales_deals::StageListQuery {
                pipeline_id: req.pipeline_id,
                stage_id: req.stage_id,
                after: cursor,
                limit: size as usize,
                status,
                account: req.account_id,
                owner_filter: owner_filter.as_ref(),
            },
        )
        .await
        .map_err(|error| {
            diagnostics::read_failure(&self.request_id, "sales_stage_list", error);
            ConnectError::internal("Sales D1 stage list failed")
        })?;
        let deals = list
            .iter()
            .map(Self::convert)
            .collect::<std::result::Result<Vec<_>, ConnectError>>()?;
        RpcResponse::ok(sale_wire::ListStageDealsResponse {
            deals,
            page_info: Some(core_v2::PageInfo {
                next_page_token: next.unwrap_or_default(),
                ..Default::default()
            })
            .into(),
            total_count,
            currency_totals: currency_totals
                .into_iter()
                .map(|total| sale_wire::StageCurrencyTotal {
                    currency_code: total.currency_code,
                    total_minor_units: total.total_minor_units.to_string(),
                    amount_deal_count: total.amount_deal_count,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn get_pipeline_metrics(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::GetPipelineMetricsRequest>,
    ) -> ServiceResult<sale_wire::GetPipelineMetricsResponse> {
        Self::pipeline_id(req.pipeline_id)?;
        let status = match req.status {
            None => None,
            Some(raw) => Some(match raw {
                x if x == sale_wire::DealStatus::DEAL_STATUS_OPEN => DomainDealStatus::Open,
                x if x == sale_wire::DealStatus::DEAL_STATUS_WON => DomainDealStatus::Won,
                x if x == sale_wire::DealStatus::DEAL_STATUS_LOST => DomainDealStatus::Lost,
                _ => return Err(ConnectError::invalid_argument("invalid DealStatus filter")),
            }),
        };
        if let Some(id) = req.account_id {
            AccountV2Service::valid_id(id)?;
        }
        let db = self.d1()?;
        if sales_pipelines::get(&db, &self.tenant, req.pipeline_id)
            .await
            .map_err(|_| ConnectError::internal("Pipeline D1 read failed"))?
            .is_none()
        {
            return Err(ConnectError::not_found("SALES_PIPELINE_NOT_FOUND"));
        }
        let input = req.to_owned_message();
        let period = input
            .analysis_period
            .as_option()
            .map(|period| {
                let date = |value: &core_v2::CalendarDate| sales_deals::DateInput {
                    year: value.year,
                    month: value.month,
                    day: value.day,
                };
                let start = period.start_date.as_option().ok_or_else(|| {
                    ConnectError::invalid_argument("SALES_ANALYSIS_PERIOD_START_REQUIRED")
                })?;
                let end = period.end_date.as_option().ok_or_else(|| {
                    ConnectError::invalid_argument("SALES_ANALYSIS_PERIOD_END_REQUIRED")
                })?;
                sales_deals::AnalysisPeriod::new(date(start), date(end))
                    .map_err(ConnectError::invalid_argument)
            })
            .transpose()?;
        let owner_filter = Self::owner_scope(input.owner_filter.as_option())?;
        let metrics = sales_deals::pipeline_metrics(
            &db,
            &self.tenant,
            req.pipeline_id,
            status,
            req.account_id,
            period,
            owner_filter.as_ref(),
        )
        .await
        .map_err(|error| {
            diagnostics::read_failure(&self.request_id, "sales_metrics", error);
            ConnectError::internal("Sales D1 metrics read failed")
        })?;
        let as_status = |value: DomainDealStatus| match value {
            DomainDealStatus::Open => sale_wire::DealStatus::DEAL_STATUS_OPEN,
            DomainDealStatus::Won => sale_wire::DealStatus::DEAL_STATUS_WON,
            DomainDealStatus::Lost => sale_wire::DealStatus::DEAL_STATUS_LOST,
        };
        let forecast_category_metric =
            |row: sales_deals::ForecastCategoryMetric| sale_wire::ForecastCategoryMetric {
                category: Self::forecast_wire(row.category).into(),
                deal_count: row.deal_count,
                stage_default_count: row.stage_default_count,
                manual_count: row.manual_count,
                ..Default::default()
            };
        let forecast_currency_metric =
            |row: sales_deals::ForecastCurrencyMetric| sale_wire::ForecastCurrencyMetric {
                category: Self::forecast_wire(row.category).into(),
                currency_code: row.currency_code,
                total_minor_units: row.total_minor_units.to_string(),
                weighted_minor_units_times_basis_points: row
                    .weighted_minor_units_times_basis_points
                    .to_string(),
                amount_deal_count: row.amount_deal_count,
                ..Default::default()
            };
        RpcResponse::ok(sale_wire::GetPipelineMetricsResponse {
            pipeline_id: req.pipeline_id.to_owned(),
            total_deal_count: metrics.total_deal_count,
            statuses: metrics
                .statuses
                .into_iter()
                .map(|row| sale_wire::PipelineStatusMetric {
                    stage_id: row.stage_id,
                    status: as_status(row.status).into(),
                    deal_count: row.deal_count,
                    ..Default::default()
                })
                .collect(),
            currencies: metrics
                .currencies
                .into_iter()
                .map(|row| sale_wire::PipelineCurrencyMetric {
                    stage_id: row.stage_id,
                    status: as_status(row.status).into(),
                    currency_code: row.currency_code,
                    total_minor_units: row.total_minor_units.to_string(),
                    weighted_minor_units_times_basis_points: row
                        .weighted_minor_units_times_basis_points
                        .to_string(),
                    amount_deal_count: row.amount_deal_count,
                    ..Default::default()
                })
                .collect(),
            operational: metrics
                .operational
                .into_iter()
                .map(|row| sale_wire::PipelineOperationalMetric {
                    stage_id: row.stage_id,
                    status: as_status(row.status).into(),
                    overdue_expected_close_count: row.overdue_expected_close_count,
                    due_next_30_days_count: row.due_next_30_days_count,
                    due_later_count: row.due_later_count,
                    no_expected_close_count: row.no_expected_close_count,
                    known_stage_age_count: row.known_stage_age_count,
                    total_stage_age_seconds: row.total_stage_age_seconds.to_string(),
                    overdue_linked_task_count: row.overdue_linked_task_count,
                    ..Default::default()
                })
                .collect(),
            forecast_categories: metrics
                .forecast_categories
                .into_iter()
                .map(forecast_category_metric)
                .collect(),
            forecast_currencies: metrics
                .forecast_currencies
                .into_iter()
                .map(forecast_currency_metric)
                .collect(),
            undated_forecast_categories: metrics
                .undated_forecast_categories
                .into_iter()
                .map(forecast_category_metric)
                .collect(),
            undated_forecast_currencies: metrics
                .undated_forecast_currencies
                .into_iter()
                .map(forecast_currency_metric)
                .collect(),
            closed_outcomes: metrics
                .closed_outcomes
                .map(|row| sale_wire::SalesClosedOutcomeMetric {
                    won_count: row.won_count,
                    lost_count: row.lost_count,
                    unknown_close_count: row.unknown_close_count,
                    denominator: row.denominator(),
                    win_rate_basis_points: row.win_rate_basis_points(),
                    ..Default::default()
                })
                .into(),
            analysis_period: input.analysis_period,
            as_of_unix_seconds: metrics.as_of_unix_seconds,
            ..Default::default()
        })
    }
    #[worker::send]
    async fn create_deal(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::CreateDealRequest>,
    ) -> ServiceResult<sale_wire::CreateDealResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.deal_id)?;
        let (key, _) = wire_precondition(input.precondition.as_option(), true)?;
        let stage = input
            .stage
            .as_option()
            .ok_or_else(|| ConnectError::invalid_argument("stage required"))?;
        let (pipeline_id, stage_id) = Self::stage(stage);
        let probability_bps = input
            .probability_basis_points
            .map(|value| {
                u16::try_from(value)
                    .map_err(|_| ConnectError::invalid_argument("invalid probability basis points"))
            })
            .transpose()?;
        let forecast_category =
            Self::forecast_category(input.forecast_category)?.unwrap_or_default();
        let (amount_minor_units, currency) = match input.amount.as_option() {
            Some(money) => (Some(money.minor_units), Some(money.currency_code.clone())),
            None => (None, None),
        };
        let db = self.d1()?;
        let saved = sales_deals::create_typed(
            sales_deals::CreateInput {
                id,
                name: input.name,
                account_id: input.account_id,
                owner: Self::owner_scope(input.owner.as_option())?,
                pipeline_id,
                stage_id,
                probability_bps,
                forecast_category,
                amount_minor_units,
                currency,
                primary_contact_id: input.primary_contact_id,
                expected_close_date: input.expected_close_date.as_option().map(|d| {
                    sales_deals::DateInput {
                        year: d.year,
                        month: d.month,
                        day: d.day,
                    }
                }),
            },
            &key,
            &db,
            &self.tenant,
            &self.actor,
            &self.request_id,
        )
        .await
        .map_err(|_| diagnostics::mutation_preparation_failure("CreateDeal", &self.request_id))?
        .map_err(|failure| diagnostics::mutation_error("CreateDeal", &self.request_id, failure))?;
        RpcResponse::ok(sale_wire::CreateDealResponse {
            deal: Some(Self::convert(&saved.deal)?).into(),
            receipt: Some(diagnostics::receipt(
                "CreateDeal",
                &self.request_id,
                saved.original_request_id.as_deref(),
                saved.deal.version(),
                saved.replayed,
                saved.d1_statement_changes,
            ))
            .into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn update_deal(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::UpdateDealRequest>,
    ) -> ServiceResult<sale_wire::UpdateDealResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.deal_id)?;
        let (key, version) = wire_precondition(input.precondition.as_option(), false)?;
        let (name, clear_name) = match input.name.as_option() {
            None => (None, None),
            Some(patch) => match patch.operation.as_ref() {
                Some(core_v2::string_patch::Operation::Set(name)) => (Some(name.clone()), None),
                Some(core_v2::string_patch::Operation::Clear(true)) => (None, Some(true)),
                _ => return Err(ConnectError::invalid_argument("invalid StringPatch")),
            },
        };
        let (amount_minor_units, currency, clear_amount) = match input.amount.as_option() {
            None => (None, None, None),
            Some(patch) => match patch.operation.as_ref() {
                Some(core_v2::money_patch::Operation::Set(money)) => (
                    Some(money.minor_units),
                    Some(money.currency_code.clone()),
                    None,
                ),
                Some(core_v2::money_patch::Operation::Clear(true)) => (None, None, Some(true)),
                _ => return Err(ConnectError::invalid_argument("invalid MoneyPatch")),
            },
        };
        let probability_bps = input
            .probability_basis_points
            .map(|v| {
                u16::try_from(v)
                    .map_err(|_| ConnectError::invalid_argument("invalid probability basis points"))
            })
            .transpose()?;
        let primary_contact = match input.primary_contact_id.as_option() {
            None => sales_deals::FieldChange::Keep,
            Some(p) => match p.operation.as_ref() {
                Some(core_v2::string_patch::Operation::Set(value)) => {
                    sales_deals::FieldChange::Set(value.clone())
                }
                Some(core_v2::string_patch::Operation::Clear(true)) => {
                    sales_deals::FieldChange::Clear
                }
                _ => {
                    return Err(ConnectError::invalid_argument(
                        "invalid primary contact patch",
                    ));
                }
            },
        };
        let expected_close_date = match input.expected_close_date.as_option() {
            None => sales_deals::FieldChange::Keep,
            Some(p) => match p.operation.as_ref() {
                Some(core_v2::calendar_date_patch::Operation::Set(d)) => {
                    sales_deals::FieldChange::Set(sales_deals::DateInput {
                        year: d.year,
                        month: d.month,
                        day: d.day,
                    })
                }
                Some(core_v2::calendar_date_patch::Operation::Clear(true)) => {
                    sales_deals::FieldChange::Clear
                }
                _ => {
                    return Err(ConnectError::invalid_argument(
                        "invalid expected date patch",
                    ));
                }
            },
        };
        let saved = sales_deals::update_typed(
            sales_deals::UpdateInput {
                expected_version: version.unwrap_or_default(),
                name,
                clear_name,
                amount_minor_units,
                currency,
                clear_amount,
                probability_bps,
                use_stage_probability: input.use_stage_probability,
                forecast_category: Self::forecast_category(input.forecast_category)?,
                primary_contact,
                expected_close_date,
                owner: Self::owner_scope(input.owner.as_option())?,
            },
            &key,
            &self.d1()?,
            &self.tenant,
            &id,
            &self.actor,
            &self.request_id,
        )
        .await
        .map_err(|_| diagnostics::mutation_preparation_failure("UpdateDeal", &self.request_id))?
        .map_err(|failure| diagnostics::mutation_error("UpdateDeal", &self.request_id, failure))?;
        RpcResponse::ok(sale_wire::UpdateDealResponse {
            deal: Some(Self::convert(&saved.deal)?).into(),
            receipt: Some(diagnostics::receipt(
                "UpdateDeal",
                &self.request_id,
                saved.original_request_id.as_deref(),
                saved.deal.version(),
                saved.replayed,
                saved.d1_statement_changes,
            ))
            .into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn advance_stage(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::AdvanceStageRequest>,
    ) -> ServiceResult<sale_wire::AdvanceStageResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.deal_id)?;
        let (key, version) = wire_precondition(input.precondition.as_option(), false)?;
        let stage = input
            .target_stage
            .as_option()
            .ok_or_else(|| ConnectError::invalid_argument("target_stage required"))?;
        let (pipeline_id, stage_id) = Self::stage(stage);
        let status = Self::deal_status(input.target_status)?;
        if !matches!(status, sales_deals::StatusInput::Open) {
            return Err(ConnectError::invalid_argument(
                "SALES_CLOSE_SEPARATE_ACTION_REQUIRED",
            ));
        }
        let advanced = sales_deals::advance_typed(
            sales_deals::StageInput {
                expected_version: version.unwrap_or_default(),
                pipeline_id,
                stage_id,
                status,
                close_reason_code: input.close_reason_code,
            },
            &key,
            &self.d1()?,
            &self.tenant,
            &id,
            &self.actor,
            &self.request_id,
        )
        .await
        .map_err(|_| diagnostics::mutation_preparation_failure("AdvanceStage", &self.request_id))?
        .map_err(|failure| {
            diagnostics::mutation_error("AdvanceStage", &self.request_id, failure)
        })?;
        RpcResponse::ok(sale_wire::AdvanceStageResponse {
            deal: Some(Self::convert(&advanced.saved.deal)?).into(),
            receipt: Some(diagnostics::receipt(
                "AdvanceStage",
                &self.request_id,
                advanced.saved.original_request_id.as_deref(),
                advanced.saved.deal.version(),
                advanced.saved.replayed,
                advanced.saved.d1_statement_changes,
            ))
            .into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn close_deal(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::CloseDealRequest>,
    ) -> ServiceResult<sale_wire::CloseDealResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.deal_id)?;
        let (key, version) = wire_precondition(input.precondition.as_option(), false)?;
        let status = Self::deal_status(input.outcome)?;
        if matches!(status, sales_deals::StatusInput::Open) {
            return Err(ConnectError::invalid_argument(
                "SALES_CLOSE_OUTCOME_REQUIRED",
            ));
        }
        if input.reason_code.is_empty() {
            return Err(ConnectError::invalid_argument(
                "SALES_CLOSE_REASON_REQUIRED",
            ));
        }
        let saved = sales_deals::close_typed(
            sales_deals::CloseInput {
                expected_version: version.unwrap_or_default(),
                outcome: status,
                reason_code: input.reason_code,
            },
            &key,
            &self.d1()?,
            &self.tenant,
            &id,
            &self.actor,
            &self.request_id,
        )
        .await
        .map_err(|_| diagnostics::mutation_preparation_failure("CloseDeal", &self.request_id))?
        .map_err(|failure| diagnostics::mutation_error("CloseDeal", &self.request_id, failure))?;
        RpcResponse::ok(sale_wire::CloseDealResponse {
            deal: Some(Self::convert(&saved.saved.deal)?).into(),
            receipt: Some(diagnostics::receipt(
                "CloseDeal",
                &self.request_id,
                saved.saved.original_request_id.as_deref(),
                saved.saved.deal.version(),
                saved.saved.replayed,
                saved.saved.d1_statement_changes,
            ))
            .into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn reopen_deal(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::ReopenDealRequest>,
    ) -> ServiceResult<sale_wire::ReopenDealResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.deal_id)?;
        let (key, version) = wire_precondition(input.precondition.as_option(), false)?;
        if input.reason_code.is_empty() {
            return Err(ConnectError::invalid_argument(
                "SALES_REOPEN_REASON_REQUIRED",
            ));
        }
        let saved = sales_deals::reopen_typed(
            sales_deals::ReopenInput {
                expected_version: version.unwrap_or_default(),
                reason_code: input.reason_code,
                owner: Self::owner_scope(input.owner.as_option())?,
            },
            &key,
            &self.d1()?,
            &self.tenant,
            &id,
            &self.actor,
            &self.request_id,
        )
        .await
        .map_err(|_| diagnostics::mutation_preparation_failure("ReopenDeal", &self.request_id))?
        .map_err(|failure| diagnostics::mutation_error("ReopenDeal", &self.request_id, failure))?;
        RpcResponse::ok(sale_wire::ReopenDealResponse {
            deal: Some(Self::convert(&saved.deal)?).into(),
            receipt: Some(diagnostics::receipt(
                "ReopenDeal",
                &self.request_id,
                saved.original_request_id.as_deref(),
                saved.deal.version(),
                saved.replayed,
                saved.d1_statement_changes,
            ))
            .into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn list_deal_events(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::ListDealEventsRequest>,
    ) -> ServiceResult<sale_wire::ListDealEventsResponse> {
        let id = Self::id(req.deal_id)?;
        let size = if req.page.page_size == 0 {
            25
        } else {
            req.page.page_size
        };
        if size > 100 {
            return Err(ConnectError::invalid_argument("page_size exceeds 100"));
        }
        let before = if req.page.page_token.is_empty() {
            i64::MAX
        } else {
            let parsed = req
                .page
                .page_token
                .parse::<i64>()
                .map_err(|_| ConnectError::invalid_argument("SALES_HISTORY_CURSOR_INVALID"))?;
            if parsed <= 0 {
                return Err(ConnectError::invalid_argument(
                    "SALES_HISTORY_CURSOR_INVALID",
                ));
            }
            parsed
        };
        let db = self.d1()?;
        if sales_deals::load(&db, &self.tenant, &id)
            .await
            .map_err(|_| {
                diagnostics::read_failure(
                    &self.request_id,
                    "sales_history_owner",
                    diagnostics::ReadFailureReason::InternalReadError,
                );
                ConnectError::internal("Sales history owner lookup failed")
            })?
            .is_none()
        {
            return Err(ConnectError::not_found("SALES_DEAL_NOT_FOUND"));
        }
        let (rows, next) = sales_deals::list_events(&db, &self.tenant, &id, before, size as usize)
            .await
            .map_err(|_| {
                diagnostics::read_failure(
                    &self.request_id,
                    "sales_history_read",
                    diagnostics::ReadFailureReason::InternalReadError,
                );
                ConnectError::internal("Sales D1 history read failed")
            })?;
        let events = rows
            .into_iter()
            .map(|row| {
                let kind = match row.action.as_str() {
                    "created" => sale_wire::DealEventKind::DEAL_EVENT_KIND_CREATED,
                    "updated" => sale_wire::DealEventKind::DEAL_EVENT_KIND_UPDATED,
                    "moved" => sale_wire::DealEventKind::DEAL_EVENT_KIND_MOVED,
                    "closed" => sale_wire::DealEventKind::DEAL_EVENT_KIND_CLOSED,
                    "reopened" => sale_wire::DealEventKind::DEAL_EVENT_KIND_REOPENED,
                    _ => {
                        return Err(ConnectError::internal(
                            "invalid persisted Sales event action",
                        ));
                    }
                };
                let status = |value: &str| match value {
                    "open" => Ok(sale_wire::DealStatus::DEAL_STATUS_OPEN),
                    "won" => Ok(sale_wire::DealStatus::DEAL_STATUS_WON),
                    "lost" => Ok(sale_wire::DealStatus::DEAL_STATUS_LOST),
                    _ => Err(ConnectError::internal(
                        "invalid persisted Sales event status",
                    )),
                };
                let previous_status = match row.previous_status {
                    Some(previous) => status(&previous)?,
                    None => sale_wire::DealStatus::DEAL_STATUS_UNSPECIFIED,
                };
                Ok(sale_wire::DealEvent {
                    deal_version: row.deal_version,
                    kind: kind.into(),
                    pipeline_id: row.pipeline_id,
                    previous_stage_id: row.previous_stage_id.unwrap_or_default(),
                    stage_id: row.stage_id,
                    previous_status: previous_status.into(),
                    status: status(&row.status)?.into(),
                    reason_code: row.reason_code.unwrap_or_default(),
                    actor: row.actor,
                    request_id: row.request_id,
                    occurred_at: Some(ContactPointV2Service::timestamp(
                        row.occurred_at_unix_nanos.parse::<i64>().map_err(|_| {
                            ConnectError::internal("invalid persisted Sales event timestamp")
                        })?,
                    ))
                    .into(),
                    catalog_version: row.catalog_version.unwrap_or_default(),
                    linked_task_id: row.linked_task_id.unwrap_or_default(),
                    ..Default::default()
                })
            })
            .collect::<std::result::Result<Vec<_>, ConnectError>>()?;
        RpcResponse::ok(sale_wire::ListDealEventsResponse {
            events,
            page_info: Some(core_v2::PageInfo {
                next_page_token: next.unwrap_or_default(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn get_deal(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::GetDealRequest>,
    ) -> ServiceResult<sale_wire::GetDealResponse> {
        let input = req.to_owned_message();
        let id = Self::id(&input.deal_id)?;
        let db = self.d1()?;
        let deal = sales_deals::load(&db, &self.tenant, &id)
            .await
            .map_err(|_| {
                diagnostics::read_failure(
                    &self.request_id,
                    "sales_deal_read",
                    diagnostics::ReadFailureReason::InternalReadError,
                );
                ConnectError::internal("Sales original D1 read failed")
            })?
            .ok_or_else(|| ConnectError::not_found("SALES_DEAL_NOT_FOUND"))?;
        let confirmed_mutation = match input.receipt_idempotency_key.as_deref() {
            None => None,
            Some(key) if (8..=120).contains(&key.len()) => {
                sales_deals::confirmed_receipt(&db, &self.tenant, &id, key)
                    .await
                    .map_err(|error| {
                        diagnostics::read_failure(&self.request_id, "sales_receipt_lookup", error);
                        ConnectError::unavailable("SALES_RECEIPT_READ_FAILED")
                    })?
                    .map(|(original, version)| core_v2::MutationCommitEvidence {
                        request_id: self.request_id.clone(),
                        resource_version: version,
                        original_request_id: original,
                        ..Default::default()
                    })
            }
            Some(_) => return Err(ConnectError::invalid_argument("IDEMPOTENCY_KEY_REQUIRED")),
        };
        RpcResponse::ok(sale_wire::GetDealResponse {
            deal: Some(Self::convert(&deal)?).into(),
            confirmed_mutation: confirmed_mutation.into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn list_deals(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::ListDealsRequest>,
    ) -> ServiceResult<sale_wire::ListDealsResponse> {
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
            Self::id(after)?;
        }
        let pipeline = req.pipeline_id;
        if let Some(id) = pipeline {
            crm_sales::PipelineId::try_new(id)
                .map_err(|_| ConnectError::invalid_argument("invalid pipeline filter"))?;
        }
        let status = match req.status {
            None => None,
            Some(s) => Some(match s {
                x if x == sale_wire::DealStatus::DEAL_STATUS_OPEN => DomainDealStatus::Open,
                x if x == sale_wire::DealStatus::DEAL_STATUS_WON => DomainDealStatus::Won,
                x if x == sale_wire::DealStatus::DEAL_STATUS_LOST => DomainDealStatus::Lost,
                _ => return Err(ConnectError::invalid_argument("invalid DealStatus filter")),
            }),
        };
        let account = req.account_id;
        if let Some(id) = account {
            AccountV2Service::valid_id(id)?;
        }
        let input = req.to_owned_message();
        let owner_filter = Self::owner_scope(input.owner_filter.as_option())?;
        let (list, next) = sales_deals::list_page_typed(
            &self.d1()?,
            &self.tenant,
            sales_deals::DealListQuery {
                after,
                limit: size as usize,
                pipeline,
                status,
                account,
                search: wire_search(req.search)?,
                owner_filter: owner_filter.as_ref(),
            },
        )
        .await
        .map_err(|_| {
            diagnostics::read_failure(
                &self.request_id,
                "sales_deal_list",
                diagnostics::ReadFailureReason::InternalReadError,
            );
            ConnectError::internal("Sales original D1 list failed")
        })?;
        let deals = list
            .iter()
            .map(Self::convert)
            .collect::<std::result::Result<Vec<_>, ConnectError>>()?;
        RpcResponse::ok(sale_wire::ListDealsResponse {
            deals,
            page_info: Some(core_v2::PageInfo {
                next_page_token: next.unwrap_or_default(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }
}

impl SalesV2Service {
    fn coverage_owner(
        value: Option<&core_v2::ActorOrTeamOwner>,
    ) -> std::result::Result<crm_sales::SalesOwnerScope, ConnectError> {
        Self::owner_scope(value)?
            .ok_or_else(|| ConnectError::invalid_argument("SALES_COVERAGE_OWNER_REQUIRED"))
    }

    fn territory_draft(
        id: String,
        cfg: &sale_wire::TerritoryConfiguration,
    ) -> std::result::Result<sales_territories::Record, ConnectError> {
        Ok(sales_territories::Record::Territory(crm_sales::SalesTerritory {
            territory_id: id,
            name: cfg.name.clone(),
            parent_id: cfg.parent_id.clone(),
            default_owner: Self::coverage_owner(cfg.default_owner.as_option())?,
            archived: cfg.archived,
            version: 1,
        }))
    }

    fn assignment_draft(
        id: String,
        cfg: &sale_wire::SalesAccountAssignmentConfiguration,
    ) -> std::result::Result<sales_territories::Record, ConnectError> {
        use crm_sales::SalesOverlayRole as Domain;
        use sale_wire::SalesOverlayRole as Wire;
        let overlays = cfg
            .overlays
            .iter()
            .map(|item| {
                let role = match item.role {
                    v if v == Wire::SALES_OVERLAY_ROLE_SPECIALIST => Domain::Specialist,
                    v if v == Wire::SALES_OVERLAY_ROLE_ACCOUNT_SUPPORT => Domain::AccountSupport,
                    v if v == Wire::SALES_OVERLAY_ROLE_EXECUTIVE_SPONSOR => Domain::ExecutiveSponsor,
                    _ => return Err(ConnectError::invalid_argument("SALES_OVERLAY_ROLE_INVALID")),
                };
                Ok(crm_sales::SalesAccountOverlay {
                    role,
                    owner: Self::coverage_owner(item.owner.as_option())?,
                })
            })
            .collect::<std::result::Result<Vec<_>, ConnectError>>()?;
        Ok(sales_territories::Record::Assignment(
            crm_sales::SalesAccountAssignment {
                account_id: id,
                territory_id: cfg.territory_id.clone(),
                primary_owner: Self::coverage_owner(cfg.primary_owner.as_option())?,
                overlays,
                archived: cfg.archived,
                version: 1,
            },
        ))
    }

    fn territory_wire(
        record: sales_territories::Record,
    ) -> std::result::Result<sale_wire::SalesTerritory, ConnectError> {
        let sales_territories::Record::Territory(value) = record else {
            return Err(ConnectError::internal("Sales coverage kind mismatch"));
        };
        Ok(sale_wire::SalesTerritory {
            territory_id: value.territory_id,
            version: value.version,
            configuration: Some(sale_wire::TerritoryConfiguration {
                name: value.name,
                parent_id: value.parent_id,
                default_owner: Some(Self::owner_wire(&value.default_owner)).into(),
                archived: value.archived,
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }

    fn assignment_wire(
        record: sales_territories::Record,
    ) -> std::result::Result<sale_wire::SalesAccountAssignment, ConnectError> {
        use crm_sales::SalesOverlayRole as Domain;
        use sale_wire::SalesOverlayRole as Wire;
        let sales_territories::Record::Assignment(value) = record else {
            return Err(ConnectError::internal("Sales coverage kind mismatch"));
        };
        Ok(sale_wire::SalesAccountAssignment {
            account_id: value.account_id,
            version: value.version,
            configuration: Some(sale_wire::SalesAccountAssignmentConfiguration {
                territory_id: value.territory_id,
                primary_owner: Some(Self::owner_wire(&value.primary_owner)).into(),
                overlays: value
                    .overlays
                    .into_iter()
                    .map(|item| sale_wire::SalesAccountOverlay {
                        owner: Some(Self::owner_wire(&item.owner)).into(),
                        role: match item.role {
                            Domain::Specialist => Wire::SALES_OVERLAY_ROLE_SPECIALIST,
                            Domain::AccountSupport => Wire::SALES_OVERLAY_ROLE_ACCOUNT_SUPPORT,
                            Domain::ExecutiveSponsor => Wire::SALES_OVERLAY_ROLE_EXECUTIVE_SPONSOR,
                        }
                        .into(),
                        ..Default::default()
                    })
                    .collect(),
                archived: value.archived,
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }

    fn coverage_receipt(&self, saved: &sales_territories::Saved) -> core_v2::MutationReceipt {
        core_v2::MutationReceipt {
            request_id: self.request_id.clone(),
            original_request_id: saved.original_request_id.clone(),
            resource_version: saved.record.version(),
            replayed: saved.replayed,
            ..Default::default()
        }
    }
}
