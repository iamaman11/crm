//! Generated Connect trait for the existing PartyV3Service Rust domain owner.
//! Access, tenant, and the single native router are defined by the parent.
use super::*;

impl party_v3::PartyService for PartyV3Service {
    #[worker::send]
    async fn create_party(
        &self,_ctx:RequestContext,req:ServiceRequest<'_,party_v3::CreatePartyRequest>,
    )->ServiceResult<party_v3::CreatePartyResponse>{
        let input=req.to_owned_message();
        let id=Self::valid_id(&input.party_id)?;
        let kind=match input.kind {
            x if x==party_v3::PartyKind::PARTY_KIND_PERSON=>DomainPartyKind::Person,
            x if x==party_v3::PartyKind::PARTY_KIND_ORGANIZATION=>DomainPartyKind::Organization,
            _=>return Err(ConnectError::invalid_argument("INVALID_KIND")),
        };
        let pre=input.precondition.as_option()
            .ok_or_else(||ConnectError::invalid_argument("mutation precondition required"))?;
        if pre.expected_version.is_some() {
            return Err(ConnectError::invalid_argument("create must not specify expected_version"));
        }
        let saved=parties::create_typed(&self.d1()?,&self.tenant,
            id.as_str().to_owned(),kind,input.display_name,
            &pre.idempotency_key,&self.request_id).await
            .map_err(|_|ConnectError::internal("Party D1 create failed"))?
            .map_err(Self::mutation_error)?;
        RpcResponse::ok(party_v3::CreatePartyResponse {
            party:Some(Self::convert(&saved.party)?).into(),
            receipt:Some(self.receipt(&saved)?).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn update_party(
        &self,_ctx:RequestContext,req:ServiceRequest<'_,party_v3::UpdatePartyRequest>,
    )->ServiceResult<party_v3::UpdatePartyResponse>{
        let input=req.to_owned_message();
        let id=Self::valid_id(&input.party_id)?;
        let pre=input.precondition.as_option()
            .ok_or_else(||ConnectError::invalid_argument("mutation precondition required"))?;
        let version=pre.expected_version
            .ok_or_else(||ConnectError::invalid_argument("expected_version required"))?;
        if version<1 {return Err(ConnectError::invalid_argument("invalid expected_version"));}
        let saved=parties::update_typed(&self.d1()?,&self.tenant,
            id.as_str(),input.display_name,version,&pre.idempotency_key,
            &self.request_id).await
            .map_err(|_|ConnectError::internal("Party D1 update failed"))?
            .map_err(Self::mutation_error)?;
        RpcResponse::ok(party_v3::UpdatePartyResponse {
            party:Some(Self::convert(&saved.party)?).into(),
            receipt:Some(self.receipt(&saved)?).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn get_party(
        &self,_ctx:RequestContext,req:ServiceRequest<'_,party_v3::GetPartyRequest>,
    )->ServiceResult<party_v3::GetPartyResponse>{
        let id=Self::valid_id(&req.party_id)?;
        let party=parties::stored(&self.d1()?,&self.tenant,id.as_str()).await
            .map_err(|_|ConnectError::internal("Party D1 get failed"))?
            .ok_or_else(||ConnectError::not_found("PARTIES_NOT_FOUND"))?;
        RpcResponse::ok(party_v3::GetPartyResponse {
            party:Some(Self::convert(&party)?).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn list_parties(
        &self,_ctx:RequestContext,req:ServiceRequest<'_,party_v3::ListPartiesRequest>,
    )->ServiceResult<party_v3::ListPartiesResponse>{
        let size=if req.page.page_size==0 {25} else {req.page.page_size};
        if size>100 {return Err(ConnectError::invalid_argument("page_size exceeds 100"));}
        let after=&req.page.page_token;
        if !after.is_empty(){Self::valid_id(after)?;}
        let filter=match req.kind {
            None=>None,
            Some(x) if x==party_v3::PartyKind::PARTY_KIND_PERSON=>Some(DomainPartyKind::Person),
            Some(x) if x==party_v3::PartyKind::PARTY_KIND_ORGANIZATION=>
                Some(DomainPartyKind::Organization),
            _=>return Err(ConnectError::invalid_argument("invalid PartyKind filter")),
        };
        let (parties,next)=parties::list_page_typed(&self.d1()?,&self.tenant,
            after,size as usize,filter).await
            .map_err(|_|ConnectError::internal("Party D1 list failed"))?;
        let mut items=Vec::with_capacity(parties.len());
        for party in &parties {items.push(Self::convert(party)?);}
        RpcResponse::ok(party_v3::ListPartiesResponse {
            parties:items,
            page_info:Some(core_v2::PageInfo {
                next_page_token:next.unwrap_or_default(),..Default::default()
            }).into(),
            ..Default::default()
        })
    }
}
