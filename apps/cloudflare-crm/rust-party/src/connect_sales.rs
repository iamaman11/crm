//! One native Connect transport for original Rust domain services.
//! Cloudflare Access and tenant selection happen BEFORE routing here.
//! Domain rules and storage belong exclusively to existing crm-sales and D1.
use super::{
    access, access_members, access_teams, accounts, activity_tasks, consents, contact_points,
    data_quality, identity_resolution, parties, party_relationships, sales_deals,
    sales_performance, sales_pipelines, sales_routing, sales_territories, sales_views,
};
use axum::Router;
use buffa_types::google::protobuf::Timestamp;
use connectrpc::{
    ConnectError, RequestContext, Response as RpcResponse, Router as ConnectRouter, ServiceRequest,
    ServiceResult,
};
use crm_activities::{
    Task as DomainTask, TaskPriority as DomainTaskPriority, TaskStatus as DomainTaskStatus,
};
use crm_consents::{
    CommunicationAuthorizationReason as DomainConsentReason,
    CommunicationChannel as DomainConsentChannel, ConsentAuthorization as DomainConsent,
    ConsentAuthorizationStatus as DomainConsentStatus, ConsentEffect as DomainConsentEffect,
};
use crm_contact_points::{
    ContactPoint as DomainContactPoint, ContactPointKind as DomainContactPointKind,
    ContactPointStatus as DomainContactPointStatus,
};
use crm_customer_accounts::{
    Account as DomainAccount, AccountPartyRole as DomainAccountPartyRole,
    AccountStatus as DomainAccountStatus,
};
use crm_identity_resolution::{CanonicalPartyPair, PartyReference};
use crm_parties::PartyKind as DomainPartyKind;
use crm_party_relationships::{
    PartyRelationship as DomainRelationship, PartyRelationshipStatus as DomainRelationshipStatus,
    RelationshipDirectionality as DomainDirectionality,
};
use crm_sales::{Deal as DomainDeal, DealStatus as DomainDealStatus};
use std::sync::Arc;
use tower_service::Service;
use worker::{D1Database, Env, HttpRequest, Request, Response, Result};

// Use the standard generated package tree, including generator-owned lint
// attributes. The root import is inherited by its generated `use super::*`
// statements so the crm.core package also resolves Rust's fmt traits.
#[expect(
    clippy::wrong_self_convention,
    reason = "Buffa 0.9.1 generates from_role/from_party_id getters from Protobuf fields; these are not constructors"
)]
pub mod proto {
    use ::core::fmt;
    include!(concat!(env!("OUT_DIR"), "/_connectrpc.rs"));
}
use proto::crm::access::v2 as member_wire;
use proto::crm::accounts::v2 as account_wire;
use proto::crm::activities::v2 as task_wire;
use proto::crm::activities::v2 as interaction_wire;
use proto::crm::consents::v2 as consent_wire;
use proto::crm::contact_points::v2 as contact_wire;
use proto::crm::core::v2 as core_v2;
use proto::crm::data_quality::v2 as quality_wire;
use proto::crm::identity_resolution::v2 as identity_wire;
use proto::crm::parties::v3 as party_v3;
use proto::crm::party_relationships::v2 as rel_wire;
use proto::crm::sales::v2 as sale_wire;

// Domain Connect traits live in native Rust submodules; one router below.
mod account_handlers;
mod consent_handlers;
mod contact_handlers;
mod identity_handlers;
mod interaction_handlers;
mod member_handlers;
mod overview_handlers;
mod party_handlers;
mod quality_handlers;
mod relationship_handlers;
mod sales_handlers;
mod task_handlers;
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

// The generated core.v2 oneof is the sole new owner wire contract for Tasks
// and Deals. The old Task scalar tags are decoded only at this boundary; both
// paths produce the exact same domain input/digest and no extra business state.
fn wire_owner(value: core_v2::actor_or_team_owner::Owner) -> core_v2::ActorOrTeamOwner {
    core_v2::ActorOrTeamOwner {
        owner: Some(value),
        ..Default::default()
    }
}

fn task_owner_intent(
    owner: Option<&core_v2::ActorOrTeamOwner>,
    assignee_email: Option<String>,
    team_id: Option<String>,
) -> std::result::Result<(Option<String>, Option<String>), ConnectError> {
    use core_v2::actor_or_team_owner::Owner;
    if let Some(owner) = owner {
        if assignee_email.is_some() || team_id.is_some() {
            return Err(ConnectError::invalid_argument("ACTIVITIES_OWNER_CONFLICT"));
        }
        return match owner.owner.as_ref() {
            Some(Owner::AssigneeIdentity(value)) => Ok((Some(value.clone()), None)),
            Some(Owner::TeamId(value)) => Ok((None, Some(value.clone()))),
            None => Err(ConnectError::invalid_argument("ACTIVITIES_OWNER_REQUIRED")),
        };
    }
    if assignee_email.is_some() && team_id.is_some() {
        return Err(ConnectError::invalid_argument("ACTIVITIES_OWNER_CONFLICT"));
    }
    Ok((assignee_email, team_id))
}

// Shared validation of the wire precondition; domain owners still perform
// authoritative CAS, idempotent replay and SQL/audit checks.
fn wire_precondition(
    pre: Option<&core_v2::MutationPrecondition>,
    create: bool,
) -> std::result::Result<(String, Option<i64>), ConnectError> {
    let pre = pre.ok_or_else(|| ConnectError::invalid_argument("precondition required"))?;
    if !(8..=120).contains(&pre.idempotency_key.len()) {
        return Err(ConnectError::invalid_argument("IDEMPOTENCY_KEY_REQUIRED"));
    }
    match (create, pre.expected_version) {
        (true, None) => Ok((pre.idempotency_key.clone(), None)),
        (false, Some(version)) if version > 0 => Ok((pre.idempotency_key.clone(), Some(version))),
        _ => Err(ConnectError::invalid_argument("invalid mutation version")),
    }
}

// S5: one read-only, tenant-bound lookup for original owner receipts.
// Receipt provenance is written by the SAME trigger as domain state and audit;
// pre-0017 rows deliberately have NULL provenance and cannot certify a commit.
#[derive(Clone, Copy)]
enum CrudReceiptOwner {
    Party,
    Account,
    Contact,
    Relationship,
    Task,
    Interaction,
    Privacy,
    Consent,
    Pipeline,
    SalesView,
    Territory,
    AccountAssignment,
    RoutingRule,
    Quota,
    PerformanceReport,
    Team,
}
impl CrudReceiptOwner {
    fn sql(self) -> &'static str {
        match self {
            Self::Party => concat!(
                "SELECT state,original_request_id FROM crm_party_receipts",
                " WHERE tenant=? AND key=?"
            ),
            Self::Account => concat!(
                "SELECT state,original_request_id FROM crm_account_receipts",
                " WHERE tenant=? AND key=?"
            ),
            Self::Contact => concat!(
                "SELECT state,original_request_id FROM crm_contact_point_receipts",
                " WHERE tenant=? AND key=?"
            ),
            Self::Relationship => concat!(
                "SELECT state,original_request_id FROM crm_party_relationship_receipts",
                " WHERE tenant=? AND key=?"
            ),
            Self::Task => concat!(
                "SELECT state,original_request_id FROM crm_activity_task_receipts",
                " WHERE tenant=? AND key=?"
            ),
            Self::Interaction => concat!(
                "SELECT state,original_request_id FROM crm_activity_interaction_receipts",
                " WHERE tenant=? AND key=?"
            ),
            Self::Privacy => concat!(
                "SELECT state,original_request_id FROM crm_privacy_case_receipts",
                " WHERE tenant=? AND key=?"
            ),
            Self::Consent => concat!(
                "SELECT state,original_request_id FROM crm_consent_receipts",
                " WHERE tenant=? AND key=?"
            ),
            Self::Pipeline => concat!(
                "SELECT state,original_request_id FROM crm_sales_pipeline_receipts",
                " WHERE tenant=? AND key=?"
            ),
            Self::Team => concat!(
                "SELECT state,original_request_id FROM crm_team_receipts",
                " WHERE tenant=? AND key=?"
            ),
            Self::Territory => concat!(
                "SELECT state,original_request_id FROM crm_sales_territory_receipts",
                " WHERE tenant=? AND key=?"
            ),
            Self::AccountAssignment => concat!(
                "SELECT state,original_request_id FROM crm_sales_account_assignment_receipts",
                " WHERE tenant=? AND key=?"
            ),
            Self::Quota => concat!(
                "SELECT state,original_request_id FROM crm_sales_quota_receipts",
                " WHERE tenant=? AND key=?"
            ),
            Self::PerformanceReport => concat!(
                "SELECT state,request_id AS original_request_id FROM crm_sales_performance_reports",
                " WHERE tenant=? AND idem_key=?"
            ),
            Self::RoutingRule => concat!(
                "SELECT state,original_request_id FROM crm_sales_routing_rule_receipts",
                " WHERE tenant=? AND key=?"
            ),
            Self::SalesView => concat!(
                "SELECT state,original_request_id FROM crm_sales_saved_view_receipts",
                " WHERE tenant=? AND key=?"
            ),
        }
    }
}
#[derive(serde::Deserialize)]
struct CrudReceiptRow {
    state: String,
    original_request_id: Option<String>,
}
async fn crud_receipt(
    db: &D1Database,
    tenant: &str,
    key: &str,
    owner: CrudReceiptOwner,
) -> worker::Result<Option<CrudReceiptRow>> {
    super::bind(
        db,
        owner.sql(),
        &[
            wasm_bindgen::JsValue::from_str(tenant),
            wasm_bindgen::JsValue::from_str(key),
        ],
    )?
    .first::<CrudReceiptRow>(None)
    .await
}
fn receipt_key(raw: Option<&str>) -> std::result::Result<Option<&str>, ConnectError> {
    match raw {
        None => Ok(None),
        Some(key) if (8..=120).contains(&key.len()) => Ok(Some(key)),
        _ => Err(ConnectError::invalid_argument("IDEMPOTENCY_KEY_REQUIRED")),
    }
}
fn crud_commit_evidence(
    row: Option<CrudReceiptRow>,
    requested_id: &str,
    read_request_id: &str,
    decode: impl FnOnce(&str) -> worker::Result<(String, i64)>,
) -> worker::Result<Option<core_v2::MutationCommitEvidence>> {
    let Some(row) = row else {
        return Ok(None);
    };
    // Receipts predating an owner's provenance migration remain unprovable.
    let Some(original) = row.original_request_id.filter(|id| !id.is_empty()) else {
        return Ok(None);
    };
    // Original domain snapshot codec checks version and resource identity;
    // never use the current mutable record version to prove an older commit.
    let (receipt_id, version) = decode(&row.state)?;
    if receipt_id != requested_id || version <= 0 {
        return Ok(None);
    }
    Ok(Some(core_v2::MutationCommitEvidence {
        request_id: read_request_id.to_owned(),
        original_request_id: original,
        resource_version: version,
        ..Default::default()
    }))
}

