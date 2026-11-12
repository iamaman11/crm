//! Generated Connect trait for the existing ContactPointV2Service Rust domain owner.
//! Access, tenant, and the single native router are defined by the parent.
use super::*;

impl contact_wire::ContactPointService for ContactPointV2Service {
    #[worker::send]
    async fn create_contact_point(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, contact_wire::CreateContactPointRequest>,
    ) -> ServiceResult<contact_wire::CreateContactPointResponse> {
        let input = req.to_owned_message();
        let id = Self::valid_id(&input.contact_point_id)?;
        let kind = match input.kind {
            x if x == contact_wire::ContactPointKind::CONTACT_POINT_KIND_EMAIL => {
                contact_points::KindInput::Email
            }
            x if x == contact_wire::ContactPointKind::CONTACT_POINT_KIND_PHONE => {
                contact_points::KindInput::Phone
            }
            x if x == contact_wire::ContactPointKind::CONTACT_POINT_KIND_POSTAL => {
                contact_points::KindInput::Postal
            }
            x if x == contact_wire::ContactPointKind::CONTACT_POINT_KIND_WEB => {
                contact_points::KindInput::Web
            }
            x if x == contact_wire::ContactPointKind::CONTACT_POINT_KIND_MESSAGING => {
                contact_points::KindInput::Messaging
            }
            _ => return Err(ConnectError::invalid_argument("invalid ContactPointKind")),
        };
        let (key, _) = wire_precondition(input.precondition.as_option(), true)?;
        let from = input.valid_from.as_option().map(Self::nanos).transpose()?;
        let until = input.valid_until.as_option().map(Self::nanos).transpose()?;
        let saved = contact_points::create_typed(
            contact_points::CreateInput {
                id,
                party_id: input.party_id,
                kind,
                value: input.value,
                preferred: input.preferred,
                valid_from_unix_nanos: from,
                valid_until_unix_nanos: until,
            },
            &key,
            &self.d1()?,
            &self.tenant,
            &self.request_id,
        )
        .await
        .map_err(|_| ConnectError::internal("ContactPoint D1 create failed"))?
        .map_err(Self::failed)?;
        RpcResponse::ok(contact_wire::CreateContactPointResponse {
            contact_point: Some(Self::convert(&saved.contact_point)?).into(),
            receipt: Some(self.receipt(&saved)?).into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn update_contact_point(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, contact_wire::UpdateContactPointRequest>,
    ) -> ServiceResult<contact_wire::UpdateContactPointResponse> {
        let input = req.to_owned_message();
        let id = Self::valid_id(&input.contact_point_id)?;
        let (key, version) = wire_precondition(input.precondition.as_option(), false)?;
        let status = match input.status {
            x if x == contact_wire::ContactPointStatus::CONTACT_POINT_STATUS_ACTIVE => {
                contact_points::StatusInput::Active
            }
            x if x == contact_wire::ContactPointStatus::CONTACT_POINT_STATUS_INACTIVE => {
                contact_points::StatusInput::Inactive
            }
            _ => return Err(ConnectError::invalid_argument("invalid ContactPointStatus")),
        };
        let db = self.d1()?;
        // Hash the unchanged wire patch intent before reading current D1 state;
        // the existing owner checks its receipt first, even after later updates.
        let from = Self::time_patch(input.valid_from.as_option())?;
        let until = Self::time_patch(input.valid_until.as_option())?;
        let saved = contact_points::update_typed(
            contact_points::UpdateInput {
                expected_version: version.unwrap_or_default(),
                value: input.value,
                status,
                preferred: input.preferred,
                valid_from_unix_nanos: from,
                valid_until_unix_nanos: until,
            },
            &key,
            &db,
            &self.tenant,
            &id,
            &self.request_id,
        )
        .await
        .map_err(|_| ConnectError::internal("ContactPoint D1 update failed"))?
        .map_err(Self::failed)?;
        RpcResponse::ok(contact_wire::UpdateContactPointResponse {
            contact_point: Some(Self::convert(&saved.contact_point)?).into(),
            receipt: Some(self.receipt(&saved)?).into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn get_contact_point(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, contact_wire::GetContactPointRequest>,
    ) -> ServiceResult<contact_wire::GetContactPointResponse> {
        let id = Self::valid_id(req.contact_point_id)?;
        let point = contact_points::load(&self.d1()?, &self.tenant, &id)
            .await
            .map_err(|_| ConnectError::internal("ContactPoint D1 read failed"))?
            .ok_or_else(|| ConnectError::not_found("CONTACT_POINTS_NOT_FOUND"))?;
        RpcResponse::ok(contact_wire::GetContactPointResponse {
            contact_point: Some(Self::convert(&point)?).into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn list_contact_points(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, contact_wire::ListContactPointsRequest>,
    ) -> ServiceResult<contact_wire::ListContactPointsResponse> {
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
        let filter = req.party_id;
        if let Some(party) = filter {
            crm_contact_points::PartyReference::try_new(party)
                .map_err(|_| ConnectError::invalid_argument("invalid party_id filter"))?;
        }
        let (list, next) = contact_points::list_page_typed(
            &self.d1()?,
            &self.tenant,
            after,
            size as usize,
            filter,
        )
        .await
        .map_err(|_| ConnectError::internal("ContactPoint D1 list failed"))?;
        let mut results = Vec::new();
        for point in &list {
            results.push(Self::convert(point)?);
        }
        RpcResponse::ok(contact_wire::ListContactPointsResponse {
            contact_points: results,
            page_info: Some(core_v2::PageInfo {
                next_page_token: next.unwrap_or_default(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }
}
