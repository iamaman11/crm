use super::*;
use crate::{diagnostics, sales_leads};
use crm_module_sdk::RecordId;
use crm_sales::lead::{Lead, LeadDecision, LeadStatus};

pub(super) struct LeadV2Service(pub SalesV2Service);

impl OwnedService for LeadV2Service {
    const OWNER: &'static str = crm_sales::MODULE_ID;
}

fn lead_id(raw: &str) -> std::result::Result<RecordId, ConnectError> {
    if raw != raw.trim() || raw.contains('/') {
        return Err(ConnectError::invalid_argument("SALES_LEAD_ID_INVALID"));
    }
    RecordId::try_new(raw).map_err(|_| ConnectError::invalid_argument("SALES_LEAD_ID_INVALID"))
}

fn to_wire(lead: Lead) -> sale_wire::Lead {
    use sale_wire::LeadStatus as Wire;
    sale_wire::Lead {
        lead_id: lead.id.as_str().to_owned(),
        original_party_id: lead.original_party_id.as_str().to_owned(),
        name: lead.name,
        status: match lead.status {
            LeadStatus::New => Wire::LEAD_STATUS_NEW,
            LeadStatus::Qualified => Wire::LEAD_STATUS_QUALIFIED,
            LeadStatus::Rejected => Wire::LEAD_STATUS_REJECTED,
            LeadStatus::Converted => Wire::LEAD_STATUS_CONVERTED,
        }
        .into(),
        reason: lead.reason,
        deal_id: lead
            .deal_id
            .map(|id| id.as_str().to_owned())
            .unwrap_or_default(),
        version: lead.version,
        ..Default::default()
    }
}

impl LeadV2Service {
    fn entry(&self, lead: Lead) -> sale_wire::LeadEntry {
        let decide = self.0.admin && lead.status.can_decide();
        let convert = self.0.admin && lead.status.can_convert();
        sale_wire::LeadEntry {
            lead: to_wire(lead).into(),
            capabilities: sale_wire::LeadCapabilities {
                qualify: decide,
                reject: decide,
                convert,
                ..Default::default()
            }
            .into(),
            ..Default::default()
        }
    }

    fn context<'a>(&'a self, db: &'a D1Database, key: &'a str) -> sales_leads::Context<'a> {
        sales_leads::Context {
            db,
            key,
            tenant: &self.0.tenant,
            actor: &self.0.actor,
            request_id: &self.0.request_id,
        }
    }

    fn write_result(
        &self,
        operation: &'static str,
        result: Result<crate::d1_write::WriteResult<sales_leads::Saved>>,
    ) -> std::result::Result<(sale_wire::Lead, core_v2::MutationReceipt), ConnectError> {
        let saved = result
            .map_err(|_| diagnostics::mutation_preparation_failure(operation, &self.0.request_id))?
            .map_err(|failure| {
                diagnostics::mutation_error(operation, &self.0.request_id, failure)
            })?;
        let receipt = diagnostics::receipt(
            operation,
            &self.0.request_id,
            Some(&saved.original_request_id),
            saved.lead.version,
            saved.replayed,
            saved.d1_statement_changes,
        );
        Ok((to_wire(saved.lead), receipt))
    }

    async fn decide(
        &self,
        id: &str,
        pre: Option<&core_v2::MutationPrecondition>,
        reason: &str,
        decision: LeadDecision,
        operation: &'static str,
    ) -> std::result::Result<(sale_wire::Lead, core_v2::MutationReceipt), ConnectError> {
        let id = lead_id(id)?;
        let (key, version) = wire_precondition(pre, false)?;
        let db = self.0.d1()?;
        let ctx = self.context(&db, &key);
        self.write_result(
            operation,
            sales_leads::decide(
                &ctx,
                id.as_str(),
                version.unwrap_or_default(),
                decision,
                reason,
            )
            .await,
        )
    }
}

