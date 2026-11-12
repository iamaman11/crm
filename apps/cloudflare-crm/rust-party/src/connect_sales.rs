//! One native Connect transport for original Rust domain services.
//! Cloudflare Access and tenant selection happen BEFORE routing here.
//! Domain rules and storage belong exclusively to existing crm-sales and D1.
use std::sync::Arc;
use axum::Router;
use connectrpc::{
    ConnectError, RequestContext, Response as RpcResponse, Router as ConnectRouter,
    ServiceRequest, ServiceResult,
};
use tower_service::Service;
use worker::{D1Database, Env, HttpRequest, Request, Response, Result};
use super::{accounts, activity_tasks, consents, contact_points, data_quality, identity_resolution, parties, party_relationships, sales_deals};
use crm_parties::PartyKind as DomainPartyKind;
use crm_customer_accounts::{Account as DomainAccount, AccountPartyRole as DomainAccountPartyRole, AccountStatus as DomainAccountStatus};
use crm_contact_points::{ContactPoint as DomainContactPoint, ContactPointKind as DomainContactPointKind,
    ContactPointStatus as DomainContactPointStatus};
use crm_party_relationships::{
    PartyRelationship as DomainRelationship,
    RelationshipDirectionality as DomainDirectionality,
    PartyRelationshipStatus as DomainRelationshipStatus,
};
use crm_activities::{
    Task as DomainTask, TaskPriority as DomainTaskPriority,
    TaskStatus as DomainTaskStatus,
};
use crm_identity_resolution::{CanonicalPartyPair, PartyReference};
use crm_sales::{Deal as DomainDeal,DealStatus as DomainDealStatus};
use crm_consents::{
    CommunicationAuthorizationReason as DomainConsentReason,
    CommunicationChannel as DomainConsentChannel,
    ConsentAuthorization as DomainConsent,
    ConsentAuthorizationStatus as DomainConsentStatus,
    ConsentEffect as DomainConsentEffect,
};
use buffa_types::google::protobuf::Timestamp;

// The published crm.core.v1 package shadows Rust's 'core' crate name.
// Mount the UNMODIFIED generated Buffa package stitchers and expose the
// standard library fmt at the enclosing package. No runtime adapter/codegen
// rewrite is required; all message and service code remains generated.
pub mod proto {
    #[allow(unused_imports)]
    pub mod crm {
        use super::*;
        pub mod core {
            use super::*;
            pub use ::core::fmt;
            pub mod v2 {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/crm.core.v2.mod.rs"));
            }
        }
                pub mod sales {
            use super::*;
            pub mod v2 {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/crm.sales.v2.mod.rs"));
            }
        }
        pub mod accounts {
            use super::*;
            pub mod v2 {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/crm.accounts.v2.mod.rs"));
            }
        }
        pub mod contact_points {
            use super::*;
            pub mod v2 {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/crm.contact_points.v2.mod.rs"));
            }
        }
        pub mod party_relationships {
            use super::*;
            pub mod v2 {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/crm.party_relationships.v2.mod.rs"));
            }
        }
        pub mod activities {
            use super::*;
            pub mod v2 {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/crm.activities.v2.mod.rs"));
            }
        }
        pub mod data_quality {
            use super::*;
            pub mod v2 {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/crm.data_quality.v2.mod.rs"));
            }
        }
        pub mod identity_resolution {
            use super::*;
            pub mod v2 {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/crm.identity_resolution.v2.mod.rs"));
            }
        }
        pub mod customer_360 {
            use super::*;
            pub mod v2 {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/crm.customer_360.v2.mod.rs"));
            }
        }
        pub mod customer_data_operations {
            use super::*;
            pub mod v2 {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/crm.customer_data_operations.v2.mod.rs"));
            }
        }
        pub mod customer_enrichment {
            use super::*;
            pub mod v2 {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/crm.customer_enrichment.v2.mod.rs"));
            }
        }
        pub mod customer_privacy {
            use super::*;
            pub mod v2 {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/crm.customer_privacy.v2.mod.rs"));
            }
        }
        pub mod consents {
            use super::*;
            pub mod v2 {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/crm.consents.v2.mod.rs"));
            }
        }
        pub mod parties {
            use super::*;
            pub mod v2 {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/crm.parties.v2.mod.rs"));
            }
            pub mod v3 {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/crm.parties.v3.mod.rs"));
            }
        }
    }
}
use proto::crm::sales::v2 as sale_wire;
use proto::crm::parties::v2 as party_wire;
use proto::crm::parties::v3 as party_v3;
use proto::crm::core::v2 as core_v2;
use proto::crm::accounts::v2 as account_wire;
use proto::crm::contact_points::v2 as contact_wire;
use proto::crm::party_relationships::v2 as rel_wire;
use proto::crm::activities::v2 as task_wire;
use proto::crm::data_quality::v2 as quality_wire;
use proto::crm::identity_resolution::v2 as identity_wire;
use proto::crm::consents::v2 as consent_wire;

