#![forbid(unsafe_code)]

//! Rebuildable Customer 360 contribution projection.
//!
//! This crate owns no authoritative customer state. It validates immutable owner
//! events and materializes exactly one current contribution document per source
//! aggregate. Each document carries the complete current set of Party roots it
//! contributes to, so mutable Account associations replace stale root membership
//! atomically instead of requiring projection delete-writes.

use crm_core_data::PostgresDataStore;
use crm_core_events::ProjectionDocumentWrite;
use crm_module_sdk::{
    DataClass, ErrorCategory, EventDelivery, EventType, ModuleId, PayloadEncoding, SdkError,
    TenantId,
};
use crm_projection_runtime::{
    ProjectionBatchResult, ProjectionDefinition, ProjectionHandler, ProjectionId,
    ProjectionRegistry, ProjectionRunner,
};
use crm_proto_contracts::{
    crm::{
        accounts::v1 as accounts, contact_points::v1 as contact_points, parties::v1 as parties,
        party_relationships::v1 as party_relationships,
    },
    message_descriptor_hash,
};
use prost::Message;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::Arc;

pub const CUSTOMER_360_PROJECTION_ID: &str = "customer.customer-360.v1";
pub const CUSTOMER_360_CONSUMER_MODULE_ID: &str = "crm.customer360-projection";
pub const CUSTOMER_360_CONTRIBUTION_RESOURCE_TYPE: &str = "customer-360.contribution";
pub const CUSTOMER_360_PROJECTION_SCHEMA_VERSION: &str = "1";

const CONTRACT_VERSION: &str = "1.0.0";

const PARTIES_MODULE_ID: &str = "crm.parties";
const PARTY_RECORD_TYPE: &str = "parties.party";
const PARTY_CREATED: &str = "parties.party.created";
const PARTY_UPDATED: &str = "parties.party.updated";
const PARTY_CREATED_SCHEMA: &str = "crm.parties.v1.PartyCreatedEvent";
const PARTY_UPDATED_SCHEMA: &str = "crm.parties.v1.PartyUpdatedEvent";

const ACCOUNTS_MODULE_ID: &str = "crm.customer-accounts";
const ACCOUNT_RECORD_TYPE: &str = "accounts.account";
const ACCOUNT_CREATED: &str = "accounts.account.created";
const ACCOUNT_UPDATED: &str = "accounts.account.updated";
const ACCOUNT_CREATED_SCHEMA: &str = "crm.accounts.v1.AccountCreatedEvent";
const ACCOUNT_UPDATED_SCHEMA: &str = "crm.accounts.v1.AccountUpdatedEvent";

const CONTACT_POINTS_MODULE_ID: &str = "crm.contact-points";
const CONTACT_POINT_RECORD_TYPE: &str = "contact-points.contact_point";
const CONTACT_POINT_CREATED: &str = "contact-points.contact-point.created";
const CONTACT_POINT_UPDATED: &str = "contact-points.contact-point.updated";
const CONTACT_POINT_VERIFIED: &str = "contact-points.contact-point.verified";
const CONTACT_POINT_CREATED_SCHEMA: &str = "crm.contact_points.v1.ContactPointCreatedEvent";
const CONTACT_POINT_UPDATED_SCHEMA: &str = "crm.contact_points.v1.ContactPointUpdatedEvent";
const CONTACT_POINT_VERIFIED_SCHEMA: &str = "crm.contact_points.v1.ContactPointVerifiedEvent";

const PARTY_RELATIONSHIPS_MODULE_ID: &str = "crm.party-relationships";
const PARTY_RELATIONSHIP_RECORD_TYPE: &str = "party-relationships.party_relationship";
const PARTY_RELATIONSHIP_CREATED: &str = "party-relationships.party-relationship.created";
const PARTY_RELATIONSHIP_UPDATED: &str = "party-relationships.party-relationship.updated";
const PARTY_RELATIONSHIP_CREATED_SCHEMA: &str =
    "crm.party_relationships.v1.PartyRelationshipCreatedEvent";
const PARTY_RELATIONSHIP_UPDATED_SCHEMA: &str =
    "crm.party_relationships.v1.PartyRelationshipUpdatedEvent";

