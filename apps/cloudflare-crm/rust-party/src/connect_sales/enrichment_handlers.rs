//! Generated Connect traits mapped to the original immutable definition owner.
use super::*;
use super::super::customer_enrichment as owner;
use crm_customer_enrichment::{ProviderProfileVersion,MappingVersion,RawPayloadPolicy};

pub(super) struct EnrichmentV2Service {
    pub(super) env:Arc<Env>,
    pub(super) tenant:String,
    pub(super) actor:String,
    pub(super) request_id:String,
}
impl EnrichmentV2Service {
    fn d1(&self)->std::result::Result<D1Database,ConnectError>{
        self.env.d1("DB").map_err(|_|ConnectError::internal("D1 unavailable"))
    }
    fn failed(error:owner::Failure)->ConnectError{
        match error.status {
            404=>ConnectError::not_found(error.code),
            409=>ConnectError::aborted(error.code),
            422=>ConnectError::invalid_argument(error.code),
            _=>ConnectError::internal(error.code),
        }
    }
    fn id(raw:&str)->std::result::Result<(),ConnectError>{
        if raw.is_empty()||raw.len()>180||raw!=raw.trim()||raw.contains('/') {
            return Err(ConnectError::invalid_argument("ENRICHMENT_VERSION_ID_INVALID"));
        }
        Ok(())
    }
    fn millis(value:&Timestamp)->std::result::Result<u64,ConnectError>{
        // The owner uses Unix milliseconds. Reject lost precision and invalid WKT.
        if !(0..=253402300799).contains(&value.seconds)||
           !(0..1_000_000_000).contains(&value.nanos)||value.nanos%1_000_000!=0 {
            return Err(ConnectError::invalid_argument("timestamp must be valid nonnegative Unix milliseconds"));
        }
        Ok(value.seconds as u64*1000+value.nanos as u64/1_000_000)
    }
    fn timestamp(ms:u64)->std::result::Result<Timestamp,ConnectError>{
        let seconds=i64::try_from(ms/1000).map_err(|_|ConnectError::internal("invalid stored timestamp"))?;
        if seconds>253402300799{return Err(ConnectError::internal("invalid stored timestamp"));}
        Ok(Timestamp{seconds,nanos:((ms%1000)*1_000_000) as i32,..Default::default()})
    }
    fn profile(p:&ProviderProfileVersion)->std::result::Result<enrichment_wire::ProviderProfileVersion,ConnectError>{
        let policy=match p.raw_payload_policy(){
            RawPayloadPolicy::DigestOnly=>enrichment_wire::RawPayloadPolicy::RAW_PAYLOAD_POLICY_DIGEST_ONLY,
            RawPayloadPolicy::GovernedProtectedEvidence=>enrichment_wire::RawPayloadPolicy::RAW_PAYLOAD_POLICY_GOVERNED_PROTECTED_EVIDENCE,
        };
        Ok(enrichment_wire::ProviderProfileVersion{
            profile_version_id:p.version_id().as_str().to_owned(),
            provider_key:p.provider_key().to_owned(),adapter_kind:p.adapter_kind().to_owned(),
            adapter_contract_version:p.adapter_contract_version().to_owned(),
            purpose_codes:p.purpose_codes().to_vec(),license_id:p.license_id().to_owned(),
            permitted_use_class:p.permitted_use_class().to_owned(),residency_region:p.residency_region().to_owned(),
            retention_days:p.retention_days(),raw_payload_policy:policy.into(),
            credential_handle_aliases:p.credential_handle_aliases().to_vec(),
            effective_at:Some(Self::timestamp(p.effective_at_unix_ms())?).into(),
            expires_at:p.expires_at_unix_ms().map(Self::timestamp).transpose()?.into(),
            ..Default::default()
        })
    }
    fn mapping(m:&MappingVersion)->enrichment_wire::MappingVersion{
        enrichment_wire::MappingVersion{
            mapping_version_id:m.version_id().as_str().to_owned(),mapping_key:m.mapping_key().to_owned(),
            provider_profile_version_id:m.provider_profile_version_id().as_str().to_owned(),
            provider_response_field_path:m.provider_response_field_path().to_owned(),
            maximum_suggestions_per_response:m.maximum_suggestions_per_response(),
            confidence_required:m.confidence_required(),..Default::default()
        }
    }
    fn receipt(&self,replayed:bool)->core_v2::MutationReceipt{
        // Each content-addressed definition is immutable: its resource version is one.
        core_v2::MutationReceipt{request_id:self.request_id.clone(),resource_version:1,replayed,..Default::default()}
    }
    fn page(page:&core_v2::PageRequest)->std::result::Result<usize,ConnectError>{
        let size=if page.page_size==0{25}else{page.page_size};
        if size>100{return Err(ConnectError::invalid_argument("page_size exceeds 100"));}
        if !page.page_token.is_empty(){Self::id(&page.page_token)?;}
        Ok(size as usize)
    }
}
impl enrichment_wire::EnrichmentConfigurationService for EnrichmentV2Service {
    #[worker::send]
    async fn publish_provider_profile(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,enrichment_wire::PublishProviderProfileRequest>)
        ->ServiceResult<enrichment_wire::PublishProviderProfileResponse>{
        let input=req.to_owned_message();
        let (key,_)=wire_precondition(input.precondition.as_option(),true)?;
        let policy=match input.raw_payload_policy {
            x if x==enrichment_wire::RawPayloadPolicy::RAW_PAYLOAD_POLICY_DIGEST_ONLY=>RawPayloadPolicy::DigestOnly,
            x if x==enrichment_wire::RawPayloadPolicy::RAW_PAYLOAD_POLICY_GOVERNED_PROTECTED_EVIDENCE=>RawPayloadPolicy::GovernedProtectedEvidence,
            _=>return Err(ConnectError::invalid_argument("RAW_PAYLOAD_POLICY_INVALID")),
        };
        let effective=input.effective_at.as_option()
            .ok_or_else(||ConnectError::invalid_argument("effective_at required"))?;
        let draft=owner::ProfileInput{
            provider_key:input.provider_key,adapter_kind:input.adapter_kind,
            adapter_contract_version:input.adapter_contract_version,purpose_codes:input.purpose_codes,
            license_id:input.license_id,permitted_use_class:input.permitted_use_class,
            residency_region:input.residency_region,retention_days:input.retention_days,
            raw_payload_policy:policy,credential_handle_aliases:input.credential_handle_aliases,
            effective_at_unix_ms:Self::millis(effective)?,
            expires_at_unix_ms:input.expires_at.as_option().map(Self::millis).transpose()?,
        };
        let saved=owner::publish_profile(draft,&key,&self.d1()?,&self.tenant,&self.actor,&self.request_id).await
            .map_err(|_|ConnectError::internal("profile D1 publish failed"))?.map_err(Self::failed)?;
        RpcResponse::ok(enrichment_wire::PublishProviderProfileResponse{
            profile:Some(Self::profile(&saved.value)?).into(),
            receipt:Some(self.receipt(saved.replayed)).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn publish_mapping(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,enrichment_wire::PublishMappingRequest>)
        ->ServiceResult<enrichment_wire::PublishMappingResponse>{
        let input=req.to_owned_message();
        let (key,_)=wire_precondition(input.precondition.as_option(),true)?;
        Self::id(&input.provider_profile_version_id)?;
        let draft=owner::MappingInput{
            mapping_key:input.mapping_key,provider_profile_version_id:input.provider_profile_version_id,
            provider_response_field_path:input.provider_response_field_path,
            maximum_suggestions_per_response:input.maximum_suggestions_per_response,
            confidence_required:input.confidence_required,
        };
        let saved=owner::publish_mapping(draft,&key,&self.d1()?,&self.tenant,&self.actor,&self.request_id).await
            .map_err(|_|ConnectError::internal("mapping D1 publish failed"))?.map_err(Self::failed)?;
        RpcResponse::ok(enrichment_wire::PublishMappingResponse{
            mapping:Some(Self::mapping(&saved.value)).into(),
            receipt:Some(self.receipt(saved.replayed)).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn get_provider_profile(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,enrichment_wire::GetProviderProfileRequest>)
        ->ServiceResult<enrichment_wire::GetProviderProfileResponse>{
        Self::id(&req.profile_version_id)?;
        let value=owner::profile(&self.d1()?,&self.tenant,&req.profile_version_id).await
            .map_err(|_|ConnectError::internal("profile D1 read failed"))?
            .ok_or_else(||ConnectError::not_found("ENRICHMENT_PROVIDER_PROFILE_NOT_FOUND"))?;
        RpcResponse::ok(enrichment_wire::GetProviderProfileResponse{
            profile:Some(Self::profile(&value)?).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn get_mapping(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,enrichment_wire::GetMappingRequest>)
        ->ServiceResult<enrichment_wire::GetMappingResponse>{
        Self::id(&req.mapping_version_id)?;
        let value=owner::mapping(&self.d1()?,&self.tenant,&req.mapping_version_id).await
            .map_err(|_|ConnectError::internal("mapping D1 read failed"))?
            .ok_or_else(||ConnectError::not_found("ENRICHMENT_MAPPING_NOT_FOUND"))?;
        RpcResponse::ok(enrichment_wire::GetMappingResponse{
            mapping:Some(Self::mapping(&value)).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn list_provider_profiles(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,enrichment_wire::ListProviderProfilesRequest>)
        ->ServiceResult<enrichment_wire::ListProviderProfilesResponse>{
        let size=Self::page(&req.page)?;
        let (items,next)=owner::profiles(&self.d1()?,&self.tenant,&req.page.page_token,size).await
            .map_err(|_|ConnectError::internal("profile D1 list failed"))?;
        RpcResponse::ok(enrichment_wire::ListProviderProfilesResponse{
            profiles:items.iter().map(Self::profile).collect::<std::result::Result<Vec<_>,_>>()?,
            page_info:Some(core_v2::PageInfo{next_page_token:next.unwrap_or_default(),..Default::default()}).into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn list_mappings(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,enrichment_wire::ListMappingsRequest>)
        ->ServiceResult<enrichment_wire::ListMappingsResponse>{
        let size=Self::page(&req.page)?;
        let (items,next)=owner::mappings(&self.d1()?,&self.tenant,&req.page.page_token,size).await
            .map_err(|_|ConnectError::internal("mapping D1 list failed"))?;
        RpcResponse::ok(enrichment_wire::ListMappingsResponse{
            mappings:items.iter().map(Self::mapping).collect(),
            page_info:Some(core_v2::PageInfo{next_page_token:next.unwrap_or_default(),..Default::default()}).into(),
            ..Default::default()
        })
    }
}
