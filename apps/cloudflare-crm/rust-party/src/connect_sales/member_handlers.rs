//! Typed Access Membership RPCs over the ORIGINAL native Access authority.
//! This is only a signed staff roster, not a JS identity backend.
use super::*;

impl member_wire::MemberService for MemberV2Service {
    #[worker::send]
    async fn get_my_membership(
        &self,
        _ctx: RequestContext,
        _req: ServiceRequest<'_, member_wire::GetMyMembershipRequest>,
    ) -> ServiceResult<member_wire::GetMyMembershipResponse> {
        let member = access_members::get(&self.d1()?, &self.actor.tenant, &self.actor.identity)
            .await
            .map_err(|_| ConnectError::internal("Access member read failed"))?
            .ok_or_else(|| ConnectError::permission_denied("ACCESS_NOT_ENROLLED"))?;
        RpcResponse::ok(member_wire::GetMyMembershipResponse {
            member: Some(Self::wire(&member)?).into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn list_members(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, member_wire::ListMembersRequest>,
    ) -> ServiceResult<member_wire::ListMembersResponse> {
        let size = if req.page.page_size == 0 {
            25
        } else {
            req.page.page_size
        };
        if size > 100 {
            return Err(ConnectError::invalid_argument("page_size exceeds 100"));
        }
        let cursor = req.page.page_token;
        if !cursor.is_empty() {
            access_members::email(cursor)
                .map_err(|_| ConnectError::invalid_argument("INVALID_MEMBER_CURSOR"))?;
        }
        let (members, next) = access_members::list(
            &self.d1()?,
            &self.actor.tenant,
            cursor,
            size as usize,
            req.include_inactive,
        )
        .await
        .map_err(|_| ConnectError::internal("Access roster read failed"))?;
        let mut converted = Vec::with_capacity(members.len());
        for m in &members {
            converted.push(Self::wire(m)?);
        }
        RpcResponse::ok(member_wire::ListMembersResponse {
            members: converted,
            page_info: Some(core_v2::PageInfo {
                next_page_token: next.unwrap_or_default(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn get_member(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, member_wire::GetMemberRequest>,
    ) -> ServiceResult<member_wire::GetMemberResponse> {
        let input = req.to_owned_message();
        let email = access_members::email(&input.email)
            .map_err(|e| e.connect("GetMember", &self.request_id))?;
        let db = self.d1()?;
        let member = access_members::get(&db, &self.actor.tenant, &email)
            .await
            .map_err(|_| ConnectError::internal("Access member read failed"))?
            .ok_or_else(|| ConnectError::not_found("ACCESS_MEMBER_NOT_FOUND"))?;
        let evidence = match receipt_key(input.receipt_idempotency_key.as_deref())? {
            None => None,
            Some(key) => access_members::proof(&db, &self.actor.tenant, &email, key)
                .await
                .map_err(|_| ConnectError::unavailable("ACCESS_MEMBER_RECEIPT_READ_FAILED"))?
                .map(|(original, version)| core_v2::MutationCommitEvidence {
                    request_id: self.request_id.clone(),
                    original_request_id: original,
                    resource_version: version,
                    ..Default::default()
                }),
        };
        RpcResponse::ok(member_wire::GetMemberResponse {
            member: Some(Self::wire(&member)?).into(),
            confirmed_mutation: evidence.into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn provision_member(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, member_wire::ProvisionMemberRequest>,
    ) -> ServiceResult<member_wire::ProvisionMemberResponse> {
        let input = req.to_owned_message();
        let (key, _) = wire_precondition(input.precondition.as_option(), true)?;
        let role = Self::role(input.role)?;
        let saved = access_members::provision(
            &self.d1()?,
            &self.actor.tenant,
            &input.email,
            role,
            &key,
            &self.request_id,
            &self.actor.identity,
        )
        .await
        .map_err(|_| ConnectError::unavailable("ACCESS_MEMBER_WRITE_FAILED"))?
        .map_err(|e| e.connect("ProvisionMember", &self.request_id))?;
        RpcResponse::ok(member_wire::ProvisionMemberResponse {
            member: Some(Self::wire(&saved.member)?).into(),
            receipt: Some(self.receipt(&saved)).into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn update_member(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, member_wire::UpdateMemberRequest>,
    ) -> ServiceResult<member_wire::UpdateMemberResponse> {
        let input = req.to_owned_message();
        let (key, expected) = wire_precondition(input.precondition.as_option(), false)?;
        let new_role = input.role.map(Self::role).transpose()?;
        let saved = access_members::update(
            &self.d1()?,
            &self.actor.tenant,
            &self.actor.identity,
            &input.email,
            new_role,
            input.active,
            expected.unwrap_or_default(),
            &key,
            &self.request_id,
        )
        .await
        .map_err(|_| ConnectError::unavailable("ACCESS_MEMBER_WRITE_FAILED"))?
        .map_err(|e| e.connect("UpdateMember", &self.request_id))?;
        RpcResponse::ok(member_wire::UpdateMemberResponse {
            member: Some(Self::wire(&saved.member)?).into(),
            receipt: Some(self.receipt(&saved)).into(),
            ..Default::default()
        })
    }
}
