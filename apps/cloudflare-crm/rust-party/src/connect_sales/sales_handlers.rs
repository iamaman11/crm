//! Generated Connect trait for the existing SalesV2Service Rust domain owner.
//! Access, tenant, and the single native router are defined by the parent.
use super::*;

impl sale_wire::DealService for SalesV2Service{
    #[worker::send]
    async fn create_deal(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,sale_wire::CreateDealRequest>,
    )->ServiceResult<sale_wire::CreateDealResponse>{
        let input=req.to_owned_message();
        let id=Self::id(&input.deal_id)?;
        let (key,_)=AccountV2Service::precondition(input.precondition.as_option(),true)?;
        let stage=input.stage.as_option()
            .ok_or_else(||ConnectError::invalid_argument("stage required"))?;
        let (pipeline_id,stage_id,ordinal)=Self::stage(stage)?;
        let probability_bps=u16::try_from(input.probability_basis_points)
            .map_err(|_|ConnectError::invalid_argument("invalid probability basis points"))?;
        let (amount_minor_units,currency)=match input.amount.as_option(){
            Some(money)=>(Some(money.minor_units),Some(money.currency_code.clone())),
            None=>(None,None),
        };
        let db=self.d1()?;
        let saved=sales_deals::create_typed(sales_deals::CreateInput{
            id,name:input.name,account_id:input.account_id,pipeline_id,stage_id,
            ordinal,probability_bps,amount_minor_units,currency,
        },&key,&db,&self.tenant,&self.actor,&self.request_id).await
            .map_err(|_|ConnectError::internal("Sales original D1 create failed"))?
            .map_err(Self::failed)?;
        RpcResponse::ok(sale_wire::CreateDealResponse{
            deal:Some(Self::convert(&saved.deal)?).into(),
            receipt:Some(self.receipt(&saved)).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn update_deal(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,sale_wire::UpdateDealRequest>,
    )->ServiceResult<sale_wire::UpdateDealResponse>{
        let input=req.to_owned_message();
        let id=Self::id(&input.deal_id)?;
        let (key,version)=AccountV2Service::precondition(input.precondition.as_option(),false)?;
        let (name,clear_name)=match input.name.as_option(){
            None=>(None,None),
            Some(patch)=>match patch.operation.as_ref(){
                Some(core_v2::string_patch::Operation::Set(name))=>
                    (Some(name.clone()),None),
                Some(core_v2::string_patch::Operation::Clear(true))=>(None,Some(true)),
                _=>return Err(ConnectError::invalid_argument("invalid StringPatch")),
            }
        };
        let (amount_minor_units,currency,clear_amount)=match input.amount.as_option(){
            None=>(None,None,None),
            Some(patch)=>match patch.operation.as_ref(){
                Some(core_v2::money_patch::Operation::Set(money))=>
                    (Some(money.minor_units),Some(money.currency_code.clone()),None),
                Some(core_v2::money_patch::Operation::Clear(true))=>
                    (None,None,Some(true)),
                _=>return Err(ConnectError::invalid_argument("invalid MoneyPatch")),
            }
        };
        let probability_bps=input.probability_basis_points
            .map(|v|u16::try_from(v)
                .map_err(|_|ConnectError::invalid_argument("invalid probability basis points")))
            .transpose()?;
        let saved=sales_deals::update_typed(sales_deals::UpdateInput{
            expected_version:version.unwrap_or_default(),name,clear_name,
            amount_minor_units,currency,clear_amount,probability_bps,
        },&key,&self.d1()?,&self.tenant,&id,&self.request_id).await
            .map_err(|_|ConnectError::internal("Sales original D1 update failed"))?
            .map_err(Self::failed)?;
        RpcResponse::ok(sale_wire::UpdateDealResponse{
            deal:Some(Self::convert(&saved.deal)?).into(),
            receipt:Some(self.receipt(&saved)).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn advance_stage(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,sale_wire::AdvanceStageRequest>,
    )->ServiceResult<sale_wire::AdvanceStageResponse>{
        let input=req.to_owned_message();
        let id=Self::id(&input.deal_id)?;
        let (key,version)=AccountV2Service::precondition(input.precondition.as_option(),false)?;
        let stage=input.target_stage.as_option()
            .ok_or_else(||ConnectError::invalid_argument("target_stage required"))?;
        let (pipeline_id,stage_id,ordinal)=Self::stage(stage)?;
        let status=Self::deal_status(input.target_status)?;
        let advanced=sales_deals::advance_typed(sales_deals::StageInput{
            expected_version:version.unwrap_or_default(),
            pipeline_id,stage_id,ordinal,status,
            close_reason_code:input.close_reason_code,
        },&key,&self.d1()?,&self.tenant,&id,&self.actor,&self.request_id).await
            .map_err(|_|ConnectError::internal("Sales original D1 stage advance failed"))?
            .map_err(Self::failed)?;
        RpcResponse::ok(sale_wire::AdvanceStageResponse{
            deal:Some(Self::convert(&advanced.saved.deal)?).into(),
            receipt:Some(self.receipt(&advanced.saved)).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn get_deal(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,sale_wire::GetDealRequest>,
    )->ServiceResult<sale_wire::GetDealResponse>{
        let id=Self::id(&req.deal_id)?;
        let deal=sales_deals::load(&self.d1()?,&self.tenant,&id).await
            .map_err(|_|ConnectError::internal("Sales original D1 read failed"))?
            .ok_or_else(||ConnectError::not_found("SALES_DEAL_NOT_FOUND"))?;
        RpcResponse::ok(sale_wire::GetDealResponse{
            deal:Some(Self::convert(&deal)?).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn list_deals(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,sale_wire::ListDealsRequest>,
    )->ServiceResult<sale_wire::ListDealsResponse>{
        let size=if req.page.page_size==0{25}else{req.page.page_size};
        if size>100{return Err(ConnectError::invalid_argument("page_size exceeds 100"));}
        let after=&req.page.page_token;
        if !after.is_empty(){Self::id(after)?;}
        let pipeline=req.pipeline_id.as_deref();
        if let Some(id)=pipeline{
            crm_sales::PipelineId::try_new(id)
                .map_err(|_|ConnectError::invalid_argument("invalid pipeline filter"))?;
        }
        let status=match req.status {
            None=>None,Some(s)=>Some(match s{
                x if x==sale_wire::DealStatus::DEAL_STATUS_OPEN=>DomainDealStatus::Open,
                x if x==sale_wire::DealStatus::DEAL_STATUS_WON=>DomainDealStatus::Won,
                x if x==sale_wire::DealStatus::DEAL_STATUS_LOST=>DomainDealStatus::Lost,
                _=>return Err(ConnectError::invalid_argument("invalid DealStatus filter")),
            })
        };
        let (list,next)=sales_deals::list_page_typed(
            &self.d1()?,&self.tenant,after,size as usize,pipeline,status).await
            .map_err(|_|ConnectError::internal("Sales original D1 list failed"))?;
        let deals=list.iter().map(Self::convert)
            .collect::<std::result::Result<Vec<_>,ConnectError>>()?;
        RpcResponse::ok(sale_wire::ListDealsResponse{
            deals,page_info:Some(core_v2::PageInfo{
                next_page_token:next.unwrap_or_default(),..Default::default()
            }).into(),..Default::default()
        })
    }
}