// Domain Connect traits live in native Rust submodules; one router below.
mod party_handlers;
mod account_handlers;
mod contact_handlers;
mod relationship_handlers;
mod task_handlers;
mod quality_handlers;
mod identity_handlers;
mod consent_handlers;
mod sales_handlers;
mod overview_handlers;
use overview_handlers::Customer360V2Service;
use proto::crm::customer_360::v2 as overview_wire;
mod data_operations_handlers;
use data_operations_handlers::PartyCsvV2Service;
use proto::crm::customer_data_operations::v2 as csv_wire;
mod enrichment_handlers;
use enrichment_handlers::EnrichmentV2Service;
use proto::crm::customer_enrichment::v2 as enrichment_wire;
mod privacy_handlers;
use privacy_handlers::PrivacyV2Service;
use proto::crm::customer_privacy::v2 as privacy_wire;

struct PartyService {
    env: Arc<Env>,
    tenant: String,
}

// Generated Connect-Rust + Buffa response, original Party owner + one D1.
impl party_wire::PartyService for PartyService {
    #[worker::send]
    async fn get_party(
        &self, _ctx: RequestContext, req: ServiceRequest<'_, party_wire::GetPartyRequest>,
    ) -> ServiceResult<party_wire::GetPartyResponse> {
        let raw = req.party_id.trim();
        if raw.is_empty() || raw.len() > 180 || raw != req.party_id || raw.contains('/') {
            return Err(ConnectError::invalid_argument("PARTIES_PARTY_ID_INVALID"));
        }
        let id = crm_parties::PartyId::try_new(raw.to_owned())
            .map_err(|_| ConnectError::invalid_argument("PARTIES_PARTY_ID_INVALID"))?;
        let db = self.env.d1("DB")
            .map_err(|_| ConnectError::internal("D1 binding unavailable"))?;
        let party = parties::stored(&db, &self.tenant, id.as_str()).await
            .map_err(|_| ConnectError::internal("Party D1 read failed"))?
            .ok_or_else(|| ConnectError::not_found("PARTIES_NOT_FOUND"))?;
        let version = i64::try_from(party.version())
            .map_err(|_| ConnectError::internal("invalid Party version"))?;
        let kind = match party.kind() {
            DomainPartyKind::Person => party_wire::PartyKind::PARTY_KIND_PERSON,
            DomainPartyKind::Organization => party_wire::PartyKind::PARTY_KIND_ORGANIZATION,
        };
        RpcResponse::ok(party_wire::GetPartyResponse {
            party: Some(party_wire::Party {
                party_id: party.party_id().as_str().to_owned(),
                kind: kind.into(),
                display_name: party.display_name().to_owned(),
                version,
                ..Default::default()
            }).into(),
            ..Default::default()
        })
    }
}

// Shared validation of the wire precondition; domain owners still perform
// authoritative CAS, idempotent replay and SQL/audit checks.
fn wire_precondition(
        pre:Option<&core_v2::MutationPrecondition>,create:bool
    )->std::result::Result<(String,Option<i64>),ConnectError>{
        let pre=pre.ok_or_else(||ConnectError::invalid_argument("precondition required"))?;
        if !(8..=120).contains(&pre.idempotency_key.len()){
            return Err(ConnectError::invalid_argument("IDEMPOTENCY_KEY_REQUIRED"));
        }
        match (create,pre.expected_version){
            (true,None)=>Ok((pre.idempotency_key.clone(),None)),
            (false,Some(version)) if version>0=>Ok((pre.idempotency_key.clone(),Some(version))),
            _=>Err(ConnectError::invalid_argument("invalid mutation version")),
        }
    }