impl sale_wire::LeadService for LeadV2Service {
    #[worker::send]
    async fn create_lead(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::CreateLeadRequest>,
    ) -> ServiceResult<sale_wire::CreateLeadResponse> {
        let input = req.to_owned_message();
        let (key, _) = wire_precondition(input.precondition.as_option(), true)?;
        let id = lead_id(&input.lead_id)?;
        let party = RecordId::try_new(input.party_id)
            .map_err(|_| ConnectError::invalid_argument("SALES_LEAD_PARTY_INVALID"))?;
        let db = self.0.d1()?;
        let (lead, receipt) = self.write_result(
            "CreateLead",
            sales_leads::create(&self.context(&db, &key), id, party, &input.name).await,
        )?;
        RpcResponse::ok(sale_wire::CreateLeadResponse {
            lead: lead.into(),
            receipt: receipt.into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn get_lead(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::GetLeadRequest>,
    ) -> ServiceResult<sale_wire::GetLeadResponse> {
        let input = req.to_owned_message();
        let id = lead_id(&input.lead_id)?;
        let db = self.0.d1()?;
        let lead = sales_leads::load(&db, &self.0.tenant, id.as_str())
            .await
            .map_err(|e| diagnostics::read_error(&self.0.request_id, &e))?
            .ok_or_else(|| ConnectError::not_found("SALES_LEAD_NOT_FOUND"))?;
        let confirmed_mutation = match receipt_key(input.receipt_idempotency_key.as_deref())? {
            Some(key) => sales_leads::confirmed(&self.context(&db, key), id.as_str())
                .await
                .map_err(|_| ConnectError::unavailable("SALES_LEAD_RECEIPT_READ_FAILED"))?
                .map(
                    |(version, original_request_id)| core_v2::MutationCommitEvidence {
                        request_id: self.0.request_id.clone(),
                        resource_version: version,
                        original_request_id,
                        ..Default::default()
                    },
                ),
            None => None,
        };
        RpcResponse::ok(sale_wire::GetLeadResponse {
            entry: self.entry(lead).into(),
            confirmed_mutation: confirmed_mutation.into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn list_leads(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::ListLeadsRequest>,
    ) -> ServiceResult<sale_wire::ListLeadsResponse> {
        let input = req.to_owned_message();
        let page = input.page.as_option();
        let size = page
            .map(|page| page.page_size)
            .filter(|size| *size != 0)
            .unwrap_or(25)
            .min(100) as usize;
        let cursor = page
            .map(|page| page.page_token.as_str())
            .unwrap_or_default();
        if !cursor.is_empty() {
            lead_id(cursor)?;
        }
        let (leads, cursor) = sales_leads::list(&self.0.d1()?, &self.0.tenant, cursor, size)
            .await
            .map_err(|e| diagnostics::read_error(&self.0.request_id, &e))?;
        RpcResponse::ok(sale_wire::ListLeadsResponse {
            entries: leads.into_iter().map(|lead| self.entry(lead)).collect(),
            page_info: core_v2::PageInfo {
                next_page_token: cursor.unwrap_or_default(),
                ..Default::default()
            }
            .into(),
            can_create: self.0.admin,
            ..Default::default()
        })
    }

    #[worker::send]
    async fn qualify_lead(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::QualifyLeadRequest>,
    ) -> ServiceResult<sale_wire::QualifyLeadResponse> {
        let input = req.to_owned_message();
        let (lead, receipt) = self
            .decide(
                &input.lead_id,
                input.precondition.as_option(),
                &input.reason,
                LeadDecision::Qualify,
                "QualifyLead",
            )
            .await?;
        RpcResponse::ok(sale_wire::QualifyLeadResponse {
            lead: lead.into(),
            receipt: receipt.into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn reject_lead(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::RejectLeadRequest>,
    ) -> ServiceResult<sale_wire::RejectLeadResponse> {
        let input = req.to_owned_message();
        let (lead, receipt) = self
            .decide(
                &input.lead_id,
                input.precondition.as_option(),
                &input.reason,
                LeadDecision::Reject,
                "RejectLead",
            )
            .await?;
        RpcResponse::ok(sale_wire::RejectLeadResponse {
            lead: lead.into(),
            receipt: receipt.into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn convert_lead(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::ConvertLeadRequest>,
    ) -> ServiceResult<sale_wire::ConvertLeadResponse> {
        let input = req.to_owned_message();
        let id = lead_id(&input.lead_id)?;
        let (key, version) = wire_precondition(input.precondition.as_option(), false)?;
        let deal = input
            .deal
            .into_option()
            .ok_or_else(|| ConnectError::invalid_argument("SALES_LEAD_DEAL_REQUIRED"))?;
        if deal.precondition.is_set() {
            return Err(ConnectError::invalid_argument(
                "SALES_LEAD_USE_OUTER_PRECONDITION",
            ));
        }
        let deal = super::sales_handlers::deal_create_input(deal)?;
        let db = self.0.d1()?;
        let conversion = sales_leads::ConversionInput {
            id: id.as_str(),
            version: version.unwrap_or_default(),
        };
        let (lead, receipt) = self.write_result(
            "ConvertLead",
            sales_deals::create_from_lead(&self.context(&db, &key), deal, conversion).await,
        )?;
        RpcResponse::ok(sale_wire::ConvertLeadResponse {
            lead: lead.into(),
            receipt: receipt.into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn list_lead_events(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, sale_wire::ListLeadEventsRequest>,
    ) -> ServiceResult<sale_wire::ListLeadEventsResponse> {
        let id = lead_id(&req.to_owned_message().lead_id)?;
        let events = sales_leads::history(&self.0.d1()?, &self.0.tenant, id.as_str())
            .await
            .map_err(|e| diagnostics::read_error(&self.0.request_id, &e))?;
        if events.is_empty() {
            return Err(ConnectError::not_found("SALES_LEAD_NOT_FOUND"));
        }
        RpcResponse::ok(sale_wire::ListLeadEventsResponse {
            events: events
                .into_iter()
                .map(|(snapshot, request_id, seconds)| sale_wire::LeadEvent {
                    snapshot: to_wire(snapshot).into(),
                    request_id,
                    occurred_at: Timestamp {
                        seconds,
                        ..Default::default()
                    }
                    .into(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        })
    }
}
