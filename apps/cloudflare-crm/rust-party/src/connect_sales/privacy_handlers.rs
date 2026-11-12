//! Typed Connect mapping to the existing PrivacyCase owner and its D1 operations.
use super::*;
use super::super::customer_privacy;
use crm_customer_privacy::{PrivacyCase,PrivacyCaseKind,PrivacyCaseStatus};

pub(super) struct PrivacyV2Service {
    pub(super) env:Arc<Env>,
    pub(super) tenant:String,
    pub(super) actor:String,
    pub(super) request_id:String,
}
impl PrivacyV2Service {
    fn d1(&self)->std::result::Result<D1Database,ConnectError>{
        self.env.d1("DB").map_err(|_|ConnectError::internal("D1 unavailable"))
    }
    fn id(raw:&str)->std::result::Result<crm_module_sdk::RecordId,ConnectError>{
        if raw!=raw.trim()||raw.contains('/'){
            return Err(ConnectError::invalid_argument("PRIVACY_CASE_ID_INVALID"));
        }
        crm_module_sdk::RecordId::try_new(raw.to_owned())
            .map_err(|_|ConnectError::invalid_argument("PRIVACY_CASE_ID_INVALID"))
    }
    fn convert(case:&PrivacyCase)->std::result::Result<privacy_wire::PrivacyCase,ConnectError>{
        let kind=match case.kind(){
            PrivacyCaseKind::Access=>privacy_wire::PrivacyCaseKind::PRIVACY_CASE_KIND_ACCESS,
            PrivacyCaseKind::PortabilityExport=>privacy_wire::PrivacyCaseKind::PRIVACY_CASE_KIND_PORTABILITY_EXPORT,
            PrivacyCaseKind::RestrictProcessing=>privacy_wire::PrivacyCaseKind::PRIVACY_CASE_KIND_RESTRICT_PROCESSING,
            PrivacyCaseKind::Erasure=>privacy_wire::PrivacyCaseKind::PRIVACY_CASE_KIND_ERASURE,
        };
        let status=match case.status(){
            PrivacyCaseStatus::Draft=>privacy_wire::PrivacyCaseStatus::PRIVACY_CASE_STATUS_DRAFT,
            PrivacyCaseStatus::Submitted=>privacy_wire::PrivacyCaseStatus::PRIVACY_CASE_STATUS_SUBMITTED,
            PrivacyCaseStatus::SubjectVerified=>privacy_wire::PrivacyCaseStatus::PRIVACY_CASE_STATUS_SUBJECT_VERIFIED,
            PrivacyCaseStatus::Scoping=>privacy_wire::PrivacyCaseStatus::PRIVACY_CASE_STATUS_SCOPING,
            PrivacyCaseStatus::Scoped=>privacy_wire::PrivacyCaseStatus::PRIVACY_CASE_STATUS_SCOPED,
            PrivacyCaseStatus::Planned=>privacy_wire::PrivacyCaseStatus::PRIVACY_CASE_STATUS_PLANNED,
            PrivacyCaseStatus::AwaitingApproval=>privacy_wire::PrivacyCaseStatus::PRIVACY_CASE_STATUS_AWAITING_APPROVAL,
            PrivacyCaseStatus::Executing=>privacy_wire::PrivacyCaseStatus::PRIVACY_CASE_STATUS_EXECUTING,
            PrivacyCaseStatus::Converging=>privacy_wire::PrivacyCaseStatus::PRIVACY_CASE_STATUS_CONVERGING,
            PrivacyCaseStatus::RescopeRequired=>privacy_wire::PrivacyCaseStatus::PRIVACY_CASE_STATUS_RESCOPE_REQUIRED,
            PrivacyCaseStatus::FailedRetryable(_)=>privacy_wire::PrivacyCaseStatus::PRIVACY_CASE_STATUS_FAILED_RETRYABLE,
            PrivacyCaseStatus::Completed=>privacy_wire::PrivacyCaseStatus::PRIVACY_CASE_STATUS_COMPLETED,
            PrivacyCaseStatus::PartiallyCompleted=>privacy_wire::PrivacyCaseStatus::PRIVACY_CASE_STATUS_PARTIALLY_COMPLETED,
            PrivacyCaseStatus::Denied=>privacy_wire::PrivacyCaseStatus::PRIVACY_CASE_STATUS_DENIED,
            PrivacyCaseStatus::Cancelled=>privacy_wire::PrivacyCaseStatus::PRIVACY_CASE_STATUS_CANCELLED,
            PrivacyCaseStatus::FailedTerminal=>privacy_wire::PrivacyCaseStatus::PRIVACY_CASE_STATUS_FAILED_TERMINAL,
        };
        Ok(privacy_wire::PrivacyCase{
            case_id:case.case_id().as_str().to_owned(),kind:kind.into(),status:status.into(),
            version:i64::try_from(case.version()).map_err(|_|ConnectError::internal("invalid PrivacyCase version"))?,
            subject_verified:case.subject_binding().is_some(),..Default::default()
        })
    }
    fn receipt(&self,saved:&customer_privacy::Saved)->std::result::Result<core_v2::MutationReceipt,ConnectError>{
        Ok(core_v2::MutationReceipt{
            request_id:self.request_id.clone(),replayed:saved.replayed,
            resource_version:i64::try_from(saved.case.version())
                .map_err(|_|ConnectError::internal("invalid PrivacyCase version"))?,
            ..Default::default()
        })
    }
    fn failed(error:customer_privacy::Failure)->ConnectError{
        match error.status{
            404=>ConnectError::not_found(error.code),
            409=>ConnectError::aborted(error.code),
            412=>ConnectError::failed_precondition(error.code),
            422=>ConnectError::invalid_argument(error.code),
            _=>ConnectError::internal(error.code),
        }
    }
    async fn transition(&self,id:&str,pre:Option<&core_v2::MutationPrecondition>,action:customer_privacy::Transition)
        ->std::result::Result<customer_privacy::Saved,ConnectError>{
        let id=Self::id(id)?;
        let (key,version)=wire_precondition(pre,false)?;
        let version=u64::try_from(version.unwrap_or_default())
            .map_err(|_|ConnectError::invalid_argument("invalid expected version"))?;
        customer_privacy::transition_typed(id.as_str(),action,version,&key,
            &self.d1()?,&self.tenant,&self.request_id).await
            .map_err(|_|ConnectError::internal("PrivacyCase D1 transition failed"))?
            .map_err(Self::failed)
    }
}
impl privacy_wire::PrivacyCaseService for PrivacyV2Service {
    #[worker::send]
    async fn create_privacy_case(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,privacy_wire::CreatePrivacyCaseRequest>)
        ->ServiceResult<privacy_wire::CreatePrivacyCaseResponse>{
        let input=req.to_owned_message();
        let id=Self::id(&input.case_id)?;
        let (key,_)=wire_precondition(input.precondition.as_option(),true)?;
        let kind=match input.kind{
            x if x==privacy_wire::PrivacyCaseKind::PRIVACY_CASE_KIND_ACCESS=>PrivacyCaseKind::Access,
            x if x==privacy_wire::PrivacyCaseKind::PRIVACY_CASE_KIND_PORTABILITY_EXPORT=>PrivacyCaseKind::PortabilityExport,
            x if x==privacy_wire::PrivacyCaseKind::PRIVACY_CASE_KIND_RESTRICT_PROCESSING=>PrivacyCaseKind::RestrictProcessing,
            x if x==privacy_wire::PrivacyCaseKind::PRIVACY_CASE_KIND_ERASURE=>PrivacyCaseKind::Erasure,
            _=>return Err(ConnectError::invalid_argument("PRIVACY_CASE_KIND_INVALID")),
        };
        let saved=customer_privacy::create_typed(id,kind,&key,&self.d1()?,
            &self.tenant,&self.actor,&self.request_id).await
            .map_err(|_|ConnectError::internal("PrivacyCase D1 create failed"))?
            .map_err(Self::failed)?;
        RpcResponse::ok(privacy_wire::CreatePrivacyCaseResponse{
            privacy_case:Some(Self::convert(&saved.case)?).into(),
            receipt:Some(self.receipt(&saved)?).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn submit_privacy_case(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,privacy_wire::SubmitPrivacyCaseRequest>)
        ->ServiceResult<privacy_wire::SubmitPrivacyCaseResponse>{
        let input=req.to_owned_message();
        let saved=self.transition(&input.case_id,input.precondition.as_option(),customer_privacy::Transition::Submit).await?;
        RpcResponse::ok(privacy_wire::SubmitPrivacyCaseResponse{
            privacy_case:Some(Self::convert(&saved.case)?).into(),
            receipt:Some(self.receipt(&saved)?).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn cancel_privacy_case(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,privacy_wire::CancelPrivacyCaseRequest>)
        ->ServiceResult<privacy_wire::CancelPrivacyCaseResponse>{
        let input=req.to_owned_message();
        let saved=self.transition(&input.case_id,input.precondition.as_option(),customer_privacy::Transition::Cancel).await?;
        RpcResponse::ok(privacy_wire::CancelPrivacyCaseResponse{
            privacy_case:Some(Self::convert(&saved.case)?).into(),
            receipt:Some(self.receipt(&saved)?).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn get_privacy_case(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,privacy_wire::GetPrivacyCaseRequest>)
        ->ServiceResult<privacy_wire::GetPrivacyCaseResponse>{
        let id=Self::id(&req.case_id)?;
        let case=customer_privacy::stored(&self.d1()?,&self.tenant,id.as_str()).await
            .map_err(|_|ConnectError::internal("PrivacyCase D1 read failed"))?
            .ok_or_else(||ConnectError::not_found("PRIVACY_CASE_NOT_FOUND"))?;
        RpcResponse::ok(privacy_wire::GetPrivacyCaseResponse{
            privacy_case:Some(Self::convert(&case)?).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn list_privacy_cases(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,privacy_wire::ListPrivacyCasesRequest>)
        ->ServiceResult<privacy_wire::ListPrivacyCasesResponse>{
        let size=if req.page.page_size==0{25}else{req.page.page_size};
        if size>100{return Err(ConnectError::invalid_argument("page_size exceeds 100"));}
        let after=&req.page.page_token;
        if !after.is_empty(){Self::id(after)?;}
        let (cases,next)=customer_privacy::list_page_typed(&self.d1()?,&self.tenant,after,size as usize).await
            .map_err(|_|ConnectError::internal("PrivacyCase D1 list failed"))?;
        let cases=cases.iter().map(Self::convert).collect::<std::result::Result<Vec<_>,_>>()?;
        RpcResponse::ok(privacy_wire::ListPrivacyCasesResponse{
            cases,page_info:Some(core_v2::PageInfo{
                next_page_token:next.unwrap_or_default(),..Default::default()
            }).into(),..Default::default()
        })
    }
}