// Cutover Party v3: generated Connect wire only. The historical Party owner
// in parties.rs remains the SOLE state codec, D1 mutation and receipt owner.
// This service is exposed to signed native-ci workerd only until full cutover.
struct PartyV3Service {
    env: Arc<Env>,
    tenant: String,
    request_id: String,
}
impl PartyV3Service {
    fn d1(&self) -> std::result::Result<D1Database, ConnectError> {
        self.env.d1("DB").map_err(|_|ConnectError::internal("D1 binding unavailable"))
    }
    fn valid_id(raw: &str) -> std::result::Result<crm_parties::PartyId, ConnectError> {
        if raw != raw.trim() || raw.contains('/') {
            return Err(ConnectError::invalid_argument("PARTIES_PARTY_ID_INVALID"));
        }
        crm_parties::PartyId::try_new(raw.to_owned())
            .map_err(|_|ConnectError::invalid_argument("PARTIES_PARTY_ID_INVALID"))
    }
    fn convert(party: &crm_parties::Party)
        -> std::result::Result<party_v3::Party,ConnectError> {
        let version=i64::try_from(party.version())
            .map_err(|_|ConnectError::internal("invalid Party version"))?;
        let kind=match party.kind(){
            DomainPartyKind::Person=>party_v3::PartyKind::PARTY_KIND_PERSON,
            DomainPartyKind::Organization=>party_v3::PartyKind::PARTY_KIND_ORGANIZATION,
        };
        Ok(party_v3::Party {
            party_id:party.party_id().as_str().to_owned(),
            kind:kind.into(),display_name:party.display_name().to_owned(),
            version,..Default::default()
        })
    }
    fn receipt(&self, outcome:&parties::MutationOutcome)
        -> std::result::Result<core_v2::MutationReceipt,ConnectError> {
        Ok(core_v2::MutationReceipt {
            request_id:self.request_id.clone(),
            resource_version:i64::try_from(outcome.party.version())
                .map_err(|_|ConnectError::internal("invalid Party version"))?,
            replayed:outcome.replayed,..Default::default()
        })
    }
    fn mutation_error(f: parties::MutationFailure) -> ConnectError {
        match f.status {
            404=>ConnectError::not_found(f.code),
            409=>ConnectError::aborted(f.code),
            422=>ConnectError::invalid_argument(f.code),
            _=>ConnectError::internal(f.code),
        }
    }
}


// One new Connect surface for the EXISTING customer-accounts Rust owner.
// All commands and D1 writes reside in accounts.rs; no parallel JSON handler.
struct AccountV2Service {
    env: Arc<Env>,
    tenant: String,
    request_id: String,
}
impl AccountV2Service {
    fn d1(&self)->std::result::Result<D1Database,ConnectError>{
        self.env.d1("DB").map_err(|_|ConnectError::internal("D1 unavailable"))
    }
    fn valid_id(id:&str)->std::result::Result<String,ConnectError>{
        if id!=id.trim()||id.contains('/') {
            return Err(ConnectError::invalid_argument("CUSTOMER_ACCOUNTS_ID_INVALID"));
        }
        let value=crm_customer_accounts::AccountId::try_new(id)
            .map_err(|_|ConnectError::invalid_argument("CUSTOMER_ACCOUNTS_ID_INVALID"))?;
        Ok(value.as_str().to_owned())
    }
    fn convert(account:&DomainAccount)
        ->std::result::Result<account_wire::Account,ConnectError>{
        let status=match account.status(){
            DomainAccountStatus::Active=>account_wire::AccountStatus::ACCOUNT_STATUS_ACTIVE,
            DomainAccountStatus::Inactive=>account_wire::AccountStatus::ACCOUNT_STATUS_INACTIVE,
        };
        let parties=account.party_associations().iter().map(|association|{
            let role=match association.role(){
                DomainAccountPartyRole::Primary=>account_wire::AccountPartyRole::ACCOUNT_PARTY_ROLE_PRIMARY,
                DomainAccountPartyRole::Member=>account_wire::AccountPartyRole::ACCOUNT_PARTY_ROLE_MEMBER,
            };
            account_wire::AccountPartyAssociation{
                party_id:association.party().as_str().to_owned(),role:role.into(),
                ..Default::default()
            }
        }).collect();
        Ok(account_wire::Account {
            account_id:account.account_id().as_str().to_owned(),
            name:account.name().to_owned(),status:status.into(),parties,
            version:i64::try_from(account.version())
                .map_err(|_|ConnectError::internal("invalid Account version"))?,
            ..Default::default()
        })
    }
    fn receipt(&self,outcome:&accounts::MutationOutcome)
        ->std::result::Result<core_v2::MutationReceipt,ConnectError>{
        Ok(core_v2::MutationReceipt {
            request_id:self.request_id.clone(),
            resource_version:i64::try_from(outcome.account.version())
                .map_err(|_|ConnectError::internal("invalid Account version"))?,
            replayed:outcome.replayed,..Default::default()
        })
    }
    fn failed(f:accounts::MutationFailure)->ConnectError{
        match f.status{
            404=>ConnectError::not_found(f.code),
            409=>ConnectError::aborted(f.code),
            422=>ConnectError::invalid_argument(f.code),
            _=>ConnectError::internal(f.code),
        }
    }

}


