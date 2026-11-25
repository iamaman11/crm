//! Generated Connect trait for the existing IdentityV2Service Rust domain owner.
//! Access, tenant, and the single native router are defined by the parent.
use super::*;

impl IdentityV2Service {
    fn d1(&self) -> std::result::Result<D1Database, ConnectError> {
        self.env
            .d1("DB")
            .map_err(|_| ConnectError::internal("D1 unavailable"))
    }
    fn merge_wire(
        operation: &crm_identity_resolution::MergeOperation,
    ) -> identity_wire::MergeOperation {
        identity_wire::MergeOperation {
            operation_id: operation.operation_id().as_str().to_owned(),
            source_party_id: operation.source_party_ref().as_str().to_owned(),
            survivor_party_id: operation.survivor_party_ref().as_str().to_owned(),
            source_party_version: operation.source_party_version(),
            survivor_party_version: operation.survivor_party_version(),
            reason: operation.reason().as_str().to_owned(),
            active: operation.status() == crm_identity_resolution::MergeOperationStatus::Active,
            version: operation.version(),
            unmerge_reason: operation
                .unmerge_decision()
                .map(|d| d.reason().as_str().to_owned()),
            decided_by: operation.decided_by().as_str().to_owned(),
            ..Default::default()
        }
    }
    fn merge_receipt(&self, commit: &identity_resolution::MergeCommit) -> core_v2::MutationReceipt {
        core_v2::MutationReceipt {
            request_id: self.request_id.clone(),
            resource_version: commit.operation.version(),
            replayed: commit.replayed,
            original_request_id: Some(commit.original_request_id.clone()),
            ..Default::default()
        }
    }
}