fn wire_search(value: &str) -> std::result::Result<&str, ConnectError> {
    if value.chars().count() > 200 {
        return Err(ConnectError::invalid_argument(
            "search exceeds 200 characters",
        ));
    }
    Ok(value.trim())
}

// Party v3 uses the original owner codec, D1 mutation and receipt owner.
struct PartyV3Service {
    env: Arc<Env>,
    actor: access::Actor,
    request_id: String,
}

impl OwnedService for PartyV3Service {
    const OWNER: &'static str = crm_parties::MODULE_ID;
}
impl PartyV3Service {
    fn d1(&self) -> std::result::Result<D1Database, ConnectError> {
        self.env
            .d1("DB")
            .map_err(|_| ConnectError::internal("D1 binding unavailable"))
    }
    fn valid_id(raw: &str) -> std::result::Result<crm_parties::PartyId, ConnectError> {
        if raw != raw.trim() || raw.contains('/') {
            return Err(ConnectError::invalid_argument("PARTIES_PARTY_ID_INVALID"));
        }
        crm_parties::PartyId::try_new(raw.to_owned())
            .map_err(|_| ConnectError::invalid_argument("PARTIES_PARTY_ID_INVALID"))
    }
    fn convert(party: &crm_parties::Party) -> std::result::Result<party_v3::Party, ConnectError> {
        let version = party.version();
        let kind = match party.kind() {
            DomainPartyKind::Person => party_v3::PartyKind::PARTY_KIND_PERSON,
            DomainPartyKind::Organization => party_v3::PartyKind::PARTY_KIND_ORGANIZATION,
        };
        Ok(party_v3::Party {
            party_id: party.party_id().as_str().to_owned(),
            kind: kind.into(),
            display_name: party.display_name().to_owned(),
            version,
            ..Default::default()
        })
    }
    fn receipt(
        &self,
        outcome: &parties::MutationOutcome,
    ) -> std::result::Result<core_v2::MutationReceipt, ConnectError> {
        Ok(core_v2::MutationReceipt {
            request_id: self.request_id.clone(),
            resource_version: outcome.party.version(),
            replayed: outcome.replayed,
            original_request_id: outcome.original_request_id.clone(),
            ..Default::default()
        })
    }
}

// One new Connect surface for the EXISTING customer-accounts Rust owner.
// All commands and D1 writes reside in accounts.rs; no parallel JSON handler.
struct AccountV2Service {
    env: Arc<Env>,
    actor: access::Actor,
    request_id: String,
}

impl OwnedService for AccountV2Service {
    const OWNER: &'static str = crm_customer_accounts::MODULE_ID;
}
impl AccountV2Service {
    fn d1(&self) -> std::result::Result<D1Database, ConnectError> {
        self.env
            .d1("DB")
            .map_err(|_| ConnectError::internal("D1 unavailable"))
    }
    fn valid_id(id: &str) -> std::result::Result<String, ConnectError> {
        if id != id.trim() || id.contains('/') {
            return Err(ConnectError::invalid_argument(
                "CUSTOMER_ACCOUNTS_ID_INVALID",
            ));
        }
        let value = crm_customer_accounts::AccountId::try_new(id)
            .map_err(|_| ConnectError::invalid_argument("CUSTOMER_ACCOUNTS_ID_INVALID"))?;
        Ok(value.as_str().to_owned())
    }
    fn convert(
        account: &DomainAccount,
    ) -> std::result::Result<account_wire::Account, ConnectError> {
        let status = match account.status() {
            DomainAccountStatus::Active => account_wire::AccountStatus::ACCOUNT_STATUS_ACTIVE,
            DomainAccountStatus::Inactive => account_wire::AccountStatus::ACCOUNT_STATUS_INACTIVE,
        };
        let parties = account
            .party_associations()
            .iter()
            .map(|association| {
                let role = match association.role() {
                    DomainAccountPartyRole::Primary => {
                        account_wire::AccountPartyRole::ACCOUNT_PARTY_ROLE_PRIMARY
                    }
                    DomainAccountPartyRole::Member => {
                        account_wire::AccountPartyRole::ACCOUNT_PARTY_ROLE_MEMBER
                    }
                };
                account_wire::AccountPartyAssociation {
                    party_id: association.party().as_str().to_owned(),
                    role: role.into(),
                    ..Default::default()
                }
            })
            .collect();
        Ok(account_wire::Account {
            account_id: account.account_id().as_str().to_owned(),
            name: account.name().to_owned(),
            status: status.into(),
            parties,
            version: account.version(),
            ..Default::default()
        })
    }
    fn receipt(
        &self,
        outcome: &accounts::MutationOutcome,
    ) -> std::result::Result<core_v2::MutationReceipt, ConnectError> {
        Ok(core_v2::MutationReceipt {
            request_id: self.request_id.clone(),
            resource_version: outcome.account.version(),
            replayed: outcome.replayed,
            original_request_id: outcome.original_request_id.clone(),
            ..Default::default()
        })
    }
}

// Connect wire for ORIGINAL crm-contact-points (no browser verification claim).
struct ContactPointV2Service {
    env: Arc<Env>,
    actor: access::Actor,
    request_id: String,
}