// Connect wire for ORIGINAL crm-contact-points (no browser verification claim).
struct ContactPointV2Service{
    env:Arc<Env>,
    tenant:String,
    request_id:String,
}
impl ContactPointV2Service{
    fn d1(&self)->std::result::Result<D1Database,ConnectError>{
        self.env.d1("DB").map_err(|_|ConnectError::internal("D1 unavailable"))
    }
    fn valid_id(id:&str)->std::result::Result<String,ConnectError>{
        if id!=id.trim()||id.contains('/') {
            return Err(ConnectError::invalid_argument("CONTACT_POINTS_ID_INVALID"));
        }
        let value=crm_contact_points::ContactPointId::try_new(id)
            .map_err(|_|ConnectError::invalid_argument("CONTACT_POINTS_ID_INVALID"))?;
        Ok(value.as_str().to_owned())
    }
    fn nanos(ts:&Timestamp)->std::result::Result<i64,ConnectError>{
        if !(0..1_000_000_000).contains(&ts.nanos){
            return Err(ConnectError::invalid_argument("invalid timestamp nanos"));
        }
        ts.seconds.checked_mul(1_000_000_000)
            .and_then(|s|s.checked_add(i64::from(ts.nanos)))
            .ok_or_else(||ConnectError::invalid_argument("timestamp outside domain range"))
    }
    fn timestamp(nanos:i64)->Timestamp{
        Timestamp{
            seconds:nanos.div_euclid(1_000_000_000),
            nanos:nanos.rem_euclid(1_000_000_000) as i32,
            ..Default::default()
        }
    }
    fn time_patch(patch:Option<&core_v2::TimestampPatch>)
        ->std::result::Result<contact_points::TimestampChange,ConnectError>{
        use contact_points::TimestampChange;
        let Some(patch)=patch else{return Ok(TimestampChange::Keep)};
        match patch.operation.as_ref(){
            Some(core_v2::timestamp_patch::Operation::Set(ts))=>
                Self::nanos(ts).map(TimestampChange::Set),
            Some(core_v2::timestamp_patch::Operation::Clear(true))=>
                Ok(TimestampChange::Clear),
            _=>Err(ConnectError::invalid_argument("invalid timestamp patch")),
        }
    }
    fn convert(point:&DomainContactPoint)
        ->std::result::Result<contact_wire::ContactPoint,ConnectError>{
        let kind=match point.kind(){
            DomainContactPointKind::Email=>contact_wire::ContactPointKind::CONTACT_POINT_KIND_EMAIL,
            DomainContactPointKind::Phone=>contact_wire::ContactPointKind::CONTACT_POINT_KIND_PHONE,
            DomainContactPointKind::Postal=>contact_wire::ContactPointKind::CONTACT_POINT_KIND_POSTAL,
            DomainContactPointKind::Web=>contact_wire::ContactPointKind::CONTACT_POINT_KIND_WEB,
            DomainContactPointKind::Messaging=>contact_wire::ContactPointKind::CONTACT_POINT_KIND_MESSAGING,
        };
        let status=match point.status(){
            DomainContactPointStatus::Active=>contact_wire::ContactPointStatus::CONTACT_POINT_STATUS_ACTIVE,
            DomainContactPointStatus::Inactive=>contact_wire::ContactPointStatus::CONTACT_POINT_STATUS_INACTIVE,
        };
        Ok(contact_wire::ContactPoint{
            contact_point_id:point.contact_point_id().as_str().to_owned(),
            party_id:point.party_ref().as_str().to_owned(),kind:kind.into(),
            display_value:point.display_value().to_owned(),
            normalized_value:point.normalized_value().to_owned(),
            preferred:point.preferred(),status:status.into(),
            verified:point.verification().is_verified(),
            valid_from:point.valid_from_unix_nanos().map(Self::timestamp).into(),
            valid_until:point.valid_until_unix_nanos().map(Self::timestamp).into(),
            version:i64::try_from(point.version())
                .map_err(|_|ConnectError::internal("invalid ContactPoint version"))?,
            ..Default::default()
        })
    }
    fn receipt(&self,outcome:&contact_points::MutationOutcome)
        ->std::result::Result<core_v2::MutationReceipt,ConnectError>{
        Ok(core_v2::MutationReceipt{
            request_id:self.request_id.clone(),
            resource_version:i64::try_from(outcome.contact_point.version())
                .map_err(|_|ConnectError::internal("invalid ContactPoint version"))?,
            replayed:outcome.replayed,..Default::default()
        })
    }
    fn failed(f:contact_points::MutationFailure)->ConnectError{
        match f.status{
            404=>ConnectError::not_found(f.code),
            409=>ConnectError::aborted(f.code),
            422=>ConnectError::invalid_argument(f.code),
            _=>ConnectError::internal(f.code),
        }
    }

}


