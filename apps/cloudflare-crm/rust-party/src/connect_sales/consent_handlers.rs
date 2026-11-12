//! Generated Connect trait for the existing ConsentV2Service Rust domain owner.
//! Access, tenant, and the single native router are defined by the parent.
use super::*;

impl consent_wire::ConsentAuthorizationService for ConsentV2Service {
    #[worker::send]
    async fn evaluate_consent(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, consent_wire::EvaluateConsentRequest>,
    ) -> ServiceResult<consent_wire::EvaluateConsentResponse> {
        let input = req.to_owned_message();
        let decision = consents::evaluate_typed(
            consents::ScopeInput {
                party_id: input.party_id,
                contact_point_id: input.contact_point_id,
                purpose: input.purpose,
                channel: Self::channel(input.channel)?,
            },
            &self.d1()?,
            &self.tenant,
        )
        .await
        .map_err(|_| ConnectError::internal("Original Consent evaluation failed"))?
        .map_err(Self::failed)?;
        let reason = match decision.reason {
            DomainConsentReason::ActiveGrant => {
                consent_wire::EvaluationReason::EVALUATION_REASON_ACTIVE_GRANT
            }
            DomainConsentReason::ActiveDeny => {
                consent_wire::EvaluationReason::EVALUATION_REASON_ACTIVE_DENY
            }
            DomainConsentReason::Withdrawn => {
                consent_wire::EvaluationReason::EVALUATION_REASON_WITHDRAWN
            }
            DomainConsentReason::NoApplicableGrant => {
                consent_wire::EvaluationReason::EVALUATION_REASON_NO_APPLICABLE_GRANT
            }
        };
        RpcResponse::ok(consent_wire::EvaluateConsentResponse {
            allowed: decision.allowed,
            reason: reason.into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn suppress_communication(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, consent_wire::SuppressCommunicationRequest>,
    ) -> ServiceResult<consent_wire::SuppressCommunicationResponse> {
        let input = req.to_owned_message();
        let (key, _) = wire_precondition(input.precondition.as_option(), true)?;
        let saved = consents::suppress_typed(
            consents::SuppressionInput {
                id: input.authorization_id,
                party_id: input.party_id,
                contact_point_id: input.contact_point_id,
                purpose: input.purpose,
                channel: Self::channel(input.channel)?,
                jurisdiction: input.jurisdiction,
            },
            &key,
            &self.d1()?,
            &self.tenant,
            &self.actor,
            &self.request_id,
        )
        .await
        .map_err(|_| ConnectError::internal("Original Consent suppression failed"))?
        .map_err(Self::failed)?;
        RpcResponse::ok(consent_wire::SuppressCommunicationResponse {
            authorization: Some(Self::convert(&saved.item)).into(),
            receipt: Some(core_v2::MutationReceipt {
                request_id: self.request_id.clone(),
                resource_version: saved.item.version(),
                replayed: saved.replayed,
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn list_consent_authorizations(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, consent_wire::ListConsentAuthorizationsRequest>,
    ) -> ServiceResult<consent_wire::ListConsentAuthorizationsResponse> {
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
            crm_consents::ConsentAuthorizationId::try_new(*after)
                .map_err(|_| ConnectError::invalid_argument("invalid page_token"))?;
        }
        let filter = req.party_id;
        if let Some(party) = filter {
            crm_consents::PartyReference::try_new(party)
                .map_err(|_| ConnectError::invalid_argument("invalid party_id filter"))?;
        }
        let (list, next) =
            consents::list_page_typed(&self.d1()?, &self.tenant, after, size as usize, filter)
                .await
                .map_err(|_| ConnectError::internal("Original Consent D1 list failed"))?;
        let authorizations = list.iter().map(Self::convert).collect();
        RpcResponse::ok(consent_wire::ListConsentAuthorizationsResponse {
            authorizations,
            page_info: Some(core_v2::PageInfo {
                next_page_token: next.unwrap_or_default(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }
}
