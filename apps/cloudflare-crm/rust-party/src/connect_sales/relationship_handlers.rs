//! Generated Connect trait for the existing RelationshipV2Service Rust domain owner.
//! Access, tenant, and the single native router are defined by the parent.
use super::*;

impl rel_wire::PartyRelationshipService for RelationshipV2Service {
    #[worker::send]
    async fn create_party_relationship(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, rel_wire::CreatePartyRelationshipRequest>,
    ) -> ServiceResult<rel_wire::CreatePartyRelationshipResponse> {
        let input = req.to_owned_message();
        let id = Self::valid_id(&input.relationship_id)?;
        let (key, _) = wire_precondition(input.precondition.as_option(), true)?;
        let typ = input
            .r#type
            .as_option()
            .ok_or_else(|| ConnectError::invalid_argument("relationship type required"))?;
        let directionality=match typ.directionality {
            x if x==rel_wire::RelationshipDirectionality::RELATIONSHIP_DIRECTIONALITY_DIRECTIONAL=>
                party_relationships::DirectionalityInput::Directional,
            x if x==rel_wire::RelationshipDirectionality::RELATIONSHIP_DIRECTIONALITY_RECIPROCAL=>
                party_relationships::DirectionalityInput::Reciprocal,
            _=>return Err(ConnectError::invalid_argument("invalid RelationshipDirectionality")),
        };
        let from = input
            .valid_from
            .as_option()
            .map(ContactPointV2Service::nanos)
            .transpose()?;
        let until = input
            .valid_until
            .as_option()
            .map(ContactPointV2Service::nanos)
            .transpose()?;
        let saved = party_relationships::create_typed(
            party_relationships::CreateInput {
                id,
                from_party_id: input.from_party_id,
                to_party_id: input.to_party_id,
                code: typ.code.clone(),
                directionality,
                from_role: typ.from_role.clone(),
                to_role: typ.to_role.clone(),
                valid_from_unix_nanos: from,
                valid_until_unix_nanos: until,
            },
            &key,
            &self.d1()?,
            &self.tenant,
            &self.request_id,
        )
        .await
        .map_err(|_| ConnectError::internal("Relationship D1 create failed"))?
        .map_err(Self::failed)?;
        RpcResponse::ok(rel_wire::CreatePartyRelationshipResponse {
            relationship: Some(Self::convert(&saved.relationship)?).into(),
            receipt: Some(self.receipt(&saved)).into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn update_party_relationship(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, rel_wire::UpdatePartyRelationshipRequest>,
    ) -> ServiceResult<rel_wire::UpdatePartyRelationshipResponse> {
        let input = req.to_owned_message();
        let id = Self::valid_id(&input.relationship_id)?;
        let (key, version) = wire_precondition(input.precondition.as_option(), false)?;
        let status = match input.status {
            x if x == rel_wire::RelationshipStatus::RELATIONSHIP_STATUS_ACTIVE => {
                party_relationships::StatusInput::Active
            }
            x if x == rel_wire::RelationshipStatus::RELATIONSHIP_STATUS_INACTIVE => {
                party_relationships::StatusInput::Inactive
            }
            _ => return Err(ConnectError::invalid_argument("invalid RelationshipStatus")),
        };
        let from = Self::patch(input.valid_from.as_option())?;
        let until = Self::patch(input.valid_until.as_option())?;
        let saved = party_relationships::update_typed(
            party_relationships::UpdateCommand {
                expected_version: version.unwrap_or_default(),
                status,
                valid_from: from,
                valid_until: until,
            },
            &key,
            &self.d1()?,
            &self.tenant,
            &id,
            &self.request_id,
        )
        .await
        .map_err(|_| ConnectError::internal("Relationship D1 update failed"))?
        .map_err(Self::failed)?;
        RpcResponse::ok(rel_wire::UpdatePartyRelationshipResponse {
            relationship: Some(Self::convert(&saved.relationship)?).into(),
            receipt: Some(self.receipt(&saved)).into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn get_party_relationship(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, rel_wire::GetPartyRelationshipRequest>,
    ) -> ServiceResult<rel_wire::GetPartyRelationshipResponse> {
        let id = Self::valid_id(req.relationship_id)?;
        let relationship = party_relationships::load(&self.d1()?, &self.tenant, &id)
            .await
            .map_err(|_| ConnectError::internal("Relationship D1 read failed"))?
            .ok_or_else(|| ConnectError::not_found("PARTY_RELATIONSHIPS_NOT_FOUND"))?;
        RpcResponse::ok(rel_wire::GetPartyRelationshipResponse {
            relationship: Some(Self::convert(&relationship)?).into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn list_party_relationships(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, rel_wire::ListPartyRelationshipsRequest>,
    ) -> ServiceResult<rel_wire::ListPartyRelationshipsResponse> {
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
            Self::valid_id(after)?;
        }
        let party = req.party_id;
        if let Some(id) = party {
            crm_party_relationships::PartyReference::try_new(id.to_owned())
                .map_err(|_| ConnectError::invalid_argument("invalid Party filter"))?;
        }
        let (list, next) = party_relationships::list_page_typed(
            &self.d1()?,
            &self.tenant,
            after,
            size as usize,
            party,
        )
        .await
        .map_err(|_| ConnectError::internal("Relationship D1 list failed"))?;
        let mut results = Vec::with_capacity(list.len());
        for relationship in &list {
            results.push(Self::convert(relationship)?);
        }
        RpcResponse::ok(rel_wire::ListPartyRelationshipsResponse {
            relationships: results,
            page_info: Some(core_v2::PageInfo {
                next_page_token: next.unwrap_or_default(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }
}