// Generated Connect wire for ORIGINAL crm-party-relationships. No second
// persistence path: party_relationships.rs owns D1, domain invariants and
// receipts; this service only checks wire fields and maps typed commands.
struct RelationshipV2Service {
    env:Arc<Env>,tenant:String,request_id:String,
}
impl RelationshipV2Service {
    fn d1(&self)->std::result::Result<D1Database,ConnectError>{
        self.env.d1("DB").map_err(|_|ConnectError::internal("D1 unavailable"))
    }
    fn valid_id(raw:&str)->std::result::Result<String,ConnectError>{
        if raw!=raw.trim()||raw.contains('/') {
            return Err(ConnectError::invalid_argument("PARTY_RELATIONSHIPS_RELATIONSHIP_ID_INVALID"));
        }
        crm_party_relationships::PartyRelationshipId::try_new(raw.to_owned())
            .map(|x|x.as_str().to_owned())
            .map_err(|_|ConnectError::invalid_argument("PARTY_RELATIONSHIPS_RELATIONSHIP_ID_INVALID"))
    }
    fn patch(patch:Option<&core_v2::TimestampPatch>)
        ->std::result::Result<party_relationships::TimestampChange,ConnectError>{
        use party_relationships::TimestampChange;
        let Some(patch)=patch else{return Ok(TimestampChange::Keep)};
        match patch.operation.as_ref(){
            Some(core_v2::timestamp_patch::Operation::Set(ts))=>
                ContactPointV2Service::nanos(ts).map(TimestampChange::Set),
            Some(core_v2::timestamp_patch::Operation::Clear(true))=>
                Ok(TimestampChange::Clear),
            _=>Err(ConnectError::invalid_argument("invalid timestamp patch")),
        }
    }
    fn convert(relation:&DomainRelationship)
        ->std::result::Result<rel_wire::PartyRelationship,ConnectError>{
        let rtype=relation.relationship_type();
        let directionality=match rtype.directionality(){
            DomainDirectionality::Directional=>
                rel_wire::RelationshipDirectionality::RELATIONSHIP_DIRECTIONALITY_DIRECTIONAL,
            DomainDirectionality::Reciprocal=>
                rel_wire::RelationshipDirectionality::RELATIONSHIP_DIRECTIONALITY_RECIPROCAL,
        };
        let status=match relation.status(){
            DomainRelationshipStatus::Active=>
                rel_wire::RelationshipStatus::RELATIONSHIP_STATUS_ACTIVE,
            DomainRelationshipStatus::Inactive=>
                rel_wire::RelationshipStatus::RELATIONSHIP_STATUS_INACTIVE,
        };
        Ok(rel_wire::PartyRelationship{
            relationship_id:relation.party_relationship_id().as_str().to_owned(),
            from_party_id:relation.from_party_ref().as_str().to_owned(),
            to_party_id:relation.to_party_ref().as_str().to_owned(),
            r#type:Some(rel_wire::RelationshipType{
                code:rtype.code().to_owned(),directionality:directionality.into(),
                from_role:rtype.from_role().to_owned(),to_role:rtype.to_role().to_owned(),
                ..Default::default()
            }).into(),
            status:status.into(),
            valid_from:relation.valid_from_unix_nanos()
                .map(ContactPointV2Service::timestamp).into(),
            valid_until:relation.valid_until_unix_nanos()
                .map(ContactPointV2Service::timestamp).into(),
            version:relation.version(),..Default::default()
        })
    }
    fn receipt(&self,out:&party_relationships::MutationOutcome)->core_v2::MutationReceipt{
        core_v2::MutationReceipt{
            request_id:self.request_id.clone(),resource_version:out.relationship.version(),
            replayed:out.replayed,..Default::default()
        }
    }
    fn failed(f:party_relationships::MutationFailure)->ConnectError{
        match f.status{
            404=>ConnectError::not_found(f.code),
            409=>ConnectError::aborted(f.code),
            422=>ConnectError::invalid_argument(f.code),
            _=>ConnectError::internal(f.code),
        }
    }
}


