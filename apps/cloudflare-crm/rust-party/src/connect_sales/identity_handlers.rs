//! Generated Connect trait for the existing IdentityV2Service Rust domain owner.
//! Access, tenant, and the single native router are defined by the parent.
use super::*;

impl identity_wire::IdentityResolutionService for IdentityV2Service{
    #[worker::send]
    async fn find_shared_email_candidates(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,identity_wire::FindSharedEmailCandidatesRequest>,
    )->ServiceResult<identity_wire::FindSharedEmailCandidatesResponse>{
        let size=if req.page.page_size==0{25}else{req.page.page_size};
        if size>100{return Err(ConnectError::invalid_argument("page_size exceeds 100"));}
        // Numeric offset is an opaque cursor over the bounded, deterministic
        // canonical pair order. Never LIMIT raw contacts before domain filtering.
        let after=&req.page.page_token;
        let offset=if after.is_empty(){0}else{
            let parsed=after.parse::<usize>()
                .map_err(|_|ConnectError::invalid_argument("invalid page_token"))?;
            if parsed==0 || parsed.to_string()!=*after{
                return Err(ConnectError::invalid_argument("invalid page_token"));
            }
            parsed
        };
        let db=self.env.d1("DB")
            .map_err(|_|ConnectError::internal("D1 unavailable"))?;
        let candidates=match identity_resolution::candidates_typed(&db,&self.tenant).await
            .map_err(|_|ConnectError::internal("Identity original owner read failed"))? {
            Ok(items)=>items,
            Err(identity_resolution::CandidateError::ScanLimit)=>
                return Err(ConnectError::unavailable("IDENTITY_RESOLUTION_SCAN_LIMIT_REACHED")),
            Err(identity_resolution::CandidateError::SourceChanged)=>
                return Err(ConnectError::aborted("IDENTITY_RESOLUTION_SOURCE_CHANGED")),
        };
        let mut items=Vec::new();
        for candidate in candidates.iter().skip(offset).take(size as usize){
            items.push(identity_wire::SharedEmailCandidate{
                first_party:Some(PartyV3Service::convert(&candidate.first)?).into(),
                second_party:Some(PartyV3Service::convert(&candidate.second)?).into(),
                first_contact_point_id:candidate.first_contact_point_id.clone(),
                second_contact_point_id:candidate.second_contact_point_id.clone(),
                both_contacts_verified:candidate.both_verified,
                ..Default::default()
            });
        }
        let next=if offset.saturating_add(size as usize)<candidates.len(){
            (offset+size as usize).to_string()
        }else{String::new()};
        RpcResponse::ok(identity_wire::FindSharedEmailCandidatesResponse{
            candidates:items,
            page_info:Some(core_v2::PageInfo{
                next_page_token:next,..Default::default()
            }).into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn compare_parties(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,identity_wire::ComparePartiesRequest>,
    )->ServiceResult<identity_wire::ComparePartiesResponse>{
        let first=PartyV3Service::valid_id(&req.first_party_id)?;
        let second=PartyV3Service::valid_id(&req.second_party_id)?;
        let pair=CanonicalPartyPair::try_new(
            PartyReference::try_new(first.as_str())
                .map_err(|_|ConnectError::invalid_argument("invalid first_party_id"))?,
            PartyReference::try_new(second.as_str())
                .map_err(|_|ConnectError::invalid_argument("invalid second_party_id"))?,
        ).map_err(|_|ConnectError::invalid_argument("IDENTITY_RESOLUTION_SELF_PAIR_INVALID"))?;
        let db=self.env.d1("DB")
            .map_err(|_|ConnectError::internal("D1 unavailable"))?;
        let comparison=identity_resolution::compare_typed(
            &db,&self.tenant,&pair,first.as_str()).await
            .map_err(|_|ConnectError::internal("Identity original Party read failed"))?
            .ok_or_else(||ConnectError::not_found("PARTIES_NOT_FOUND"))?;
        RpcResponse::ok(identity_wire::ComparePartiesResponse{
            first_party:Some(PartyV3Service::convert(&comparison.first)?).into(),
            second_party:Some(PartyV3Service::convert(&comparison.second)?).into(),
            same_party_id:false,identity_confirmed:false,
            ..Default::default()
        })
    }
}