const ALL_EVENT_TYPES: [&str; 9] = [
    PARTY_CREATED,
    PARTY_UPDATED,
    ACCOUNT_CREATED,
    ACCOUNT_UPDATED,
    CONTACT_POINT_CREATED,
    CONTACT_POINT_UPDATED,
    CONTACT_POINT_VERIFIED,
    PARTY_RELATIONSHIP_CREATED,
    PARTY_RELATIONSHIP_UPDATED,
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Customer360ContributionKind {
    Party,
    Account,
    ContactPoint,
    PartyRelationship,
}

impl Customer360ContributionKind {
    const fn storage_key(&self) -> &'static str {
        match self {
            Self::Party => "party",
            Self::Account => "account",
            Self::ContactPoint => "contact-point",
            Self::PartyRelationship => "party-relationship",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Customer360ContributionDocument {
    pub projection_schema_version: String,
    pub contribution_kind: Customer360ContributionKind,
    pub root_party_ids: Vec<String>,
    pub source_owner_module_id: String,
    pub source_resource_type: String,
    pub source_resource_id: String,
    pub source_version: i64,
    pub source_event_id: String,
    pub snapshot: Customer360ContributionSnapshot,
}

impl Customer360ContributionDocument {
    pub fn from_json(value: &serde_json::Value) -> Result<Self, SdkError> {
        let document: Self = serde_json::from_value(value.clone())
            .map_err(|error| contribution_invalid(error.to_string()))?;
        document.validate()?;
        Ok(document)
    }

    pub fn validate(&self) -> Result<(), SdkError> {
        if self.projection_schema_version != CUSTOMER_360_PROJECTION_SCHEMA_VERSION
            || self.root_party_ids.is_empty()
            || self.source_owner_module_id.is_empty()
            || self.source_resource_type.is_empty()
            || self.source_resource_id.is_empty()
            || self.source_version <= 0
            || self.source_event_id.is_empty()
        {
            return Err(contribution_invalid(
                "Customer 360 contribution identity is invalid",
            ));
        }
        let canonical_roots = self
            .root_party_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if canonical_roots != self.root_party_ids
            || self
                .root_party_ids
                .iter()
                .any(|party_id| party_id.is_empty())
        {
            return Err(contribution_invalid(
                "Customer 360 root Party ids are not canonical",
            ));
        }
        Ok(())
    }

    pub fn affects_party(&self, party_id: &str) -> bool {
        self.root_party_ids
            .binary_search_by(|candidate| candidate.as_str().cmp(party_id))
            .is_ok()
    }

    pub fn resource_id(&self) -> String {
        format!(
            "{}:{}",
            self.contribution_kind.storage_key(),
            self.source_resource_id
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "snapshot_kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Customer360ContributionSnapshot {
    Party(PartyContributionSnapshot),
    Account(AccountContributionSnapshot),
    ContactPoint(ContactPointContributionSnapshot),
    PartyRelationship(PartyRelationshipContributionSnapshot),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartyContributionSnapshot {
    pub kind: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountContributionSnapshot {
    pub name: String,
    pub status: String,
    pub party_associations: Vec<AccountPartyAssociationContribution>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountPartyAssociationContribution {
    pub party_id: String,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContactPointContributionSnapshot {
    pub party_id: String,
    pub kind: String,
    pub normalized_value: String,
    pub display_value: String,
    pub status: String,
    pub preferred: bool,
    pub valid_from_unix_nanos: Option<i64>,
    pub valid_until_unix_nanos: Option<i64>,
    pub verification_status: String,
    pub verification_evidence_ref: Option<String>,
    pub verified_at_unix_nanos: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartyRelationshipContributionSnapshot {
    pub from_party_id: String,
    pub to_party_id: String,
    pub relationship_type_code: String,
    pub directionality: String,
    pub from_role: String,
    pub to_role: String,
    pub status: String,
    pub valid_from_unix_nanos: Option<i64>,
    pub valid_until_unix_nanos: Option<i64>,
}

#[derive(Debug, Clone, Copy)]
struct Customer360ProjectionHandler;

impl ProjectionHandler for Customer360ProjectionHandler {
    fn project(&self, delivery: &EventDelivery) -> Result<Vec<ProjectionDocumentWrite>, SdkError> {
        let document = match delivery.event_type.as_str() {
            PARTY_CREATED => {
                validate_contract(
                    delivery,
                    PARTIES_MODULE_ID,
                    PARTY_RECORD_TYPE,
                    PARTY_CREATED_SCHEMA,
                )?;
                let event = decode::<parties::PartyCreatedEvent>(delivery)?;
                party_contribution(
                    delivery,
                    event
                        .party
                        .ok_or_else(|| event_invalid("Party created event is missing Party"))?,
                )?
            }
            PARTY_UPDATED => {
                validate_contract(
                    delivery,
                    PARTIES_MODULE_ID,
                    PARTY_RECORD_TYPE,
                    PARTY_UPDATED_SCHEMA,
                )?;
                let event = decode::<parties::PartyUpdatedEvent>(delivery)?;
                party_contribution(
                    delivery,
                    event
                        .party
                        .ok_or_else(|| event_invalid("Party updated event is missing Party"))?,
                )?
            }
            ACCOUNT_CREATED => {
                validate_contract(
                    delivery,
                    ACCOUNTS_MODULE_ID,
                    ACCOUNT_RECORD_TYPE,
                    ACCOUNT_CREATED_SCHEMA,
                )?;
                let event = decode::<accounts::AccountCreatedEvent>(delivery)?;
                account_contribution(
                    delivery,
                    event
                        .account
                        .ok_or_else(|| event_invalid("Account created event is missing Account"))?,
                )?
            }
            ACCOUNT_UPDATED => {
                validate_contract(
                    delivery,
                    ACCOUNTS_MODULE_ID,
                    ACCOUNT_RECORD_TYPE,
                    ACCOUNT_UPDATED_SCHEMA,
                )?;
                let event = decode::<accounts::AccountUpdatedEvent>(delivery)?;
                account_contribution(
                    delivery,
                    event
                        .account
                        .ok_or_else(|| event_invalid("Account updated event is missing Account"))?,
                )?
            }
            CONTACT_POINT_CREATED => {
                validate_contract(
                    delivery,
                    CONTACT_POINTS_MODULE_ID,
                    CONTACT_POINT_RECORD_TYPE,
                    CONTACT_POINT_CREATED_SCHEMA,
                )?;
                let event = decode::<contact_points::ContactPointCreatedEvent>(delivery)?;
                contact_point_contribution(
                    delivery,
                    event.contact_point.ok_or_else(|| {
                        event_invalid("Contact Point created event is missing Contact Point")
                    })?,
                )?
            }
            CONTACT_POINT_UPDATED => {
                validate_contract(
                    delivery,
                    CONTACT_POINTS_MODULE_ID,
                    CONTACT_POINT_RECORD_TYPE,
                    CONTACT_POINT_UPDATED_SCHEMA,
                )?;
                let event = decode::<contact_points::ContactPointUpdatedEvent>(delivery)?;
                contact_point_contribution(
                    delivery,
                    event.contact_point.ok_or_else(|| {
                        event_invalid("Contact Point updated event is missing Contact Point")
                    })?,
                )?
            }
            CONTACT_POINT_VERIFIED => {
                validate_contract(
                    delivery,
                    CONTACT_POINTS_MODULE_ID,
                    CONTACT_POINT_RECORD_TYPE,
                    CONTACT_POINT_VERIFIED_SCHEMA,
                )?;
                let event = decode::<contact_points::ContactPointVerifiedEvent>(delivery)?;
                contact_point_contribution(
                    delivery,
                    event.contact_point.ok_or_else(|| {
                        event_invalid("Contact Point verified event is missing Contact Point")
                    })?,
                )?
            }
            PARTY_RELATIONSHIP_CREATED => {
                validate_contract(
                    delivery,
                    PARTY_RELATIONSHIPS_MODULE_ID,
                    PARTY_RELATIONSHIP_RECORD_TYPE,
                    PARTY_RELATIONSHIP_CREATED_SCHEMA,
                )?;
                let event = decode::<party_relationships::PartyRelationshipCreatedEvent>(delivery)?;
                party_relationship_contribution(
                    delivery,
                    event.party_relationship.ok_or_else(|| {
                        event_invalid(
                            "Party Relationship created event is missing Party Relationship",
                        )
                    })?,
                )?
            }
            PARTY_RELATIONSHIP_UPDATED => {
                validate_contract(
                    delivery,
                    PARTY_RELATIONSHIPS_MODULE_ID,
                    PARTY_RELATIONSHIP_RECORD_TYPE,
                    PARTY_RELATIONSHIP_UPDATED_SCHEMA,
                )?;
                let event = decode::<party_relationships::PartyRelationshipUpdatedEvent>(delivery)?;
                party_relationship_contribution(
                    delivery,
                    event.party_relationship.ok_or_else(|| {
                        event_invalid(
                            "Party Relationship updated event is missing Party Relationship",
                        )
                    })?,
                )?
            }
            _ => {
                return Err(event_invalid(
                    "Customer 360 projection event type is unsupported",
                ));
            }
        };
        document.validate()?;
        let resource_id = document.resource_id();
        let source_version = document.source_version;
        let value = serde_json::to_value(document)
            .map_err(|error| contribution_invalid(error.to_string()))?;
        Ok(vec![ProjectionDocumentWrite {
            resource_type: CUSTOMER_360_CONTRIBUTION_RESOURCE_TYPE.to_owned(),
            resource_id,
            source_version,
            document: value,
        }])
    }
}

pub fn customer_360_projection_registry() -> Result<ProjectionRegistry, SdkError> {
    let event_types = ALL_EVENT_TYPES
        .into_iter()
        .map(EventType::try_new)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| configuration_invalid(error.to_string()))?;
    let definition = ProjectionDefinition::new(
        ProjectionId::try_new(CUSTOMER_360_PROJECTION_ID)
            .map_err(|error| configuration_invalid(error.to_string()))?,
        ModuleId::try_new(CUSTOMER_360_CONSUMER_MODULE_ID)
            .map_err(|error| configuration_invalid(error.to_string()))?,
        event_types,
        Arc::new(Customer360ProjectionHandler),
    )?;
    ProjectionRegistry::new(vec![definition])
}

#[derive(Debug, Clone)]
pub struct Customer360ProjectionWorker {
    runner: ProjectionRunner,
}

impl Customer360ProjectionWorker {
    pub fn new(store: PostgresDataStore) -> Result<Self, SdkError> {
        Ok(Self {
            runner: ProjectionRunner::new(Arc::new(store), customer_360_projection_registry()?),
        })
    }

    pub async fn run_batch(
        &self,
        tenant_id: TenantId,
        page_size: u32,
    ) -> Result<ProjectionBatchResult, SdkError> {
        self.runner
            .run_batch(tenant_id, CUSTOMER_360_PROJECTION_ID, page_size)
            .await
    }

    pub async fn rebuild(&self, tenant_id: TenantId, page_size: u32) -> Result<u64, SdkError> {
        self.runner
            .rebuild(tenant_id, CUSTOMER_360_PROJECTION_ID, page_size)
            .await
    }

    pub fn runner(&self) -> &ProjectionRunner {
        &self.runner
    }
}

fn party_contribution(
    delivery: &EventDelivery,
    party: parties::Party,
) -> Result<Customer360ContributionDocument, SdkError> {
    let party_id = party
        .party_ref
        .ok_or_else(|| event_invalid("Party snapshot is missing Party reference"))?
        .party_id;
    let version = resource_version(
        party.resource_version,
        "Party snapshot is missing resource version",
    )?;
    validate_snapshot_identity(delivery, &party_id, version)?;
    let kind = match parties::PartyKind::try_from(party.kind) {
        Ok(parties::PartyKind::Person) => "person",
        Ok(parties::PartyKind::Organization) => "organization",
        Ok(parties::PartyKind::Unspecified) | Err(_) => {
            return Err(event_invalid("Party snapshot kind is invalid"));
        }
    };
    let display_name = party.display_name.trim().to_owned();
    if display_name.is_empty() {
        return Err(event_invalid("Party display name must not be empty"));
    }
    Ok(contribution_document(
        delivery,
        Customer360ContributionKind::Party,
        vec![party_id],
        version,
        Customer360ContributionSnapshot::Party(PartyContributionSnapshot {
            kind: kind.to_owned(),
            display_name,
        }),
    ))
}

fn account_contribution(
    delivery: &EventDelivery,
    account: accounts::Account,
) -> Result<Customer360ContributionDocument, SdkError> {
    let account_id = account
        .account_ref
        .ok_or_else(|| event_invalid("Account snapshot is missing Account reference"))?
        .account_id;
    let version = resource_version(
        account.resource_version,
        "Account snapshot is missing resource version",
    )?;
    validate_snapshot_identity(delivery, &account_id, version)?;
    let status = match accounts::AccountStatus::try_from(account.status) {
        Ok(accounts::AccountStatus::Active) => "active",
        Ok(accounts::AccountStatus::Inactive) => "inactive",
        Ok(accounts::AccountStatus::Unspecified) | Err(_) => {
            return Err(event_invalid("Account snapshot status is invalid"));
        }
    };
    let name = account.name.trim().to_owned();
    if name.is_empty() {
        return Err(event_invalid("Account name must not be empty"));
    }
    let mut associations = account
        .party_associations
        .into_iter()
        .map(|association| {
            let party_id = association
                .party_ref
                .ok_or_else(|| event_invalid("Account association is missing Party reference"))?
                .party_id;
            if party_id.is_empty() {
                return Err(event_invalid("Account association Party id is empty"));
            }
            let role = match accounts::AccountPartyRole::try_from(association.role) {
                Ok(accounts::AccountPartyRole::Primary) => "primary",
                Ok(accounts::AccountPartyRole::Member) => "member",
                Ok(accounts::AccountPartyRole::Unspecified) | Err(_) => {
                    return Err(event_invalid("Account association role is invalid"));
                }
            };
            Ok(AccountPartyAssociationContribution {
                party_id,
                role: role.to_owned(),
            })
        })
        .collect::<Result<Vec<_>, SdkError>>()?;
    associations.sort_by(|left, right| {
        left.party_id
            .cmp(&right.party_id)
            .then(left.role.cmp(&right.role))
    });
    if associations.is_empty()
        || associations
            .windows(2)
            .any(|pair| pair[0].party_id == pair[1].party_id)
    {
        return Err(event_invalid(
            "Account associations must contain distinct Party references",
        ));
    }
    let root_party_ids = associations
        .iter()
        .map(|association| association.party_id.clone())
        .collect::<Vec<_>>();
    Ok(contribution_document(
        delivery,
        Customer360ContributionKind::Account,
        root_party_ids,
        version,
        Customer360ContributionSnapshot::Account(AccountContributionSnapshot {
            name,
            status: status.to_owned(),
            party_associations: associations,
        }),
    ))
}

fn contact_point_contribution(
    delivery: &EventDelivery,
    contact_point: contact_points::ContactPoint,
) -> Result<Customer360ContributionDocument, SdkError> {
    let contact_point_id = contact_point
        .contact_point_ref
        .ok_or_else(|| event_invalid("Contact Point snapshot is missing reference"))?
        .contact_point_id;
    let party_id = contact_point
        .party_ref
        .ok_or_else(|| event_invalid("Contact Point snapshot is missing Party reference"))?
        .party_id;
    let version = resource_version(
        contact_point.resource_version,
        "Contact Point snapshot is missing resource version",
    )?;
    validate_snapshot_identity(delivery, &contact_point_id, version)?;
    let kind = contact_point_kind(contact_point.kind)?;
    let status = contact_point_status(contact_point.status)?;
    let verification = contact_point
        .verification
        .ok_or_else(|| event_invalid("Contact Point verification snapshot is missing"))?;
    let verification_status = verification_status(verification.status)?;
    Ok(contribution_document(
        delivery,
        Customer360ContributionKind::ContactPoint,
        vec![party_id.clone()],
        version,
        Customer360ContributionSnapshot::ContactPoint(ContactPointContributionSnapshot {
            party_id,
            kind: kind.to_owned(),
            normalized_value: contact_point.normalized_value,
            display_value: contact_point.display_value,
            status: status.to_owned(),
            preferred: contact_point.preferred,
            valid_from_unix_nanos: contact_point.valid_from.map(|value| value.unix_nanos),
            valid_until_unix_nanos: contact_point.valid_until.map(|value| value.unix_nanos),
            verification_status: verification_status.to_owned(),
            verification_evidence_ref: verification.evidence_ref,
            verified_at_unix_nanos: verification.verified_at.map(|value| value.unix_nanos),
        }),
    ))
}

fn party_relationship_contribution(
    delivery: &EventDelivery,
    relationship: party_relationships::PartyRelationship,
) -> Result<Customer360ContributionDocument, SdkError> {
    let relationship_id = relationship
        .party_relationship_ref
        .ok_or_else(|| event_invalid("Party Relationship snapshot is missing reference"))?
        .party_relationship_id;
    let from_party_id = relationship
        .from_party_ref
        .ok_or_else(|| event_invalid("Party Relationship snapshot is missing from Party"))?
        .party_id;
    let to_party_id = relationship
        .to_party_ref
        .ok_or_else(|| event_invalid("Party Relationship snapshot is missing to Party"))?
        .party_id;
    if from_party_id.is_empty() || to_party_id.is_empty() || from_party_id == to_party_id {
        return Err(event_invalid("Party Relationship endpoints are invalid"));
    }
    let version = resource_version(
        relationship.resource_version,
        "Party Relationship snapshot is missing resource version",
    )?;
    validate_snapshot_identity(delivery, &relationship_id, version)?;
    let relationship_type = relationship
        .relationship_type
        .ok_or_else(|| event_invalid("Party Relationship type is missing"))?;
    let directionality = match party_relationships::PartyRelationshipDirectionality::try_from(
        relationship_type.directionality,
    ) {
        Ok(party_relationships::PartyRelationshipDirectionality::Directional) => "directional",
        Ok(party_relationships::PartyRelationshipDirectionality::Reciprocal) => "reciprocal",
        Ok(party_relationships::PartyRelationshipDirectionality::Unspecified) | Err(_) => {
            return Err(event_invalid(
                "Party Relationship directionality is invalid",
            ));
        }
    };
    let status = match party_relationships::PartyRelationshipStatus::try_from(relationship.status) {
        Ok(party_relationships::PartyRelationshipStatus::Active) => "active",
        Ok(party_relationships::PartyRelationshipStatus::Inactive) => "inactive",
        Ok(party_relationships::PartyRelationshipStatus::Unspecified) | Err(_) => {
            return Err(event_invalid("Party Relationship status is invalid"));
        }
    };
    let roots = BTreeSet::from([from_party_id.clone(), to_party_id.clone()])
        .into_iter()
        .collect::<Vec<_>>();
    Ok(contribution_document(
        delivery,
        Customer360ContributionKind::PartyRelationship,
        roots,
        version,
        Customer360ContributionSnapshot::PartyRelationship(PartyRelationshipContributionSnapshot {
            from_party_id,
            to_party_id,
            relationship_type_code: relationship_type.code,
            directionality: directionality.to_owned(),
            from_role: relationship_type.from_role,
            to_role: relationship_type.to_role,
            status: status.to_owned(),
            valid_from_unix_nanos: relationship.valid_from.map(|value| value.unix_nanos),
            valid_until_unix_nanos: relationship.valid_until.map(|value| value.unix_nanos),
        }),
    ))
}

fn contribution_document(
    delivery: &EventDelivery,
    contribution_kind: Customer360ContributionKind,
    root_party_ids: Vec<String>,
    source_version: i64,
    snapshot: Customer360ContributionSnapshot,
) -> Customer360ContributionDocument {
    Customer360ContributionDocument {
        projection_schema_version: CUSTOMER_360_PROJECTION_SCHEMA_VERSION.to_owned(),
        contribution_kind,
        root_party_ids,
        source_owner_module_id: delivery.source_module_id.as_str().to_owned(),
        source_resource_type: delivery.aggregate.record_type.as_str().to_owned(),
        source_resource_id: delivery.aggregate.record_id.as_str().to_owned(),
        source_version,
        source_event_id: delivery.event_id.as_str().to_owned(),
        snapshot,
    }
}

fn validate_contract(
    delivery: &EventDelivery,
    module_id: &str,
    record_type: &str,
    schema_id: &str,
) -> Result<(), SdkError> {
    if delivery.source_module_id.as_str() != module_id
        || delivery.aggregate.record_type.as_str() != record_type
        || delivery.event_version.as_str() != CONTRACT_VERSION
        || delivery.payload.owner.as_str() != module_id
        || delivery.payload.schema_id.as_str() != schema_id
        || delivery.payload.schema_version.as_str() != CONTRACT_VERSION
        || delivery.payload.descriptor_hash != message_descriptor_hash(schema_id)
        || delivery.payload.data_class != DataClass::Personal
        || delivery.payload.encoding != PayloadEncoding::Protobuf
        || delivery.payload.validate().is_err()
    {
        return Err(event_invalid(
            "Customer 360 source event contract identity is invalid",
        ));
    }
    Ok(())
}

fn validate_snapshot_identity(
    delivery: &EventDelivery,
    resource_id: &str,
    version: i64,
) -> Result<(), SdkError> {
    if resource_id != delivery.aggregate.record_id.as_str() || version != delivery.aggregate_version
    {
        return Err(event_invalid(
            "Customer 360 source snapshot identity/version is inconsistent",
        ));
    }
    Ok(())
}

fn resource_version(
    version: Option<crm_proto_contracts::crm::customer::v1::CustomerResourceVersion>,
    missing_message: &'static str,
) -> Result<i64, SdkError> {
    let version = version
        .ok_or_else(|| event_invalid(missing_message))?
        .version;
    if version <= 0 {
        return Err(event_invalid("Customer 360 source version is invalid"));
    }
    Ok(version)
}

fn contact_point_kind(value: i32) -> Result<&'static str, SdkError> {
    match contact_points::ContactPointKind::try_from(value) {
        Ok(contact_points::ContactPointKind::Email) => Ok("email"),
        Ok(contact_points::ContactPointKind::Phone) => Ok("phone"),
        Ok(contact_points::ContactPointKind::Postal) => Ok("postal"),
        Ok(contact_points::ContactPointKind::Web) => Ok("web"),
        Ok(contact_points::ContactPointKind::Messaging) => Ok("messaging"),
        Ok(contact_points::ContactPointKind::Unspecified) | Err(_) => {
            Err(event_invalid("Contact Point kind is invalid"))
        }
    }
}

fn contact_point_status(value: i32) -> Result<&'static str, SdkError> {
    match contact_points::ContactPointStatus::try_from(value) {
        Ok(contact_points::ContactPointStatus::Active) => Ok("active"),
        Ok(contact_points::ContactPointStatus::Inactive) => Ok("inactive"),
        Ok(contact_points::ContactPointStatus::Unspecified) | Err(_) => {
            Err(event_invalid("Contact Point status is invalid"))
        }
    }
}

fn verification_status(value: i32) -> Result<&'static str, SdkError> {
    match contact_points::ContactPointVerificationStatus::try_from(value) {
        Ok(contact_points::ContactPointVerificationStatus::Unverified) => Ok("unverified"),
        Ok(contact_points::ContactPointVerificationStatus::Verified) => Ok("verified"),
        Ok(contact_points::ContactPointVerificationStatus::Unspecified) | Err(_) => Err(
            event_invalid("Contact Point verification status is invalid"),
        ),
    }
}

fn decode<M>(delivery: &EventDelivery) -> Result<M, SdkError>
where
    M: Message + Default,
{
    M::decode(delivery.payload.bytes.as_slice()).map_err(|error| event_invalid(error.to_string()))
}

fn configuration_invalid(internal: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_360_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "Customer 360 projection configuration is invalid.",
    )
    .with_internal_reference(internal.into())
}

fn event_invalid(internal: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_360_SOURCE_EVENT_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "A Customer 360 source event is invalid.",
    )
    .with_internal_reference(internal.into())
}

fn contribution_invalid(internal: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_360_CONTRIBUTION_INVALID",
        ErrorCategory::Internal,
        false,
        "A Customer 360 projection contribution is invalid.",
    )
    .with_internal_reference(internal.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_module_sdk::{
        ActorId, CorrelationId, DeliveryId, EventId, EventVersion, RecordId, RecordRef, RecordType,
        RetentionPolicyId, SchemaId, SchemaVersion, TraceId, TypedPayload,
    };
    use crm_proto_contracts::crm::{core::v1 as core, customer::v1 as customer};

    #[test]
    fn account_update_document_replaces_the_complete_canonical_root_set() {
        let delivery = delivery(
            ACCOUNTS_MODULE_ID,
            ACCOUNT_RECORD_TYPE,
            ACCOUNT_UPDATED,
            ACCOUNT_UPDATED_SCHEMA,
            "account-1",
            4,
            accounts::AccountUpdatedEvent {
                account: Some(accounts::Account {
                    account_ref: Some(customer::AccountRef {
                        account_id: "account-1".to_owned(),
                    }),
                    name: "Enterprise Account".to_owned(),
                    status: accounts::AccountStatus::Active as i32,
                    party_associations: vec![
                        accounts::AccountPartyAssociation {
                            party_ref: Some(customer::PartyRef {
                                party_id: "party-z".to_owned(),
                            }),
                            role: accounts::AccountPartyRole::Member as i32,
                        },
                        accounts::AccountPartyAssociation {
                            party_ref: Some(customer::PartyRef {
                                party_id: "party-a".to_owned(),
                            }),
                            role: accounts::AccountPartyRole::Primary as i32,
                        },
                    ],
                    resource_version: Some(customer::CustomerResourceVersion {
                        version: 4,
                        ..Default::default()
                    }),
                }),
            },
        );
        let writes = Customer360ProjectionHandler.project(&delivery).unwrap();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].resource_id, "account:account-1");
        let document = Customer360ContributionDocument::from_json(&writes[0].document).unwrap();
        assert_eq!(
            document.root_party_ids,
            vec!["party-a".to_owned(), "party-z".to_owned()]
        );
        assert!(document.affects_party("party-a"));
        assert!(!document.affects_party("party-removed"));
    }

    #[test]
    fn contact_point_verification_snapshot_is_projected_as_owner_state_not_consent() {
        let delivery = delivery(
            CONTACT_POINTS_MODULE_ID,
            CONTACT_POINT_RECORD_TYPE,
            CONTACT_POINT_VERIFIED,
            CONTACT_POINT_VERIFIED_SCHEMA,
            "contact-point-1",
            2,
            contact_points::ContactPointVerifiedEvent {
                contact_point: Some(contact_points::ContactPoint {
                    contact_point_ref: Some(customer::ContactPointRef {
                        contact_point_id: "contact-point-1".to_owned(),
                    }),
                    party_ref: Some(customer::PartyRef {
                        party_id: "party-a".to_owned(),
                    }),
                    kind: contact_points::ContactPointKind::Email as i32,
                    normalized_value: "ada@example.com".to_owned(),
                    display_value: "Ada@example.com".to_owned(),
                    status: contact_points::ContactPointStatus::Active as i32,
                    preferred: true,
                    valid_from: None,
                    valid_until: None,
                    verification: Some(contact_points::ContactPointVerification {
                        status: contact_points::ContactPointVerificationStatus::Verified as i32,
                        evidence_ref: Some("evidence-1".to_owned()),
                        verified_at: Some(core::UnixTime { unix_nanos: 20 }),
                    }),
                    resource_version: Some(customer::CustomerResourceVersion {
                        version: 2,
                        ..Default::default()
                    }),
                }),
            },
        );
        let write = Customer360ProjectionHandler
            .project(&delivery)
            .unwrap()
            .remove(0);
        let document = Customer360ContributionDocument::from_json(&write.document).unwrap();
        let Customer360ContributionSnapshot::ContactPoint(snapshot) = document.snapshot else {
            panic!("expected Contact Point contribution")
        };
        assert_eq!(snapshot.verification_status, "verified");
        assert_eq!(
            snapshot.verification_evidence_ref.as_deref(),
            Some("evidence-1")
        );
    }

    #[test]
    fn relationship_contribution_targets_both_canonical_party_roots() {
        let delivery = delivery(
            PARTY_RELATIONSHIPS_MODULE_ID,
            PARTY_RELATIONSHIP_RECORD_TYPE,
            PARTY_RELATIONSHIP_CREATED,
            PARTY_RELATIONSHIP_CREATED_SCHEMA,
            "relationship-1",
            1,
            party_relationships::PartyRelationshipCreatedEvent {
                party_relationship: Some(party_relationships::PartyRelationship {
                    party_relationship_ref: Some(customer::PartyRelationshipRef {
                        party_relationship_id: "relationship-1".to_owned(),
                    }),
                    from_party_ref: Some(customer::PartyRef {
                        party_id: "party-z".to_owned(),
                    }),
                    to_party_ref: Some(customer::PartyRef {
                        party_id: "party-a".to_owned(),
                    }),
                    relationship_type: Some(party_relationships::PartyRelationshipType {
                        code: "household".to_owned(),
                        directionality:
                            party_relationships::PartyRelationshipDirectionality::Reciprocal as i32,
                        from_role: "household_member".to_owned(),
                        to_role: "household_member".to_owned(),
                    }),
                    status: party_relationships::PartyRelationshipStatus::Active as i32,
                    valid_from: None,
                    valid_until: None,
                    resource_version: Some(customer::CustomerResourceVersion {
                        version: 1,
                        ..Default::default()
                    }),
                }),
            },
        );
        let write = Customer360ProjectionHandler
            .project(&delivery)
            .unwrap()
            .remove(0);
        let document = Customer360ContributionDocument::from_json(&write.document).unwrap();
        assert_eq!(
            document.root_party_ids,
            vec!["party-a".to_owned(), "party-z".to_owned()]
        );
    }

    #[test]
    fn registry_subscribes_to_all_owner_snapshot_events() {
        let registry = customer_360_projection_registry().unwrap();
        let definition = registry
            .get(CUSTOMER_360_PROJECTION_ID)
            .expect("Customer 360 projection definition");
        assert_eq!(definition.event_types().len(), ALL_EVENT_TYPES.len());
    }

    fn delivery<M: Message>(
        module_id: &str,
        record_type: &str,
        event_type: &str,
        schema_id: &str,
        resource_id: &str,
        version: i64,
        message: M,
    ) -> EventDelivery {
        let module_id = ModuleId::try_new(module_id).unwrap();
        EventDelivery {
            delivery_id: DeliveryId::try_new(format!("delivery-{resource_id}-{version}")).unwrap(),
            event_id: EventId::try_new(format!("event-{resource_id}-{version}")).unwrap(),
            tenant_id: TenantId::try_new("tenant-a").unwrap(),
            source_module_id: module_id.clone(),
            consumer_module_id: ModuleId::try_new(CUSTOMER_360_CONSUMER_MODULE_ID).unwrap(),
            source_actor_id: ActorId::try_new("actor-a").unwrap(),
            event_type: EventType::try_new(event_type).unwrap(),
            event_version: EventVersion::try_new(CONTRACT_VERSION).unwrap(),
            aggregate: RecordRef {
                record_type: RecordType::try_new(record_type).unwrap(),
                record_id: RecordId::try_new(resource_id).unwrap(),
            },
            aggregate_version: version,
            occurred_at_unix_nanos: 100,
            correlation_id: CorrelationId::try_new("customer-360-correlation").unwrap(),
            trace_id: TraceId::try_new("customer-360-trace").unwrap(),
            payload: TypedPayload {
                owner: module_id,
                schema_id: SchemaId::try_new(schema_id).unwrap(),
                schema_version: SchemaVersion::try_new(CONTRACT_VERSION).unwrap(),
                descriptor_hash: message_descriptor_hash(schema_id),
                data_class: DataClass::Personal,
                encoding: PayloadEncoding::Protobuf,
                maximum_size_bytes: 1_048_576,
                retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
                bytes: message.encode_to_vec(),
            },
        }
    }
}