// Native generated Connect service for the ORIGINAL crm-activities Task.
// Same Task codec, D1 table, CAS and receipts as the blocked historical HTTP.
struct TaskV2Service {
    env:Arc<Env>,tenant:String,actor:String,request_id:String,
}
impl TaskV2Service {
    fn d1(&self)->std::result::Result<D1Database,ConnectError>{
        self.env.d1("DB").map_err(|_|ConnectError::internal("D1 unavailable"))
    }
    fn valid_id(raw:&str)->std::result::Result<String,ConnectError>{
        if raw!=raw.trim()||raw.contains('/') {
            return Err(ConnectError::invalid_argument("ACTIVITIES_TASK_ID_INVALID"));
        }
        crm_module_sdk::RecordId::try_new(raw.to_owned())
            .map(|x|x.as_str().to_owned())
            .map_err(|_|ConnectError::invalid_argument("ACTIVITIES_TASK_ID_INVALID"))
    }
    fn convert(task:&DomainTask)->std::result::Result<task_wire::Task,ConnectError>{
        let priority=match task.priority(){
            DomainTaskPriority::Low=>task_wire::TaskPriority::TASK_PRIORITY_LOW,
            DomainTaskPriority::Normal=>task_wire::TaskPriority::TASK_PRIORITY_NORMAL,
            DomainTaskPriority::High=>task_wire::TaskPriority::TASK_PRIORITY_HIGH,
            DomainTaskPriority::Urgent=>task_wire::TaskPriority::TASK_PRIORITY_URGENT,
        };
        let status=match task.status(){
            DomainTaskStatus::Open=>task_wire::TaskStatus::TASK_STATUS_OPEN,
            DomainTaskStatus::Completed=>task_wire::TaskStatus::TASK_STATUS_COMPLETED,
        };
        let account_id=task.related_resources().iter().find(
            |item|item.resource_type=="crm.customer-accounts")
            .ok_or_else(||ConnectError::internal("original Task missing Account reference"))?
            .resource_id.clone();
        Ok(task_wire::Task{
            task_id:task.task_id().as_str().to_owned(),
            subject:task.subject().to_owned(),
            description:task.description().unwrap_or_default().to_owned(),
            account_id,priority:priority.into(),status:status.into(),
            due_at:task.due_at_unix_nanos()
                .map(ContactPointV2Service::timestamp).into(),
            reminder_at:task.reminder_at_unix_nanos()
                .map(ContactPointV2Service::timestamp).into(),
            completed_at:task.completed_at_unix_nanos()
                .map(ContactPointV2Service::timestamp).into(),
            version:task.version(),..Default::default()
        })
    }
    fn receipt(&self,out:&activity_tasks::MutationOutcome)->core_v2::MutationReceipt{
        core_v2::MutationReceipt{
            request_id:self.request_id.clone(),resource_version:out.task.version(),
            replayed:out.replayed,..Default::default()
        }
    }
    fn failed(f:activity_tasks::MutationFailure)->ConnectError{
        match f.status{
            404=>ConnectError::not_found(f.code),
            409=>ConnectError::aborted(f.code),
            422=>ConnectError::invalid_argument(f.code),
            _=>ConnectError::internal(f.code),
        }
    }
}



// Original read-only crm-data-quality: generated Protobuf, original rule set,
// tenant-scoped Party read. No shadow rule engine or persisted quality score.
struct DataQualityV2Service{
    env:Arc<Env>,
    tenant:String,
}


// Read-only crm-identity-resolution. Same original discovered observations,
// same tenant-bound D1. Identical email does NOT confirm identity.
struct IdentityV2Service{
    env:Arc<Env>,
    tenant:String,
}



// The original crm-consents domain owns communication decisions and operator
// suppression. This generated Connect boundary never accepts evidence/GRANT.
struct ConsentV2Service{
    env:Arc<Env>,
    tenant:String,
    actor:String,
    request_id:String,
}
impl ConsentV2Service{
    fn d1(&self)->std::result::Result<D1Database,ConnectError>{
        self.env.d1("DB").map_err(|_|ConnectError::internal("D1 unavailable"))
    }
    fn channel(raw:buffa::EnumValue<consent_wire::CommunicationChannel>)->std::result::Result<consents::ChannelInput,ConnectError>{
        match raw{
            x if x==consent_wire::CommunicationChannel::COMMUNICATION_CHANNEL_EMAIL=>
                Ok(consents::ChannelInput::Email),
            x if x==consent_wire::CommunicationChannel::COMMUNICATION_CHANNEL_PHONE=>
                Ok(consents::ChannelInput::Phone),
            x if x==consent_wire::CommunicationChannel::COMMUNICATION_CHANNEL_SMS=>
                Ok(consents::ChannelInput::Sms),
            x if x==consent_wire::CommunicationChannel::COMMUNICATION_CHANNEL_POSTAL=>
                Ok(consents::ChannelInput::Postal),
            x if x==consent_wire::CommunicationChannel::COMMUNICATION_CHANNEL_MESSAGING=>
                Ok(consents::ChannelInput::Messaging),
            x if x==consent_wire::CommunicationChannel::COMMUNICATION_CHANNEL_PUSH=>
                Ok(consents::ChannelInput::Push),
            _=>Err(ConnectError::invalid_argument("invalid CommunicationChannel")),
        }
    }
    fn convert(item:&DomainConsent)->consent_wire::ConsentAuthorization{
        let channel=match item.channel(){
            DomainConsentChannel::Email=>consent_wire::CommunicationChannel::COMMUNICATION_CHANNEL_EMAIL,
            DomainConsentChannel::Phone=>consent_wire::CommunicationChannel::COMMUNICATION_CHANNEL_PHONE,
            DomainConsentChannel::Sms=>consent_wire::CommunicationChannel::COMMUNICATION_CHANNEL_SMS,
            DomainConsentChannel::Postal=>consent_wire::CommunicationChannel::COMMUNICATION_CHANNEL_POSTAL,
            DomainConsentChannel::Messaging=>consent_wire::CommunicationChannel::COMMUNICATION_CHANNEL_MESSAGING,
            DomainConsentChannel::Push=>consent_wire::CommunicationChannel::COMMUNICATION_CHANNEL_PUSH,
        };
        let effect=match item.effect(){
            DomainConsentEffect::Grant=>consent_wire::ConsentEffect::CONSENT_EFFECT_GRANT,
            DomainConsentEffect::Deny=>consent_wire::ConsentEffect::CONSENT_EFFECT_DENY,
        };
        let status=match item.status(){
            DomainConsentStatus::Active=>consent_wire::ConsentStatus::CONSENT_STATUS_ACTIVE,
            DomainConsentStatus::Withdrawn=>consent_wire::ConsentStatus::CONSENT_STATUS_WITHDRAWN,
        };
        consent_wire::ConsentAuthorization{
            authorization_id:item.authorization_id().as_str().to_owned(),
            party_id:item.party_ref().as_str().to_owned(),
            contact_point_id:item.contact_point_ref().map(|x|x.as_str().to_owned()),
            purpose:item.purpose().as_str().to_owned(),
            channel:channel.into(),effect:effect.into(),status:status.into(),
            jurisdiction:item.jurisdiction().as_str().to_owned(),
            version:item.version(),..Default::default()
        }
    }
    fn failed(f:consents::OperationFailure)->ConnectError{
        match f.status{
            404=>ConnectError::not_found(f.code),
            409=>ConnectError::aborted(f.code),
            422=>ConnectError::invalid_argument(f.code),
            _=>ConnectError::internal(f.code),
        }
    }
}



