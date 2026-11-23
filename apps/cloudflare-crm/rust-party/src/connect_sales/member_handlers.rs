//! Typed Access Membership RPCs over the ORIGINAL native Access authority.
//! This is only a signed staff roster, not a JS identity backend.
use super::*;

impl member_wire::MemberService for MemberV2Service {
    #[worker::send]
    async fn get_team(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, member_wire::GetTeamRequest>,
    ) -> ServiceResult<member_wire::GetTeamResponse> {
        let input = req.to_owned_message();
        let id =
            access_teams::id(&input.team_id).map_err(|e| e.connect("GetTeam", &self.request_id))?;
        let db = self.d1()?;
        let team = access_teams::get(&db, &self.actor.tenant, &id)
            .await
            .map_err(|_| ConnectError::internal("Team read failed"))?
            .ok_or_else(|| ConnectError::not_found("ACCESS_TEAM_NOT_FOUND"))?;
        let evidence = match receipt_key(input.receipt_idempotency_key.as_deref())? {
            None => None,
            Some(key) => {
                let row = crud_receipt(&db, &self.actor.tenant, key, CrudReceiptOwner::Team)
                    .await
                    .map_err(|_| ConnectError::unavailable("ACCESS_TEAM_RECEIPT_READ_FAILED"))?;
                crud_commit_evidence(row, &id, &self.request_id, |state| {
                    access_teams::decode(state).map(|team| (team.team_id, team.version))
                })
                .map_err(|_| ConnectError::unavailable("ACCESS_TEAM_RECEIPT_DECODE_FAILED"))?
            }
        };
        RpcResponse::ok(member_wire::GetTeamResponse {
            team: Some(team_wire(team)).into(),
            confirmed_mutation: evidence.into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn list_teams(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, member_wire::ListTeamsRequest>,
    ) -> ServiceResult<member_wire::ListTeamsResponse> {
        let size = if req.page.page_size == 0 {
            25
        } else {
            req.page.page_size
        };
        if size > 100 {
            return Err(ConnectError::invalid_argument("page_size exceeds 100"));
        }
        let after = req.page.page_token;
        if !after.is_empty() {
            access_teams::id(after).map_err(|e| e.connect("ListTeams", &self.request_id))?;
        }
        let (teams, next) = access_teams::list(
            &self.d1()?,
            &self.actor.tenant,
            after,
            size as usize,
            req.include_inactive,
        )
        .await
        .map_err(|_| ConnectError::internal("Team list failed"))?;
        RpcResponse::ok(member_wire::ListTeamsResponse {
            teams: teams.into_iter().map(team_wire).collect(),
            page_info: Some(core_v2::PageInfo {
                next_page_token: next.unwrap_or_default(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn create_team(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, member_wire::CreateTeamRequest>,
    ) -> ServiceResult<member_wire::CreateTeamResponse> {
        let input = req.to_owned_message();
        let id = access_teams::id(&input.team_id)
            .map_err(|e| e.connect("CreateTeam", &self.request_id))?;
        let (key, _) = wire_precondition(input.precondition.as_option(), true)?;
        let saved = access_teams::mutate(
            &self.d1()?,
            &self.actor,
            &id,
            access_teams::Intent {
                name: Some(input.name),
                description: Some(input.description),
                active: Some(true),
                roster: input.roster.as_option().map(|v| v.member_emails.clone()),
                expected: None,
            },
            &key,
            &self.request_id,
        )
        .await
        .map_err(|_| ConnectError::unavailable("ACCESS_TEAM_WRITE_FAILED"))?
        .map_err(|e| e.connect("CreateTeam", &self.request_id))?;
        let receipt = team_receipt(&saved, &self.request_id);
        RpcResponse::ok(member_wire::CreateTeamResponse {
            team: Some(team_wire(saved.team)).into(),
            receipt: Some(receipt).into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn update_team(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, member_wire::UpdateTeamRequest>,
    ) -> ServiceResult<member_wire::UpdateTeamResponse> {
        let input = req.to_owned_message();
        let id = access_teams::id(&input.team_id)
            .map_err(|e| e.connect("UpdateTeam", &self.request_id))?;
        let (key, expected) = wire_precondition(input.precondition.as_option(), false)?;
        if input.name.is_none()
            && input.description.is_none()
            && input.active.is_none()
            && input.roster.as_option().is_none()
        {
            return Err(ConnectError::invalid_argument("ACCESS_TEAM_NO_CHANGES"));
        }
        let saved = access_teams::mutate(
            &self.d1()?,
            &self.actor,
            &id,
            access_teams::Intent {
                name: input.name,
                description: input.description,
                active: input.active,
                roster: input.roster.as_option().map(|v| v.member_emails.clone()),
                expected,
            },
            &key,
            &self.request_id,
        )
        .await
        .map_err(|_| ConnectError::unavailable("ACCESS_TEAM_WRITE_FAILED"))?
        .map_err(|e| e.connect("UpdateTeam", &self.request_id))?;
        let receipt = team_receipt(&saved, &self.request_id);
        RpcResponse::ok(member_wire::UpdateTeamResponse {
            team: Some(team_wire(saved.team)).into(),
            receipt: Some(receipt).into(),
            ..Default::default()
        })
    }
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
        let db = self.d1()?;
        let ctx = access_members::MemberContext {
            db: &db,
            tenant: &self.actor.tenant,
            actor: &self.actor.identity,
        };
        let saved = access_members::provision(&ctx, &input.email, role, &key, &self.request_id)
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
        let db = self.d1()?;
        let ctx = access_members::MemberContext {
            db: &db,
            tenant: &self.actor.tenant,
            actor: &self.actor.identity,
        };
        let saved = access_members::update(
            &ctx,
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

fn team_wire(team: access_teams::Team) -> member_wire::Team {
    member_wire::Team {
        team_id: team.team_id,
        name: team.name,
        description: team.description,
        active: team.active,
        member_emails: team.member_emails,
        version: team.version,
        ..Default::default()
    }
}
fn team_receipt(saved: &access_teams::Saved, trace: &str) -> core_v2::MutationReceipt {
    core_v2::MutationReceipt {
        request_id: trace.to_owned(),
        resource_version: saved.team.version,
        replayed: saved.replayed,
        original_request_id: saved.original_request_id.clone(),
        ..Default::default()
    }
}