impl OwnedService for ContactPointV2Service {
    const OWNER: &'static str = crm_contact_points::MODULE_ID;
}
impl ContactPointV2Service {
    fn d1(&self) -> std::result::Result<D1Database, ConnectError> {
        self.env
            .d1("DB")
            .map_err(|_| ConnectError::internal("D1 unavailable"))
    }
    fn valid_id(id: &str) -> std::result::Result<String, ConnectError> {
        if id != id.trim() || id.contains('/') {
            return Err(ConnectError::invalid_argument("CONTACT_POINTS_ID_INVALID"));
        }
        let value = crm_contact_points::ContactPointId::try_new(id)
            .map_err(|_| ConnectError::invalid_argument("CONTACT_POINTS_ID_INVALID"))?;
        Ok(value.as_str().to_owned())
    }
    fn nanos(ts: &Timestamp) -> std::result::Result<i64, ConnectError> {
        crm_core_contracts::UtcInstant::from_proto_parts(ts.seconds, ts.nanos)
            .map(crm_core_contracts::UtcInstant::unix_nanos)
            .map_err(|error| ConnectError::invalid_argument(error.message))
    }
    fn timestamp(nanos: i64) -> Timestamp {
        let (seconds, nanos) = crm_core_contracts::UtcInstant::from_unix_nanos(nanos).proto_parts();
        Timestamp {
            seconds,
            nanos,
            ..Default::default()
        }
    }
    fn time_patch(
        patch: Option<&core_v2::TimestampPatch>,
    ) -> std::result::Result<contact_points::TimestampChange, ConnectError> {
        use contact_points::TimestampChange;
        let Some(patch) = patch else {
            return Ok(TimestampChange::Keep);
        };
        match patch.operation.as_ref() {
            Some(core_v2::timestamp_patch::Operation::Set(ts)) => {
                Self::nanos(ts).map(TimestampChange::Set)
            }
            Some(core_v2::timestamp_patch::Operation::Clear(true)) => Ok(TimestampChange::Clear),
            _ => Err(ConnectError::invalid_argument("invalid timestamp patch")),
        }
    }
    fn convert(
        point: &DomainContactPoint,
    ) -> std::result::Result<contact_wire::ContactPoint, ConnectError> {
        let kind = match point.kind() {
            DomainContactPointKind::Email => {
                contact_wire::ContactPointKind::CONTACT_POINT_KIND_EMAIL
            }
            DomainContactPointKind::Phone => {
                contact_wire::ContactPointKind::CONTACT_POINT_KIND_PHONE
            }
            DomainContactPointKind::Postal => {
                contact_wire::ContactPointKind::CONTACT_POINT_KIND_POSTAL
            }
            DomainContactPointKind::Web => contact_wire::ContactPointKind::CONTACT_POINT_KIND_WEB,
            DomainContactPointKind::Messaging => {
                contact_wire::ContactPointKind::CONTACT_POINT_KIND_MESSAGING
            }
        };
        let status = match point.status() {
            DomainContactPointStatus::Active => {
                contact_wire::ContactPointStatus::CONTACT_POINT_STATUS_ACTIVE
            }
            DomainContactPointStatus::Inactive => {
                contact_wire::ContactPointStatus::CONTACT_POINT_STATUS_INACTIVE
            }
        };
        Ok(contact_wire::ContactPoint {
            contact_point_id: point.contact_point_id().as_str().to_owned(),
            party_id: point.party_ref().as_str().to_owned(),
            kind: kind.into(),
            display_value: point.display_value().to_owned(),
            normalized_value: point.normalized_value().to_owned(),
            preferred: point.preferred(),
            status: status.into(),
            verified: point.verification().is_verified(),
            valid_from: point.valid_from_unix_nanos().map(Self::timestamp).into(),
            valid_until: point.valid_until_unix_nanos().map(Self::timestamp).into(),
            version: point.version(),
            ..Default::default()
        })
    }
    fn receipt(
        &self,
        outcome: &contact_points::MutationOutcome,
    ) -> std::result::Result<core_v2::MutationReceipt, ConnectError> {
        Ok(core_v2::MutationReceipt {
            request_id: self.request_id.clone(),
            resource_version: outcome.contact_point.version(),
            replayed: outcome.replayed,
            original_request_id: outcome.original_request_id.clone(),
            ..Default::default()
        })
    }
}

// Generated Connect wire for ORIGINAL crm-party-relationships. No second
// persistence path: party_relationships.rs owns D1, domain invariants and
// receipts; this service only checks wire fields and maps typed commands.
struct RelationshipV2Service {
    env: Arc<Env>,
    tenant: String,
    actor: String,
    request_id: String,
}

impl OwnedService for RelationshipV2Service {
    const OWNER: &'static str = crm_party_relationships::MODULE_ID;
}
impl RelationshipV2Service {
    fn d1(&self) -> std::result::Result<D1Database, ConnectError> {
        self.env
            .d1("DB")
            .map_err(|_| ConnectError::internal("D1 unavailable"))
    }
    fn valid_id(raw: &str) -> std::result::Result<String, ConnectError> {
        if raw != raw.trim() || raw.contains('/') {
            return Err(ConnectError::invalid_argument(
                "PARTY_RELATIONSHIPS_RELATIONSHIP_ID_INVALID",
            ));
        }
        crm_party_relationships::PartyRelationshipId::try_new(raw.to_owned())
            .map(|x| x.as_str().to_owned())
            .map_err(|_| {
                ConnectError::invalid_argument("PARTY_RELATIONSHIPS_RELATIONSHIP_ID_INVALID")
            })
    }
    fn patch(
        patch: Option<&core_v2::TimestampPatch>,
    ) -> std::result::Result<party_relationships::TimestampChange, ConnectError> {
        use party_relationships::TimestampChange;
        let Some(patch) = patch else {
            return Ok(TimestampChange::Keep);
        };
        match patch.operation.as_ref() {
            Some(core_v2::timestamp_patch::Operation::Set(ts)) => {
                ContactPointV2Service::nanos(ts).map(TimestampChange::Set)
            }
            Some(core_v2::timestamp_patch::Operation::Clear(true)) => Ok(TimestampChange::Clear),
            _ => Err(ConnectError::invalid_argument("invalid timestamp patch")),
        }
    }
    fn convert(
        relation: &DomainRelationship,
    ) -> std::result::Result<rel_wire::PartyRelationship, ConnectError> {
        let rtype = relation.relationship_type();
        let directionality = match rtype.directionality() {
            DomainDirectionality::Directional => {
                rel_wire::RelationshipDirectionality::RELATIONSHIP_DIRECTIONALITY_DIRECTIONAL
            }
            DomainDirectionality::Reciprocal => {
                rel_wire::RelationshipDirectionality::RELATIONSHIP_DIRECTIONALITY_RECIPROCAL
            }
        };
        let status = match relation.status() {
            DomainRelationshipStatus::Active => {
                rel_wire::RelationshipStatus::RELATIONSHIP_STATUS_ACTIVE
            }
            DomainRelationshipStatus::Inactive => {
                rel_wire::RelationshipStatus::RELATIONSHIP_STATUS_INACTIVE
            }
        };
        Ok(rel_wire::PartyRelationship {
            relationship_id: relation.party_relationship_id().as_str().to_owned(),
            from_party_id: relation.from_party_ref().as_str().to_owned(),
            to_party_id: relation.to_party_ref().as_str().to_owned(),
            r#type: Some(rel_wire::RelationshipType {
                code: rtype.code().to_owned(),
                directionality: directionality.into(),
                from_role: rtype.from_role().to_owned(),
                to_role: rtype.to_role().to_owned(),
                ..Default::default()
            })
            .into(),
            status: status.into(),
            valid_from: relation
                .valid_from_unix_nanos()
                .map(ContactPointV2Service::timestamp)
                .into(),
            valid_until: relation
                .valid_until_unix_nanos()
                .map(ContactPointV2Service::timestamp)
                .into(),
            version: relation.version(),
            ..Default::default()
        })
    }
    fn receipt(&self, out: &party_relationships::MutationOutcome) -> core_v2::MutationReceipt {
        core_v2::MutationReceipt {
            request_id: self.request_id.clone(),
            resource_version: out.relationship.version(),
            replayed: out.replayed,
            original_request_id: out.original_request_id.clone(),
            ..Default::default()
        }
    }
}

// Original Access owns signed membership. Neither UI nor any business owner
// can create an authenticated user; only the signed Cloudflare Access claim can.
struct MemberV2Service {
    env: Arc<Env>,
    actor: access::Actor,
    request_id: String,
}
impl OwnedService for MemberV2Service {
    const OWNER: &'static str = "crm.access";
}
impl MemberV2Service {
    fn d1(&self) -> std::result::Result<D1Database, ConnectError> {
        self.env
            .d1("DB")
            .map_err(|_| ConnectError::internal("D1 unavailable"))
    }
    fn role(
        value: buffa::EnumValue<member_wire::MemberRole>,
    ) -> std::result::Result<&'static str, ConnectError> {
        if value == member_wire::MemberRole::MEMBER_ROLE_ADMIN {
            Ok("admin")
        } else if value == member_wire::MemberRole::MEMBER_ROLE_VIEWER {
            Ok("viewer")
        } else {
            Err(ConnectError::invalid_argument("ACCESS_MEMBER_ROLE_INVALID"))
        }
    }
    fn wire(
        value: &access_members::Member,
    ) -> std::result::Result<member_wire::Member, ConnectError> {
        let role = match value.role.as_str() {
            "admin" => member_wire::MemberRole::MEMBER_ROLE_ADMIN,
            "viewer" => member_wire::MemberRole::MEMBER_ROLE_VIEWER,
            _ => return Err(ConnectError::internal("Access member has invalid role")),
        };
        Ok(member_wire::Member {
            email: value.email.clone(),
            role: role.into(),
            active: value.active == 1,
            version: value.version,
            ..Default::default()
        })
    }
    fn receipt(&self, saved: &access_members::Saved) -> core_v2::MutationReceipt {
        core_v2::MutationReceipt {
            request_id: self.request_id.clone(),
            resource_version: saved.member.version,
            replayed: saved.replayed,
            original_request_id: Some(saved.original_request_id.clone()),
            ..Default::default()
        }
    }
}