// Sales v2 wraps the EXISTING crm-sales aggregate, D1 persistence and atomic
// sales->activities delivery. Neither generated wire nor React implements
// Deal transitions, price rounding, CAS or new business policy.
struct SalesV2Service {
    env:Arc<Env>,
    tenant:String,
    actor:String,
    request_id:String,
}
impl SalesV2Service {
    fn d1(&self)->std::result::Result<D1Database,ConnectError>{
        self.env.d1("DB").map_err(|_|ConnectError::internal("D1 unavailable"))
    }
    fn id(raw:&str)->std::result::Result<String,ConnectError>{
        if raw!=raw.trim()||raw.contains('/'){
            return Err(ConnectError::invalid_argument("SALES_DEAL_ID_INVALID"));
        }
        crm_module_sdk::RecordId::try_new(raw.to_owned())
            .map(|x|x.as_str().to_owned())
            .map_err(|_|ConnectError::invalid_argument("SALES_DEAL_ID_INVALID"))
    }
    fn stage(wire:&sale_wire::DealStage)
        ->std::result::Result<(String,String,u16),ConnectError>{
        let ordinal=u16::try_from(wire.ordinal)
            .map_err(|_|ConnectError::invalid_argument("invalid stage ordinal"))?;
        Ok((wire.pipeline_id.clone(),wire.stage_id.clone(),ordinal))
    }
    fn deal_status(raw:buffa::EnumValue<sale_wire::DealStatus>)
        ->std::result::Result<sales_deals::StatusInput,ConnectError>{
        match raw {
            x if x==sale_wire::DealStatus::DEAL_STATUS_OPEN=>
                Ok(sales_deals::StatusInput::Open),
            x if x==sale_wire::DealStatus::DEAL_STATUS_WON=>
                Ok(sales_deals::StatusInput::Won),
            x if x==sale_wire::DealStatus::DEAL_STATUS_LOST=>
                Ok(sales_deals::StatusInput::Lost),
            _=>Err(ConnectError::invalid_argument("invalid DealStatus")),
        }
    }
    fn convert(deal:&DomainDeal)->std::result::Result<sale_wire::Deal,ConnectError>{
        let stage=deal.stage();
        let status=match deal.status(){
            DomainDealStatus::Open=>sale_wire::DealStatus::DEAL_STATUS_OPEN,
            DomainDealStatus::Won=>sale_wire::DealStatus::DEAL_STATUS_WON,
            DomainDealStatus::Lost=>sale_wire::DealStatus::DEAL_STATUS_LOST,
        };
        let account=deal.account()
            .ok_or_else(||ConnectError::internal("original Deal missing Account"))?;
        let amount=deal.amount().map(|money|{
            let units=i64::try_from(money.minor_units())
                .map_err(|_|ConnectError::internal("original money exceeds wire i64"))?;
            Ok::<core_v2::ExactMoney,ConnectError>(core_v2::ExactMoney{
                minor_units:units,currency_code:money.currency().as_str().to_owned(),
                ..Default::default()
            })
        }).transpose()?;
        Ok(sale_wire::Deal{
            deal_id:deal.deal_id().as_str().to_owned(),
            name:deal.name().to_owned(),account_id:account.resource_id.clone(),
            stage:Some(sale_wire::DealStage{
                pipeline_id:stage.pipeline_id().as_str().to_owned(),
                stage_id:stage.stage_id().as_str().to_owned(),
                ordinal:u32::from(stage.ordinal()),..Default::default()
            }).into(),
            status:status.into(),amount:amount.into(),
            probability_basis_points:u32::from(deal.probability().get()),
            version:deal.version(),
            closed_at:deal.close_outcome().map(|outcome|
                ContactPointV2Service::timestamp(outcome.closed_at_unix_nanos())).into(),
            ..Default::default()
        })
    }
    fn receipt(&self,saved:&sales_deals::SalesSaved)->core_v2::MutationReceipt{
        core_v2::MutationReceipt{
            request_id:self.request_id.clone(),
            resource_version:saved.deal.version(),replayed:saved.replayed,
            ..Default::default()
        }
    }
    fn failed(f:sales_deals::SalesFailure)->ConnectError{
        match f.status{
            404=>ConnectError::not_found(f.code),
            409=>ConnectError::aborted(f.code),
            422=>ConnectError::invalid_argument(f.code),
            _=>ConnectError::internal(f.code),
        }
    }
}