impl identity_wire::IdentityResolutionService for IdentityV2Service {
    #[worker::send]
    async fn find_shared_email_candidates(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, identity_wire::FindSharedEmailCandidatesRequest>,
    ) -> ServiceResult<identity_wire::FindSharedEmailCandidatesResponse> {
        let size = if req.page.page_size == 0 {
            25
        } else {
            req.page.page_size
        };
        if size > 100 {
            return Err(ConnectError::invalid_argument("page_size exceeds 100"));
        }
        // Numeric offset is an opaque cursor over the bounded, deterministic
        // canonical pair order. Never LIMIT raw contacts before domain filtering.
        let after = &req.page.page_token;
        let offset = if after.is_empty() {
            0
        } else {
            let parsed = after
                .parse::<usize>()
                .map_err(|_| ConnectError::invalid_argument("invalid page_token"))?;
            if parsed == 0 || parsed.to_string() != *after {
                return Err(ConnectError::invalid_argument("invalid page_token"));
            }
            parsed
        };
        let db = self
            .env
            .d1("DB")
            .map_err(|_| ConnectError::internal("D1 unavailable"))?;
        let candidates = match identity_resolution::candidates_typed(&db, &self.actor.tenant)
            .await
            .map_err(|error| crate::diagnostics::read_error(&self.request_id, &error))?
        {
            Ok(items) => items,
            Err(identity_resolution::CandidateError::ScanLimit) => {
                return Err(ConnectError::unavailable(
                    "IDENTITY_RESOLUTION_SCAN_LIMIT_REACHED",
                ));
            }
            Err(identity_resolution::CandidateError::SourceChanged) => {
                return Err(ConnectError::aborted("IDENTITY_RESOLUTION_SOURCE_CHANGED"));
            }
        };
        let mut items = Vec::new();
        for candidate in candidates.iter().skip(offset).take(size as usize) {
            items.push(identity_wire::SharedEmailCandidate {
                first_party: Some(PartyV3Service::convert(&candidate.first)?).into(),
                second_party: Some(PartyV3Service::convert(&candidate.second)?).into(),
                first_contact_point_id: candidate.first_contact_point_id.clone(),
                second_contact_point_id: candidate.second_contact_point_id.clone(),
                both_contacts_verified: candidate.both_verified,
                ..Default::default()
            });
        }
        let next = if offset.saturating_add(size as usize) < candidates.len() {
            (offset + size as usize).to_string()
        } else {
            String::new()
        };
        RpcResponse::ok(identity_wire::FindSharedEmailCandidatesResponse {
            candidates: items,
            page_info: Some(core_v2::PageInfo {
                next_page_token: next,
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn compare_parties(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, identity_wire::ComparePartiesRequest>,
    ) -> ServiceResult<identity_wire::ComparePartiesResponse> {
        let first = PartyV3Service::valid_id(req.first_party_id)?;
        let second = PartyV3Service::valid_id(req.second_party_id)?;
        let pair = CanonicalPartyPair::try_new(
            PartyReference::try_new(first.as_str())
                .map_err(|_| ConnectError::invalid_argument("invalid first_party_id"))?,
            PartyReference::try_new(second.as_str())
                .map_err(|_| ConnectError::invalid_argument("invalid second_party_id"))?,
        )
        .map_err(|_| ConnectError::invalid_argument("IDENTITY_RESOLUTION_SELF_PAIR_INVALID"))?;
        let db = self
            .env
            .d1("DB")
            .map_err(|_| ConnectError::internal("D1 unavailable"))?;
        let comparison =
            identity_resolution::compare_typed(&db, &self.actor.tenant, &pair, first.as_str())
                .await
                .map_err(|error| crate::diagnostics::read_error(&self.request_id, &error))?
                .ok_or_else(|| ConnectError::not_found("PARTIES_NOT_FOUND"))?;
        RpcResponse::ok(identity_wire::ComparePartiesResponse {
            first_party: Some(PartyV3Service::convert(&comparison.first)?).into(),
            second_party: Some(PartyV3Service::convert(&comparison.second)?).into(),
            same_party_id: false,
            identity_confirmed: false,
            ..Default::default()
        })
    }

    #[worker::send]
    async fn merge_party(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, identity_wire::MergePartyRequest>,
    ) -> ServiceResult<identity_wire::MergePartyResponse> {
        let input = req.to_owned_message();
        let id = PartyV3Service::valid_id(&input.operation_id)?;
        let source = PartyV3Service::valid_id(&input.source_party_id)?;
        let survivor = PartyV3Service::valid_id(&input.survivor_party_id)?;
        let pre = input
            .precondition
            .as_option()
            .ok_or_else(|| ConnectError::invalid_argument("mutation precondition required"))?;
        if pre.expected_version.is_some() {
            return Err(ConnectError::invalid_argument(
                "merge create must not include expected_version",
            ));
        }
        let db = self.d1()?;
        let committed = identity_resolution::create_merge_typed(
            &db,
            &self.actor,
            identity_resolution::CreateMergeInput {
                operation_id: id.as_str().to_owned(),
                source_party_id: source.as_str().to_owned(),
                source_version: input.source_party_version,
                survivor_party_id: survivor.as_str().to_owned(),
                survivor_version: input.survivor_party_version,
                reason: input.reason,
            },
            &pre.idempotency_key,
            &self.request_id,
        )
        .await
        .map_err(|_| ConnectError::unavailable("IDENTITY_MERGE_WRITE_UNAVAILABLE"))?
        .map_err(|failure| failure.connect("MergeParty", &self.request_id))?;
        RpcResponse::ok(identity_wire::MergePartyResponse {
            operation: Some(Self::merge_wire(&committed.operation)).into(),
            receipt: Some(self.merge_receipt(&committed)).into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn unmerge_party(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, identity_wire::UnmergePartyRequest>,
    ) -> ServiceResult<identity_wire::UnmergePartyResponse> {
        let input = req.to_owned_message();
        let id = PartyV3Service::valid_id(&input.operation_id)?;
        let pre = input
            .precondition
            .as_option()
            .ok_or_else(|| ConnectError::invalid_argument("mutation precondition required"))?;
        let expected = pre
            .expected_version
            .ok_or_else(|| ConnectError::invalid_argument("unmerge expected_version required"))?;
        if expected < 1 {
            return Err(ConnectError::invalid_argument(
                "unmerge expected_version invalid",
            ));
        }
        let db = self.d1()?;
        let committed = identity_resolution::unmerge_typed(
            &db,
            &self.actor,
            identity_resolution::UnmergeInput {
                operation_id: id.as_str(),
                expected_version: expected,
                source_version: input.source_party_version,
                survivor_version: input.survivor_party_version,
                reason: input.reason,
            },
            &pre.idempotency_key,
            &self.request_id,
        )
        .await
        .map_err(|_| ConnectError::unavailable("IDENTITY_UNMERGE_WRITE_UNAVAILABLE"))?
        .map_err(|failure| failure.connect("UnmergeParty", &self.request_id))?;
        RpcResponse::ok(identity_wire::UnmergePartyResponse {
            operation: Some(Self::merge_wire(&committed.operation)).into(),
            receipt: Some(self.merge_receipt(&committed)).into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn get_merge_operation(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, identity_wire::GetMergeOperationRequest>,
    ) -> ServiceResult<identity_wire::GetMergeOperationResponse> {
        let id = PartyV3Service::valid_id(req.operation_id)?;
        let db = self.d1()?;
        let operation = identity_resolution::stored_merge(&db, &self.actor.tenant, id.as_str())
            .await
            .map_err(|_| ConnectError::unavailable("IDENTITY_MERGE_READ_UNAVAILABLE"))?
            .ok_or_else(|| ConnectError::not_found("IDENTITY_MERGE_NOT_FOUND"))?;
        let confirmed_mutation = match receipt_key(req.receipt_idempotency_key)? {
            None => None,
            Some(key) => {
                let prior = identity_resolution::merge_receipt(&db, &self.actor.tenant, key)
                    .await
                    .map_err(|_| ConnectError::unavailable("IDENTITY_MERGE_RECEIPT_UNAVAILABLE"))?;
                prior.map(|proof|->std::result::Result<core_v2::MutationCommitEvidence,ConnectError>{
                    let stored=crm_identity_resolution::decode_merge_operation_state(proof.state.as_bytes())
                        .map_err(|_|ConnectError::unavailable("IDENTITY_MERGE_RECEIPT_INVALID"))?;
                    if stored.operation_id().as_str()!=id.as_str() || proof.original_request_id.is_empty(){
                        return Err(ConnectError::aborted("IDENTITY_MERGE_RECEIPT_NOT_MATCHED"));
                    }
                    Ok(core_v2::MutationCommitEvidence{
                        request_id:self.request_id.clone(),
                        resource_version:stored.version(),
                        original_request_id:proof.original_request_id,
                        ..Default::default()
                    })
                }).transpose()?
            }
        };
        RpcResponse::ok(identity_wire::GetMergeOperationResponse {
            operation: Some(Self::merge_wire(&operation)).into(),
            confirmed_mutation: confirmed_mutation.into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn list_merge_operations_by_party(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, identity_wire::ListMergeOperationsByPartyRequest>,
    ) -> ServiceResult<identity_wire::ListMergeOperationsByPartyResponse> {
        let party = PartyV3Service::valid_id(req.party_id)?;
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
            PartyV3Service::valid_id(after)?;
        }
        let db = self.d1()?;
        if parties::stored(&db, &self.actor.tenant, party.as_str())
            .await
            .map_err(|_| ConnectError::unavailable("IDENTITY_PARTY_READ_UNAVAILABLE"))?
            .is_none()
        {
            return Err(ConnectError::not_found("IDENTITY_PARTY_NOT_FOUND"));
        }
        let (operations, next) = identity_resolution::list_merges(
            &db,
            &self.actor.tenant,
            party.as_str(),
            after,
            size as usize,
        )
        .await
        .map_err(|_| ConnectError::unavailable("IDENTITY_MERGE_LIST_UNAVAILABLE"))?;
        RpcResponse::ok(identity_wire::ListMergeOperationsByPartyResponse {
            operations: operations.iter().map(Self::merge_wire).collect(),
            page_info: Some(core_v2::PageInfo {
                next_page_token: next.unwrap_or_default(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn resolve_canonical_party(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, identity_wire::ResolveCanonicalPartyRequest>,
    ) -> ServiceResult<identity_wire::ResolveCanonicalPartyResponse> {
        let party = PartyV3Service::valid_id(req.party_id)?;
        let db = self.d1()?;
        let resolution =
            identity_resolution::resolve_canonical(&db, &self.actor.tenant, party.as_str())
                .await
                .map_err(|_| ConnectError::unavailable("IDENTITY_MERGE_TOPOLOGY_UNAVAILABLE"))?
                .ok_or_else(|| ConnectError::not_found("IDENTITY_PARTY_NOT_FOUND"))?;
        RpcResponse::ok(identity_wire::ResolveCanonicalPartyResponse {
            requested_party_id: resolution.requested_party_ref().as_str().to_owned(),
            canonical_party_id: resolution.canonical_party_ref().as_str().to_owned(),
            party_path: resolution
                .party_path()
                .iter()
                .map(|p| p.as_str().to_owned())
                .collect(),
            merge_operation_path: resolution
                .merge_operation_path()
                .iter()
                .map(|o| o.as_str().to_owned())
                .collect(),
            ..Default::default()
        })
    }
}
