//! Generated binary Connect handler for immutable Activities Interaction facts.
use super::*;
use crate::activity_interactions as interactions;
use crate::diagnostics;

impl interaction_wire::InteractionService for InteractionV2Service {
    #[worker::send]
    async fn create_interaction(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, interaction_wire::CreateInteractionRequest>,
    ) -> ServiceResult<interaction_wire::CreateInteractionResponse> {
        let input = req.to_owned_message();
        let (key, _) = wire_precondition(input.precondition.as_option(),true)?;
        let kind=match input.kind {
            value if value==interaction_wire::InteractionKind::INTERACTION_KIND_CALL =>
                crm_activities::interactions::InteractionKind::Call,
            value if value==interaction_wire::InteractionKind::INTERACTION_KIND_MEETING =>
                crm_activities::interactions::InteractionKind::Meeting,
            value if value==interaction_wire::InteractionKind::INTERACTION_KIND_NOTE =>
                crm_activities::interactions::InteractionKind::Note,
            _ => return Err(ConnectError::invalid_argument("ACTIVITIES_INTERACTION_KIND_INVALID")),
        };
        let occurred=input.occurred_at.as_option()
            .ok_or_else(||ConnectError::invalid_argument("ACTIVITIES_INTERACTION_TIME_REQUIRED"))?;
        let saved=interactions::create(
            crm_activities::interactions::CreateInteraction {
                interaction_id:input.interaction_id,
                account_id:input.account_id,
                deal_id: if input.deal_id.is_empty(){None}else{Some(input.deal_id)},
                kind,
                text:input.text,
                occurred_at_nanos:ContactPointV2Service::nanos(occurred)?,
            },
            &key,&self.d1()?,&self.actor,&self.request_id
        ).await.map_err(|e| diagnostics::mutation_error("CreateInteraction",&self.request_id,e))?;
        RpcResponse::ok(interaction_wire::CreateInteractionResponse{
            interaction:Some(Self::convert(&saved.interaction)).into(),
            receipt:Some(core_v2::MutationReceipt{
                request_id:self.request_id.clone(),
                resource_version:saved.interaction.version,
                replayed:saved.replayed,
                original_request_id:saved.original_request_id,
                ..Default::default()
            }).into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn get_interaction(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, interaction_wire::GetInteractionRequest>,
    ) -> ServiceResult<interaction_wire::GetInteractionResponse> {
        let input=req.to_owned_message();
        crm_activities::interactions::validate_identifier(&input.interaction_id)
            .map_err(|_|ConnectError::invalid_argument("ACTIVITIES_INTERACTION_ID_INVALID"))?;
        let db=self.d1()?;
        let item=interactions::get(&db,&self.actor.tenant,&input.interaction_id)
            .await.map_err(|_|ConnectError::internal("Interaction D1 read failed"))?
            .ok_or_else(||ConnectError::not_found("ACTIVITIES_INTERACTION_NOT_FOUND"))?;
        let confirmed_mutation=match receipt_key(input.receipt_idempotency_key.as_deref())? {
            None=>None,
            Some(key)=>{
                let row=crud_receipt(&db,&self.actor.tenant,key,CrudReceiptOwner::Interaction)
                    .await.map_err(|_|ConnectError::unavailable("INTERACTION_RECEIPT_READ_FAILED"))?;
                crud_commit_evidence(row,&input.interaction_id,&self.request_id,|state|{
                    let saved:crm_activities::interactions::Interaction=
                        serde_json::from_str(state)
                        .map_err(|_|worker::Error::RustError("Invalid Interaction receipt".into()))?;
                    saved.verify_stored()
                        .map_err(|e|worker::Error::RustError(e.code))?;
                    Ok((saved.interaction_id,saved.version))
                }).map_err(|_|ConnectError::unavailable("INTERACTION_RECEIPT_DECODE_FAILED"))?
            }
        };
        RpcResponse::ok(interaction_wire::GetInteractionResponse{
            interaction:Some(Self::convert(&item)).into(),
            confirmed_mutation:confirmed_mutation.into(),
            ..Default::default()
        })
    }

    #[worker::send]
    async fn list_interactions(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, interaction_wire::ListInteractionsRequest>,
    ) -> ServiceResult<interaction_wire::ListInteractionsResponse> {
        let input=req.to_owned_message();
        crm_activities::interactions::validate_identifier(&input.account_id)
            .map_err(|_|ConnectError::invalid_argument("ACTIVITIES_INTERACTION_ACCOUNT_ID_INVALID"))?;
        if !input.deal_id.is_empty(){
            crm_activities::interactions::validate_identifier(&input.deal_id)
                .map_err(|_|ConnectError::invalid_argument("ACTIVITIES_INTERACTION_DEAL_ID_INVALID"))?;
        }
        let size=if input.page.page_size==0{25}else{input.page.page_size};
        if size>100 {return Err(ConnectError::invalid_argument("page_size exceeds 100"))}
        if !input.page.page_token.is_empty(){
            let Some((timestamp,id))=input.page.page_token.split_once('|') else {
                return Err(ConnectError::invalid_argument("INVALID_INTERACTION_CURSOR"));
            };
            if timestamp.len()!=20 || !timestamp.bytes().all(|b|b.is_ascii_digit()) ||
                crm_activities::interactions::validate_identifier(id).is_err() {
                return Err(ConnectError::invalid_argument("INVALID_INTERACTION_CURSOR"));
            }
        }
        let (items,next)=interactions::list(&self.d1()?,&self.actor.tenant,
            &input.account_id,&input.deal_id,&input.page.page_token,size as usize)
            .await.map_err(|_|ConnectError::internal("Interaction D1 list failed"))?;
        RpcResponse::ok(interaction_wire::ListInteractionsResponse{
            interactions:items.iter().map(Self::convert).collect(),
            page_info:Some(core_v2::PageInfo{
                next_page_token:next.unwrap_or_default(),..Default::default()
            }).into(),
            ..Default::default()
        })
    }
}