// One constructor serves dispatch and the existing signed workerd diagnostic.
// Router::methods() inspects the registered compiled handlers.
pub(super) fn router(env:Env,tenant:&str,actor:&str,request_id:&str)->ConnectRouter {
    let env = Arc::new(env);
    ConnectRouter::new()
        .add_service(Arc::new(PartyService {
            env: Arc::clone(&env), tenant: tenant.to_owned(),
        }))
        .add_service(Arc::new(PartyV3Service {
            env: Arc::clone(&env), tenant: tenant.to_owned(),
            request_id:request_id.to_owned(),
        }))
        .add_service(Arc::new(AccountV2Service {
            env: Arc::clone(&env), tenant: tenant.to_owned(),
            request_id:request_id.to_owned(),
        }))
        .add_service(Arc::new(ContactPointV2Service {
            env: Arc::clone(&env), tenant: tenant.to_owned(),
            request_id:request_id.to_owned(),
        }))
        .add_service(Arc::new(RelationshipV2Service {
            env: Arc::clone(&env), tenant: tenant.to_owned(),
            request_id:request_id.to_owned(),
        }))
        .add_service(Arc::new(TaskV2Service {
            env: Arc::clone(&env),tenant:tenant.to_owned(),
            actor:actor.to_owned(),request_id:request_id.to_owned(),
        }))
        .add_service(Arc::new(DataQualityV2Service{
            env:Arc::clone(&env),tenant:tenant.to_owned(),
        }))
        .add_service(Arc::new(IdentityV2Service{
            env:Arc::clone(&env),tenant:tenant.to_owned(),
        }))
        .add_service(Arc::new(ConsentV2Service{
            env:Arc::clone(&env),tenant:tenant.to_owned(),
            actor:actor.to_owned(),request_id:request_id.to_owned(),
        }))
        .add_service(Arc::new(Customer360V2Service{
            env:Arc::clone(&env),tenant:tenant.to_owned(),
        }))
        .add_service(Arc::new(PartyCsvV2Service{
            env:Arc::clone(&env),tenant:tenant.to_owned(),request_id:request_id.to_owned(),
        }))
        .add_service(Arc::new(EnrichmentV2Service{
            env:Arc::clone(&env),tenant:tenant.to_owned(),
            actor:actor.to_owned(),request_id:request_id.to_owned(),
        }))
        .add_service(Arc::new(PrivacyV2Service{
            env:Arc::clone(&env),tenant:tenant.to_owned(),
            actor:actor.to_owned(),request_id:request_id.to_owned(),
        }))
        .add_service(Arc::new(SalesV2Service{
            env:Arc::clone(&env),tenant:tenant.to_owned(),
            actor:actor.to_owned(),request_id:request_id.to_owned(),
        }))

}

pub(super) async fn route(
    req:Request,env:Env,tenant:&str,actor:&str,request_id:&str,
)->Result<Response> {
    let mut app:Router=router(env,tenant,actor,request_id).into_axum_router();
    let http=HttpRequest::try_from(req)?;
    Ok(Response::try_from(app.call(http).await?)?)
}