// Native generated Connect service for the ORIGINAL crm-activities Task.
// Same Task codec, D1 table, CAS and receipts as the blocked historical HTTP.
struct TaskV2Service {
    env: Arc<Env>,
    actor: access::Actor,
    request_id: String,
}

impl OwnedService for TaskV2Service {
    const OWNER: &'static str = crm_activities::MODULE_ID;
}
impl TaskV2Service {
    async fn persisted_account(&self, id: &str) -> std::result::Result<String, ConnectError> {
        activity_tasks::account_id(&self.d1()?, &self.actor.tenant, id)
            .await
            .map_err(|_| ConnectError::internal("Task Account read failed"))?
            .ok_or_else(|| ConnectError::internal("Task Account missing"))
    }
    fn d1(&self) -> std::result::Result<D1Database, ConnectError> {
        self.env
            .d1("DB")
            .map_err(|_| ConnectError::internal("D1 unavailable"))
    }
    fn valid_id(raw: &str) -> std::result::Result<String, ConnectError> {
        if raw != raw.trim() || raw.contains('/') {
            return Err(ConnectError::invalid_argument("ACTIVITIES_TASK_ID_INVALID"));
        }
        crm_module_sdk::RecordId::try_new(raw.to_owned())
            .map(|x| x.as_str().to_owned())
            .map_err(|_| ConnectError::invalid_argument("ACTIVITIES_TASK_ID_INVALID"))
    }
    fn convert(
        task: &DomainTask,
        account_id: &str,
    ) -> std::result::Result<task_wire::Task, ConnectError> {
        let priority = match task.priority() {
            DomainTaskPriority::Low => task_wire::TaskPriority::TASK_PRIORITY_LOW,
            DomainTaskPriority::Normal => task_wire::TaskPriority::TASK_PRIORITY_NORMAL,
            DomainTaskPriority::High => task_wire::TaskPriority::TASK_PRIORITY_HIGH,
            DomainTaskPriority::Urgent => task_wire::TaskPriority::TASK_PRIORITY_URGENT,
        };
        let status = match task.status() {
            DomainTaskStatus::Open => task_wire::TaskStatus::TASK_STATUS_OPEN,
            DomainTaskStatus::Completed => task_wire::TaskStatus::TASK_STATUS_COMPLETED,
        };
        Ok(task_wire::Task {
            task_id: task.task_id().as_str().to_owned(),
            subject: task.subject().to_owned(),
            description: task.description().unwrap_or_default().to_owned(),
            account_id: account_id.to_owned(),
            priority: priority.into(),
            status: status.into(),
            due_at: task
                .due_at_unix_nanos()
                .map(ContactPointV2Service::timestamp)
                .into(),
            reminder_at: task
                .reminder_at_unix_nanos()
                .map(ContactPointV2Service::timestamp)
                .into(),
            completed_at: task
                .completed_at_unix_nanos()
                .map(ContactPointV2Service::timestamp)
                .into(),
            version: task.version(),
            related_deal_id: task
                .related_resources()
                .iter()
                .find(|relation| relation.resource_type == "sales.deal")
                .map(|relation| relation.resource_id.clone())
                .unwrap_or_default(),
            owner: Some(wire_owner(match task.owner() {
                crm_activities::TaskOwner::Actor(value) => {
                    core_v2::actor_or_team_owner::Owner::AssigneeIdentity(value.to_string())
                }
                crm_activities::TaskOwner::Team(value) => {
                    core_v2::actor_or_team_owner::Owner::TeamId(value.as_str().to_owned())
                }
            }))
            .into(),
            assignee_identity: match task.owner() {
                crm_activities::TaskOwner::Actor(value) => value.to_string(),
                crm_activities::TaskOwner::Team(_) => String::new(),
            },
            owner_team_id: match task.owner() {
                crm_activities::TaskOwner::Team(value) => value.as_str().to_owned(),
                crm_activities::TaskOwner::Actor(_) => String::new(),
            },
            ..Default::default()
        })
    }
    fn receipt(&self, out: &activity_tasks::MutationOutcome) -> core_v2::MutationReceipt {
        core_v2::MutationReceipt {
            request_id: self.request_id.clone(),
            resource_version: out.task.version(),
            replayed: out.replayed,
            original_request_id: out.original_request_id.clone(),
            ..Default::default()
        }
    }
}

// Original read-only crm-data-quality: generated Protobuf, original rule set,
// tenant-scoped Party read. No shadow rule engine or persisted quality score.
struct DataQualityV2Service {
    env: Arc<Env>,
    tenant: String,
}

impl OwnedService for DataQualityV2Service {
    const OWNER: &'static str = crm_data_quality::MODULE_ID;
}

// Original crm-identity-resolution: candidate reads and reversible canonical
// merge lineage. Only this domain owner validates topology and persisted state.
struct IdentityV2Service {
    env: Arc<Env>,
    actor: access::Actor,
    request_id: String,
}

impl OwnedService for IdentityV2Service {
    const OWNER: &'static str = crm_identity_resolution::MODULE_ID;
}

// The original crm-consents domain owns communication decisions and operator
// suppression. This generated Connect boundary never accepts evidence/GRANT.
struct ConsentV2Service {
    env: Arc<Env>,
    actor: access::Actor,
    request_id: String,
}

impl OwnedService for ConsentV2Service {
    const OWNER: &'static str = crm_consents::MODULE_ID;
}
impl ConsentV2Service {
    fn d1(&self) -> std::result::Result<D1Database, ConnectError> {
        self.env
            .d1("DB")
            .map_err(|_| ConnectError::internal("D1 unavailable"))
    }
    fn channel(
        raw: buffa::EnumValue<consent_wire::CommunicationChannel>,
    ) -> std::result::Result<consents::ChannelInput, ConnectError> {
        match raw {
            x if x == consent_wire::CommunicationChannel::COMMUNICATION_CHANNEL_EMAIL => {
                Ok(consents::ChannelInput::Email)
            }
            x if x == consent_wire::CommunicationChannel::COMMUNICATION_CHANNEL_PHONE => {
                Ok(consents::ChannelInput::Phone)
            }
            x if x == consent_wire::CommunicationChannel::COMMUNICATION_CHANNEL_SMS => {
                Ok(consents::ChannelInput::Sms)
            }
            x if x == consent_wire::CommunicationChannel::COMMUNICATION_CHANNEL_POSTAL => {
                Ok(consents::ChannelInput::Postal)
            }
            x if x == consent_wire::CommunicationChannel::COMMUNICATION_CHANNEL_MESSAGING => {
                Ok(consents::ChannelInput::Messaging)
            }
            x if x == consent_wire::CommunicationChannel::COMMUNICATION_CHANNEL_PUSH => {
                Ok(consents::ChannelInput::Push)
            }
            _ => Err(ConnectError::invalid_argument(
                "invalid CommunicationChannel",
            )),
        }
    }
    fn convert(item: &DomainConsent) -> consent_wire::ConsentAuthorization {
        let channel = match item.channel() {
            DomainConsentChannel::Email => {
                consent_wire::CommunicationChannel::COMMUNICATION_CHANNEL_EMAIL
            }
            DomainConsentChannel::Phone => {
                consent_wire::CommunicationChannel::COMMUNICATION_CHANNEL_PHONE
            }
            DomainConsentChannel::Sms => {
                consent_wire::CommunicationChannel::COMMUNICATION_CHANNEL_SMS
            }
            DomainConsentChannel::Postal => {
                consent_wire::CommunicationChannel::COMMUNICATION_CHANNEL_POSTAL
            }
            DomainConsentChannel::Messaging => {
                consent_wire::CommunicationChannel::COMMUNICATION_CHANNEL_MESSAGING
            }
            DomainConsentChannel::Push => {
                consent_wire::CommunicationChannel::COMMUNICATION_CHANNEL_PUSH
            }
        };
        let effect = match item.effect() {
            DomainConsentEffect::Grant => consent_wire::ConsentEffect::CONSENT_EFFECT_GRANT,
            DomainConsentEffect::Deny => consent_wire::ConsentEffect::CONSENT_EFFECT_DENY,
        };
        let status = match item.status() {
            DomainConsentStatus::Active => consent_wire::ConsentStatus::CONSENT_STATUS_ACTIVE,
            DomainConsentStatus::Withdrawn => consent_wire::ConsentStatus::CONSENT_STATUS_WITHDRAWN,
        };
        consent_wire::ConsentAuthorization {
            authorization_id: item.authorization_id().as_str().to_owned(),
            party_id: item.party_ref().as_str().to_owned(),
            contact_point_id: item.contact_point_ref().map(|x| x.as_str().to_owned()),
            purpose: item.purpose().as_str().to_owned(),
            channel: channel.into(),
            effect: effect.into(),
            status: status.into(),
            jurisdiction: item.jurisdiction().as_str().to_owned(),
            version: item.version(),
            ..Default::default()
        }
    }
}

// Sales v2 wraps the EXISTING crm-sales aggregate, D1 persistence and atomic
// sales->activities delivery. Neither generated wire nor React implements
// Deal transitions, price rounding, CAS or new business policy.
struct SalesV2Service {
    env: Arc<Env>,
    tenant: String,
    actor: String,
    admin: bool,
    request_id: String,
}

impl OwnedService for SalesV2Service {
    const OWNER: &'static str = crm_sales::MODULE_ID;
}
impl SalesV2Service {
    fn d1(&self) -> std::result::Result<D1Database, ConnectError> {
        self.env
            .d1("DB")
            .map_err(|_| ConnectError::internal("D1 unavailable"))
    }
    fn id(raw: &str) -> std::result::Result<String, ConnectError> {
        if raw != raw.trim() || raw.contains('/') {
            return Err(ConnectError::invalid_argument("SALES_DEAL_ID_INVALID"));
        }
        crm_module_sdk::RecordId::try_new(raw.to_owned())
            .map(|x| x.as_str().to_owned())
            .map_err(|_| ConnectError::invalid_argument("SALES_DEAL_ID_INVALID"))
    }
    // The incoming rank is ignored: the domain catalog owns it on writes.
    fn stage(wire: &sale_wire::DealStage) -> (String, String) {
        (wire.pipeline_id.clone(), wire.stage_id.clone())
    }
    fn deal_status(
        raw: buffa::EnumValue<sale_wire::DealStatus>,
    ) -> std::result::Result<sales_deals::StatusInput, ConnectError> {
        match raw {
            x if x == sale_wire::DealStatus::DEAL_STATUS_OPEN => Ok(sales_deals::StatusInput::Open),
            x if x == sale_wire::DealStatus::DEAL_STATUS_WON => Ok(sales_deals::StatusInput::Won),
            x if x == sale_wire::DealStatus::DEAL_STATUS_LOST => Ok(sales_deals::StatusInput::Lost),
            _ => Err(ConnectError::invalid_argument("invalid DealStatus")),
        }
    }
    fn forecast_category(
        raw: Option<buffa::EnumValue<sale_wire::ForecastCategory>>,
    ) -> std::result::Result<Option<crm_sales::ForecastCategory>, ConnectError> {
        use crm_sales::ForecastCategory as Domain;
        use sale_wire::ForecastCategory as Wire;
        Ok(match raw {
            None => None,
            Some(x) if x == Wire::FORECAST_CATEGORY_PIPELINE => Some(Domain::Pipeline),
            Some(x) if x == Wire::FORECAST_CATEGORY_BEST_CASE => Some(Domain::BestCase),
            Some(x) if x == Wire::FORECAST_CATEGORY_COMMIT => Some(Domain::Commit),
            Some(x) if x == Wire::FORECAST_CATEGORY_OMITTED => Some(Domain::Omitted),
            _ => return Err(ConnectError::invalid_argument("invalid ForecastCategory")),
        })
    }

    fn forecast_wire(value: crm_sales::ForecastCategory) -> sale_wire::ForecastCategory {
        use crm_sales::ForecastCategory as Domain;
        use sale_wire::ForecastCategory as Wire;
        match value {
            Domain::Pipeline => Wire::FORECAST_CATEGORY_PIPELINE,
            Domain::BestCase => Wire::FORECAST_CATEGORY_BEST_CASE,
            Domain::Commit => Wire::FORECAST_CATEGORY_COMMIT,
            Domain::Omitted => Wire::FORECAST_CATEGORY_OMITTED,
        }
    }

    fn owner_scope(
        value: Option<&core_v2::ActorOrTeamOwner>,
    ) -> std::result::Result<Option<crm_sales::SalesOwnerScope>, ConnectError> {
        use core_v2::actor_or_team_owner::Owner;
        let Some(value) = value else {
            return Ok(None);
        };
        let selection = match value.owner.as_ref() {
            Some(Owner::AssigneeIdentity(id)) => crm_sales::SalesOwnerScope::Actor(id.clone()),
            Some(Owner::TeamId(id)) => crm_sales::SalesOwnerScope::Team(id.clone()),
            None => return Err(ConnectError::invalid_argument("SALES_OWNER_REQUIRED")),
        };
        selection
            .validate()
            .map_err(|e| ConnectError::invalid_argument(e.code))?;
        Ok(Some(selection))
    }
    fn owner_wire(owner: &crm_sales::SalesOwnerScope) -> core_v2::ActorOrTeamOwner {
        use core_v2::actor_or_team_owner::Owner;
        wire_owner(match owner {
            crm_sales::SalesOwnerScope::Actor(id) => Owner::AssigneeIdentity(id.clone()),
            crm_sales::SalesOwnerScope::Team(id) => Owner::TeamId(id.clone()),
        })
    }
    fn dispatch_wire(value: crm_sales::SalesDispatchMode) -> sale_wire::SalesDispatchMode {
        match value {
            crm_sales::SalesDispatchMode::Direct => {
                sale_wire::SalesDispatchMode::SALES_DISPATCH_MODE_DIRECT
            }
            crm_sales::SalesDispatchMode::LeastOpenDeals => {
                sale_wire::SalesDispatchMode::SALES_DISPATCH_MODE_LEAST_OPEN_DEALS
            }
        }
    }
    fn routing_draft(
        id: String,
        cfg: &sale_wire::SalesRoutingRuleConfiguration,
    ) -> std::result::Result<sales_territories::Record, ConnectError> {
        let criteria = cfg
            .criteria
            .as_option()
            .ok_or_else(|| ConnectError::invalid_argument("SALES_ROUTING_CRITERIA_REQUIRED"))?;
        let dispatch = if cfg.dispatch == sale_wire::SalesDispatchMode::SALES_DISPATCH_MODE_DIRECT {
            crm_sales::SalesDispatchMode::Direct
        } else if cfg.dispatch == sale_wire::SalesDispatchMode::SALES_DISPATCH_MODE_LEAST_OPEN_DEALS
        {
            crm_sales::SalesDispatchMode::LeastOpenDeals
        } else {
            return Err(ConnectError::invalid_argument(
                "SALES_ROUTING_DISPATCH_INVALID",
            ));
        };
        let overflow =
            if cfg.overflow == sale_wire::SalesCapacityOverflow::SALES_CAPACITY_OVERFLOW_REJECT {
                crm_sales::SalesCapacityOverflow::Reject
            } else if cfg.overflow
                == sale_wire::SalesCapacityOverflow::SALES_CAPACITY_OVERFLOW_TEAM_QUEUE
            {
                crm_sales::SalesCapacityOverflow::TeamQueue
            } else {
                return Err(ConnectError::invalid_argument(
                    "SALES_ROUTING_OVERFLOW_INVALID",
                ));
            };
        let target = Self::owner_scope(cfg.target.as_option())?
            .ok_or_else(|| ConnectError::invalid_argument("SALES_ROUTING_TARGET_REQUIRED"))?;
        Ok(sales_territories::Record::RoutingRule(
            crm_sales::SalesRoutingRule {
                rule_id: id,
                name: cfg.name.clone(),
                priority: cfg.priority,
                criteria: crm_sales::SalesRoutingCriteria {
                    territory_id: criteria.territory_id.clone(),
                    pipeline_id: criteria.pipeline_id.clone(),
                    stage_id: criteria.stage_id.clone(),
                    currency_code: criteria.currency_code.clone(),
                    minimum_minor_units: criteria.minimum_minor_units,
                    maximum_minor_units: criteria.maximum_minor_units,
                },
                target,
                dispatch,
                maximum_open_deals: cfg.maximum_open_deals,
                overflow,
                archived: cfg.archived,
                version: 1,
            },
        ))
    }
    fn routing_rule_wire(
        record: sales_territories::Record,
    ) -> std::result::Result<sale_wire::SalesRoutingRule, ConnectError> {
        let sales_territories::Record::RoutingRule(rule) = record else {
            return Err(ConnectError::internal("SALES_ROUTING_RULE_STATE_INVALID"));
        };
        let criteria = rule.criteria;
        Ok(sale_wire::SalesRoutingRule {
            rule_id: rule.rule_id,
            version: rule.version,
            configuration: Some(sale_wire::SalesRoutingRuleConfiguration {
                name: rule.name,
                priority: rule.priority,
                criteria: Some(sale_wire::SalesRoutingCriteria {
                    territory_id: criteria.territory_id,
                    pipeline_id: criteria.pipeline_id,
                    stage_id: criteria.stage_id,
                    currency_code: criteria.currency_code,
                    minimum_minor_units: criteria.minimum_minor_units,
                    maximum_minor_units: criteria.maximum_minor_units,
                    ..Default::default()
                })
                .into(),
                target: Some(Self::owner_wire(&rule.target)).into(),
                dispatch: Self::dispatch_wire(rule.dispatch).into(),
                maximum_open_deals: rule.maximum_open_deals,
                overflow: match rule.overflow {
                    crm_sales::SalesCapacityOverflow::Reject => {
                        sale_wire::SalesCapacityOverflow::SALES_CAPACITY_OVERFLOW_REJECT
                    }
                    crm_sales::SalesCapacityOverflow::TeamQueue => {
                        sale_wire::SalesCapacityOverflow::SALES_CAPACITY_OVERFLOW_TEAM_QUEUE
                    }
                }
                .into(),
                archived: rule.archived,
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }
    fn routing_decision_wire(
        value: &crm_sales::SalesRoutingDecision,
    ) -> sale_wire::SalesRoutingDecision {
        use crm_sales::SalesRoutingSource as Source;
        use sale_wire::SalesRoutingSource as Wire;
        sale_wire::SalesRoutingDecision {
            source: match value.source {
                Source::Rule => Wire::SALES_ROUTING_SOURCE_RULE,
                Source::Account => Wire::SALES_ROUTING_SOURCE_ACCOUNT,
                Source::Territory => Wire::SALES_ROUTING_SOURCE_TERRITORY,
                Source::Manual => Wire::SALES_ROUTING_SOURCE_MANUAL,
                Source::Creator => Wire::SALES_ROUTING_SOURCE_CREATOR,
            }
            .into(),
            selected_owner: Some(Self::owner_wire(&value.selected_owner)).into(),
            target: Some(Self::owner_wire(&value.target)).into(),
            rule_id: value.rule_id.clone(),
            rule_version: value.rule_version,
            rule_name: value.rule_name.clone(),
            territory_id: value.territory_id.clone(),
            territory_version: value.territory_version,
            territory_name: value.territory_name.clone(),
            assignment_version: value.assignment_version,
            dispatch: Self::dispatch_wire(value.dispatch).into(),
            selected_open_deals: value.selected_open_deals,
            maximum_open_deals: value.maximum_open_deals,
            capacity_overflow: value.capacity_overflow,
            ..Default::default()
        }
    }

    fn quota_draft(
        id: String,
        cfg: &sale_wire::SalesQuotaConfiguration,
    ) -> std::result::Result<sales_territories::Record, ConnectError> {
        use crm_sales::{QuotaCadence, QuotaDate, QuotaPeriod, SalesQuotaScope};
        use sale_wire::sales_quota_scope::Scope;
        let scope = match cfg.scope.as_option().and_then(|value| value.scope.as_ref()) {
            Some(Scope::TenantWide(true)) => SalesQuotaScope::Tenant,
            Some(Scope::Owner(owner)) => SalesQuotaScope::Owner(
                Self::owner_scope(Some(owner))?
                    .ok_or_else(|| ConnectError::invalid_argument("SALES_QUOTA_SCOPE_REQUIRED"))?,
            ),
            Some(Scope::TerritoryId(id)) => SalesQuotaScope::Territory(Self::id(id)?),
            _ => return Err(ConnectError::invalid_argument("SALES_QUOTA_SCOPE_REQUIRED")),
        };
        let period = cfg
            .period
            .as_option()
            .ok_or_else(|| ConnectError::invalid_argument("SALES_QUOTA_PERIOD_REQUIRED"))?;
        let date = |value: Option<&core_v2::CalendarDate>| {
            let value =
                value.ok_or_else(|| ConnectError::invalid_argument("SALES_QUOTA_DATE_REQUIRED"))?;
            Ok::<_, ConnectError>(QuotaDate {
                year: value.year,
                month: u8::try_from(value.month)
                    .map_err(|_| ConnectError::invalid_argument("SALES_QUOTA_DATE_INVALID"))?,
                day: u8::try_from(value.day)
                    .map_err(|_| ConnectError::invalid_argument("SALES_QUOTA_DATE_INVALID"))?,
            })
        };
        let cadence = if period.cadence == sale_wire::SalesQuotaCadence::SALES_QUOTA_CADENCE_MONTHLY
        {
            QuotaCadence::Monthly
        } else if period.cadence == sale_wire::SalesQuotaCadence::SALES_QUOTA_CADENCE_QUARTERLY {
            QuotaCadence::Quarterly
        } else if period.cadence == sale_wire::SalesQuotaCadence::SALES_QUOTA_CADENCE_CUSTOM {
            QuotaCadence::Custom
        } else {
            return Err(ConnectError::invalid_argument(
                "SALES_QUOTA_CADENCE_INVALID",
            ));
        };
        let period = QuotaPeriod::new(
            date(period.start_date.as_option())?,
            date(period.end_date.as_option())?,
            period.time_zone.clone(),
            cadence,
        )
        .map_err(|error| ConnectError::invalid_argument(error.code))?;
        let target = cfg
            .target
            .as_option()
            .ok_or_else(|| ConnectError::invalid_argument("SALES_QUOTA_TARGET_REQUIRED"))?;
        Ok(sales_territories::Record::Quota(crm_sales::SalesQuota {
            quota_id: id,
            name: cfg.name.clone(),
            scope,
            pipeline_id: cfg.pipeline_id.clone(),
            period,
            currency_code: target.currency_code.clone(),
            target_minor_units: target.minor_units.to_string(),
            published: cfg.published,
            archived: cfg.archived,
            revision_reason: cfg.revision_reason.clone(),
            revision_at_unix_nanos: "0".into(),
            version: 1,
        }))
    }

    fn quota_wire(
        record: sales_territories::Record,
    ) -> std::result::Result<sale_wire::SalesQuota, ConnectError> {
        use crm_sales::{QuotaCadence, SalesQuotaScope};
        use sale_wire::sales_quota_scope::Scope;
        let sales_territories::Record::Quota(quota) = record else {
            return Err(ConnectError::internal("SALES_QUOTA_STATE_INVALID"));
        };
        let scope = match &quota.scope {
            SalesQuotaScope::Tenant => Scope::TenantWide(true),
            SalesQuotaScope::Owner(owner) => Scope::Owner(Box::new(Self::owner_wire(owner))),
            SalesQuotaScope::Territory(id) => Scope::TerritoryId(id.clone()),
        };
        let date = |value: crm_sales::QuotaDate| core_v2::CalendarDate {
            year: value.year,
            month: u32::from(value.month),
            day: u32::from(value.day),
            ..Default::default()
        };
        let (start, end) = quota
            .period
            .bounds()
            .map_err(|_| ConnectError::internal("SALES_QUOTA_PERIOD_INVALID"))?;
        Ok(sale_wire::SalesQuota {
            quota_id: quota.quota_id,
            version: quota.version,
            start_unix_nanos: start,
            end_exclusive_unix_nanos: end,
            revision_at_unix_nanos: quota
                .revision_at_unix_nanos
                .parse()
                .map_err(|_| ConnectError::internal("SALES_QUOTA_REVISION_INVALID"))?,
            configuration: Some(sale_wire::SalesQuotaConfiguration {
                name: quota.name,
                scope: Some(sale_wire::SalesQuotaScope {
                    scope: Some(scope),
                    ..Default::default()
                })
                .into(),
                pipeline_id: quota.pipeline_id,
                period: Some(sale_wire::SalesQuotaPeriod {
                    start_date: Some(date(quota.period.start_date)).into(),
                    end_date: Some(date(quota.period.end_date)).into(),
                    time_zone: quota.period.time_zone,
                    cadence: match quota.period.cadence {
                        QuotaCadence::Monthly => {
                            sale_wire::SalesQuotaCadence::SALES_QUOTA_CADENCE_MONTHLY
                        }
                        QuotaCadence::Quarterly => {
                            sale_wire::SalesQuotaCadence::SALES_QUOTA_CADENCE_QUARTERLY
                        }
                        QuotaCadence::Custom => {
                            sale_wire::SalesQuotaCadence::SALES_QUOTA_CADENCE_CUSTOM
                        }
                    }
                    .into(),
                    ..Default::default()
                })
                .into(),
                target: Some(core_v2::ExactMoney {
                    minor_units: quota
                        .target_minor_units
                        .parse()
                        .map_err(|_| ConnectError::internal("SALES_QUOTA_TARGET_INVALID"))?,
                    currency_code: quota.currency_code,
                    ..Default::default()
                })
                .into(),
                published: quota.published,
                archived: quota.archived,
                revision_reason: quota.revision_reason,
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }

    fn commercial_fact_wire(
        value: &crm_sales::CommercialCloseFact,
    ) -> std::result::Result<sale_wire::CommercialCloseFact, ConnectError> {
        value
            .validate()
            .map_err(|_| ConnectError::internal("SALES_COMMERCIAL_FACT_INVALID"))?;
        let dimension = |value: &crm_sales::CommercialDimension| sale_wire::CommercialDimension {
            id: value.id.clone(),
            name: value.name.clone(),
            version: value.version,
            ..Default::default()
        };
        let amount = value
            .amount
            .as_ref()
            .map(|amount| {
                let minor_units = amount
                    .minor_units
                    .parse::<i64>()
                    .map_err(|_| ConnectError::internal("SALES_COMMERCIAL_AMOUNT_INVALID"))?;
                Ok::<_, ConnectError>(core_v2::ExactMoney {
                    minor_units,
                    currency_code: amount.currency_code.clone(),
                    ..Default::default()
                })
            })
            .transpose()?;
        Ok(sale_wire::CommercialCloseFact {
            close_version: value.close_version,
            outcome: match value.outcome {
                DomainDealStatus::Won => sale_wire::DealStatus::DEAL_STATUS_WON,
                DomainDealStatus::Lost => sale_wire::DealStatus::DEAL_STATUS_LOST,
                DomainDealStatus::Open => {
                    return Err(ConnectError::internal("SALES_COMMERCIAL_FACT_INVALID"));
                }
            }
            .into(),
            closed_at: Some(ContactPointV2Service::timestamp(value.closed_at_unix_nanos)).into(),
            owner: Some(Self::owner_wire(&value.owner)).into(),
            account_id: value.account_id.clone(),
            pipeline_id: value.pipeline_id.clone(),
            stage_id: value.stage_id.clone(),
            amount: amount.into(),
            credited_actor: value.credited_actor.clone(),
            credited_team: value.credited_team.as_ref().map(dimension).into(),
            credited_territory: value.credited_territory.as_ref().map(dimension).into(),
            assignment_version: value.assignment_version,
            catalog_version: value.catalog_version,
            ..Default::default()
        })
    }

    fn opportunity_participant_wire(
        value: &crm_sales::OpportunityParticipant,
    ) -> sale_wire::OpportunityParticipant {
        sale_wire::OpportunityParticipant {
            email: value.email.clone(),
            role: match value.role {
                crm_sales::OpportunityTeamRole::Observer => {
                    sale_wire::OpportunityTeamRole::OPPORTUNITY_TEAM_ROLE_OBSERVER
                }
                crm_sales::OpportunityTeamRole::Contributor => {
                    sale_wire::OpportunityTeamRole::OPPORTUNITY_TEAM_ROLE_CONTRIBUTOR
                }
                crm_sales::OpportunityTeamRole::Lead => {
                    sale_wire::OpportunityTeamRole::OPPORTUNITY_TEAM_ROLE_LEAD
                }
            }
            .into(),
            member_version: value.member_version,
            ..Default::default()
        }
    }

    fn convert(deal: &DomainDeal) -> std::result::Result<sale_wire::Deal, ConnectError> {
        let stage = deal.stage();
        let status = match deal.status() {
            DomainDealStatus::Open => sale_wire::DealStatus::DEAL_STATUS_OPEN,
            DomainDealStatus::Won => sale_wire::DealStatus::DEAL_STATUS_WON,
            DomainDealStatus::Lost => sale_wire::DealStatus::DEAL_STATUS_LOST,
        };
        let account = deal
            .account()
            .ok_or_else(|| ConnectError::internal("original Deal missing Account"))?;
        let amount = deal
            .amount()
            .map(|money| {
                let units = i64::try_from(money.minor_units())
                    .map_err(|_| ConnectError::internal("original money exceeds wire i64"))?;
                Ok::<core_v2::ExactMoney, ConnectError>(core_v2::ExactMoney {
                    minor_units: units,
                    currency_code: money.currency().as_str().to_owned(),
                    ..Default::default()
                })
            })
            .transpose()?;
        Ok(sale_wire::Deal {
            deal_id: deal.deal_id().as_str().to_owned(),
            opportunity_team: deal
                .opportunity_team()
                .iter()
                .map(Self::opportunity_participant_wire)
                .collect(),
            commercial_close_fact: deal
                .close_outcome()
                .and_then(|outcome| outcome.commercial_fact())
                .map(Self::commercial_fact_wire)
                .transpose()?
                .into(),
            initial_routing: deal
                .initial_routing()
                .map(Self::routing_decision_wire)
                .into(),
            owner: Some(Self::owner_wire(&match deal.owner() {
                crm_sales::DealOwner::Actor(id) => {
                    crm_sales::SalesOwnerScope::Actor(id.as_str().to_owned())
                }
                crm_sales::DealOwner::Team(id) => {
                    crm_sales::SalesOwnerScope::Team(id.as_str().to_owned())
                }
            }))
            .into(),
            name: deal.name().to_owned(),
            account_id: account.resource_id.clone(),
            stage: Some(sale_wire::DealStage {
                pipeline_id: stage.pipeline_id().as_str().to_owned(),
                stage_id: stage.stage_id().as_str().to_owned(),
                ordinal: u32::from(stage.ordinal()),
                ..Default::default()
            })
            .into(),
            status: status.into(),
            amount: amount.into(),
            probability_basis_points: u32::from(deal.probability().get()),
            probability_source: match deal.probability_source() {
                crm_sales::ProbabilitySource::Manual => {
                    sale_wire::ProbabilitySource::PROBABILITY_SOURCE_MANUAL
                }
                crm_sales::ProbabilitySource::StageDefault => {
                    sale_wire::ProbabilitySource::PROBABILITY_SOURCE_STAGE_DEFAULT
                }
            }
            .into(),
            forecast_category: Self::forecast_wire(deal.forecast_category()).into(),
            version: deal.version(),
            closed_at: deal
                .close_outcome()
                .map(|outcome| ContactPointV2Service::timestamp(outcome.closed_at_unix_nanos()))
                .into(),
            close_reason_code: deal
                .close_outcome()
                .map_or(String::new(), |x| x.reason_code().as_str().to_owned()),
            primary_contact_id: deal
                .primary_contact()
                .map_or(String::new(), |x| x.resource_id.clone()),
            expected_close_date: deal
                .expected_close_date()
                .map(|d| core_v2::CalendarDate {
                    year: d.year(),
                    month: u32::from(d.month()),
                    day: u32::from(d.day()),
                    ..Default::default()
                })
                .into(),
            created_at: Some(ContactPointV2Service::timestamp(
                deal.created_at_unix_nanos(),
            ))
            .into(),
            updated_at: Some(ContactPointV2Service::timestamp(
                deal.updated_at_unix_nanos(),
            ))
            .into(),
            ..Default::default()
        })
    }
    fn pipeline_id(raw: &str) -> std::result::Result<(), ConnectError> {
        if raw != raw.trim() || raw.contains('/') {
            return Err(ConnectError::invalid_argument("SALES_PIPELINE_ID_INVALID"));
        }
        crm_sales::PipelineId::try_new(raw.to_owned())
            .map_err(|_| ConnectError::invalid_argument("SALES_PIPELINE_ID_INVALID"))?;
        Ok(())
    }
    fn stage_inputs(
        input: &[sale_wire::PipelineStageInput],
    ) -> std::result::Result<Vec<crm_sales::PipelineStageInput>, ConnectError> {
        input
            .iter()
            .map(|stage| {
                let probability = u16::try_from(stage.probability_basis_points).map_err(|_| {
                    ConnectError::invalid_argument("SALES_PIPELINE_PROBABILITY_INVALID")
                })?;
                Ok(crm_sales::PipelineStageInput {
                    stage_id: stage.stage_id.clone(),
                    name: stage.name.clone(),
                    probability_basis_points: probability,
                    archived: stage.archived,
                })
            })
            .collect()
    }
    fn pipeline(pipeline: &crm_sales::SalesPipeline) -> sale_wire::SalesPipeline {
        sale_wire::SalesPipeline {
            pipeline_id: pipeline.pipeline_id.clone(),
            name: pipeline.name.clone(),
            version: pipeline.version,
            archived: pipeline.archived,
            stages: pipeline
                .stages
                .iter()
                .map(|stage| sale_wire::PipelineStage {
                    stage_id: stage.stage_id.clone(),
                    name: stage.name.clone(),
                    ordinal: u32::from(stage.ordinal),
                    probability_basis_points: u32::from(stage.probability_basis_points),
                    archived: stage.archived,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }
    }
    fn view_status(
        raw: Option<buffa::EnumValue<sale_wire::DealStatus>>,
    ) -> std::result::Result<Option<crm_sales::DealStatus>, ConnectError> {
        match raw {
            None => Ok(None),
            Some(value) if value == sale_wire::DealStatus::DEAL_STATUS_OPEN => {
                Ok(Some(crm_sales::DealStatus::Open))
            }
            Some(value) if value == sale_wire::DealStatus::DEAL_STATUS_WON => {
                Ok(Some(crm_sales::DealStatus::Won))
            }
            Some(value) if value == sale_wire::DealStatus::DEAL_STATUS_LOST => {
                Ok(Some(crm_sales::DealStatus::Lost))
            }
            _ => Err(ConnectError::invalid_argument("SALES_VIEW_STATUS_INVALID")),
        }
    }
    fn view(value: &crm_sales::SalesDealView) -> sale_wire::SalesDealView {
        let status = value.status.map(|status| {
            match status {
                crm_sales::DealStatus::Open => sale_wire::DealStatus::DEAL_STATUS_OPEN,
                crm_sales::DealStatus::Won => sale_wire::DealStatus::DEAL_STATUS_WON,
                crm_sales::DealStatus::Lost => sale_wire::DealStatus::DEAL_STATUS_LOST,
            }
            .into()
        });
        sale_wire::SalesDealView {
            view_id: value.view_id.clone(),
            owner_filter: value.owner_filter.as_ref().map(Self::owner_wire).into(),
            name: value.name.clone(),
            pipeline_id: value.pipeline_id.clone(),
            status,
            account_id: value.account_id.clone(),
            version: value.version,
            archived: value.archived,
            ..Default::default()
        }
    }
    fn view_receipt(&self, saved: &sales_views::Saved) -> core_v2::MutationReceipt {
        core_v2::MutationReceipt {
            request_id: self.request_id.clone(),
            resource_version: saved.value.version,
            replayed: saved.replayed,
            original_request_id: saved.original_request_id.clone(),
            ..Default::default()
        }
    }
    fn pipeline_receipt(&self, saved: &sales_pipelines::Saved) -> core_v2::MutationReceipt {
        core_v2::MutationReceipt {
            request_id: self.request_id.clone(),
            resource_version: saved.value.version,
            replayed: saved.replayed,
            original_request_id: saved.original_request_id.clone(),
            ..Default::default()
        }
    }
}

// The ORIGINAL crm-activities owner validates immutable Interaction events.
// Only its D1 adapter writes receipts/audit; this is a generated Connect trait.
struct InteractionV2Service {
    env: Arc<Env>,
    actor: access::Actor,
    request_id: String,
}
impl OwnedService for InteractionV2Service {
    const OWNER: &'static str = crm_activities::MODULE_ID;
}
impl InteractionV2Service {
    fn d1(&self) -> std::result::Result<D1Database, ConnectError> {
        self.env
            .d1("DB")
            .map_err(|_| ConnectError::internal("D1 unavailable"))
    }
    fn convert(item: &crm_activities::interactions::Interaction) -> interaction_wire::Interaction {
        let kind = match item.kind {
            crm_activities::interactions::InteractionKind::Call => {
                interaction_wire::InteractionKind::INTERACTION_KIND_CALL
            }
            crm_activities::interactions::InteractionKind::Meeting => {
                interaction_wire::InteractionKind::INTERACTION_KIND_MEETING
            }
            crm_activities::interactions::InteractionKind::Note => {
                interaction_wire::InteractionKind::INTERACTION_KIND_NOTE
            }
        };
        interaction_wire::Interaction {
            interaction_id: item.interaction_id.clone(),
            account_id: item.account_id.clone(),
            deal_id: item.deal_id.clone().unwrap_or_default(),
            kind: kind.into(),
            text: item.text.clone(),
            author: item.author.clone(),
            occurred_at: Some(ContactPointV2Service::timestamp(item.occurred_at_nanos)).into(),
            recorded_at: Some(ContactPointV2Service::timestamp(item.recorded_at_nanos)).into(),
            version: item.version,
            ..Default::default()
        }
    }
}

// One constructor serves dispatch and the existing signed workerd diagnostic.
// Router::methods() inspects the registered compiled handlers.
trait OwnedService {
    const OWNER: &'static str;
}

pub(super) type MethodOwners = std::collections::BTreeMap<String, &'static str>;

struct ServiceRegistry<'a> {
    router: ConnectRouter,
    owners: Option<&'a mut MethodOwners>,
}

impl ServiceRegistry<'_> {
    fn add_service<S: OwnedService, Marker>(mut self, service: Arc<S>) -> Self
    where
        Arc<S>: connectrpc::ServiceRegister<Marker>,
    {
        let routes = ConnectRouter::new().add_service(service);
        if let Some(owners) = self.owners.as_mut() {
            for path in routes.methods() {
                owners.insert(format!("/{}", path.trim_start_matches('/')), S::OWNER);
            }
        }
        self.router = self.router.merge(routes);
        self
    }
}

pub(super) fn router(
    env: Env,
    verified: &access::Actor,
    request_id: &str,
    owners: Option<&mut MethodOwners>,
) -> ConnectRouter {
    let env = Arc::new(env);
    let tenant = &verified.tenant;
    let actor = &verified.identity;
    ServiceRegistry {
        router: ConnectRouter::new(),
        owners,
    }
    .add_service(Arc::new(PartyV3Service {
        env: Arc::clone(&env),
        actor: verified.clone(),
        request_id: request_id.to_owned(),
    }))
    .add_service(Arc::new(AccountV2Service {
        env: Arc::clone(&env),
        actor: verified.clone(),
        request_id: request_id.to_owned(),
    }))
    .add_service(Arc::new(ContactPointV2Service {
        env: Arc::clone(&env),
        actor: verified.clone(),
        request_id: request_id.to_owned(),
    }))
    .add_service(Arc::new(RelationshipV2Service {
        env: Arc::clone(&env),
        tenant: tenant.to_owned(),
        actor: actor.to_owned(),
        request_id: request_id.to_owned(),
    }))
    .add_service(Arc::new(MemberV2Service {
        env: Arc::clone(&env),
        actor: verified.clone(),
        request_id: request_id.to_owned(),
    }))
    .add_service(Arc::new(TaskV2Service {
        env: Arc::clone(&env),
        actor: verified.clone(),
        request_id: request_id.to_owned(),
    }))
    .add_service(Arc::new(InteractionV2Service {
        env: Arc::clone(&env),
        actor: verified.clone(),
        request_id: request_id.to_owned(),
    }))
    .add_service(Arc::new(DataQualityV2Service {
        env: Arc::clone(&env),
        tenant: tenant.to_owned(),
    }))
    .add_service(Arc::new(IdentityV2Service {
        env: Arc::clone(&env),
        actor: verified.clone(),
        request_id: request_id.to_owned(),
    }))
    .add_service(Arc::new(ConsentV2Service {
        env: Arc::clone(&env),
        actor: verified.clone(),
        request_id: request_id.to_owned(),
    }))
    .add_service(Arc::new(Customer360V2Service {
        env: Arc::clone(&env),
        tenant: tenant.to_owned(),
    }))
    .add_service(Arc::new(PartyCsvV2Service {
        env: Arc::clone(&env),
        actor: verified.clone(),
        request_id: request_id.to_owned(),
    }))
    .add_service(Arc::new(EnrichmentV2Service {
        env: Arc::clone(&env),
        actor: verified.clone(),
        request_id: request_id.to_owned(),
    }))
    .add_service(Arc::new(PrivacyV2Service {
        env: Arc::clone(&env),
        actor: verified.clone(),
        request_id: request_id.to_owned(),
    }))
    .add_service(Arc::new(SalesV2Service {
        env: Arc::clone(&env),
        tenant: tenant.to_owned(),
        actor: actor.to_owned(),
        admin: verified.role == "admin",
        request_id: request_id.to_owned(),
    }))
    .router
}

pub(super) async fn route(
    req: Request,
    env: Env,
    actor: &access::Actor,
    request_id: &str,
    policy: super::RpcPolicy,
) -> Result<Response> {
    // Preserve the typed Connect code for the existing request log; never log payloads.
    let correlation = request_id.to_owned();
    let service = router(env, actor, request_id, None)
        .into_axum_service()
        .with_interceptor(connectrpc::unary_interceptor(move |req, next| {
            let correlation = correlation.clone();
            Box::pin(async move {
                next.run(req).await.map_err(|error| {
                    let mut error = policy.wire_error(error, &correlation, false);
                    let code = axum::http::HeaderValue::from_static(error.code.as_str());
                    error
                        .response_headers_mut()
                        .insert("x-crm-error-code", code);
                    error
                })
            })
        }));
    let mut app: Router = Router::new().fallback_service(service);
    let http = HttpRequest::try_from(req)?;
    Response::try_from(app.call(http).await?)
}
