#![forbid(unsafe_code)]

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_core_data::PostgresDataStore;
use crm_customer_360_composition::{
    AccountContributionSnapshot, CUSTOMER_360_CONTRIBUTION_RESOURCE_TYPE,
    CUSTOMER_360_PROJECTION_ID, ContactPointContributionSnapshot, Customer360ContributionDocument,
    Customer360ContributionKind, Customer360ContributionSnapshot, PartyContributionSnapshot,
    PartyRelationshipContributionSnapshot,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, PayloadEncoding,
    PortFuture, RecordId, RecordRef, RecordType, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::{
    accounts::v1 as accounts, contact_points::v1 as contact_points, customer::v1 as customer,
    customer_360::v1 as wire, parties::v1 as parties,
    party_relationships::v1 as party_relationships,
};
use crm_query_runtime::{
    QueryExecutionResult, QueryExecutor, QueryRequest, QuerySemanticValidator,
    QueryVisibilityAuthorizer, QueryVisibilityDecision,
};
use prost::Message;
use serde_json::Value;
use sqlx::Row;
use std::collections::BTreeSet;
use std::sync::Arc;

pub const MODULE_ID: &str = "crm.customer360";
pub const GET_CAPABILITY: &str = "customer360.customer.get";
pub const GET_REQUEST_SCHEMA: &str = "crm.customer_360.v1.GetCustomer360Request";
pub const GET_RESPONSE_SCHEMA: &str = "crm.customer_360.v1.GetCustomer360Response";
pub const QUERY_CAPABILITY_IDS: [&str; 1] = [GET_CAPABILITY];

const PARTIES_MODULE_ID: &str = "crm.parties";
const PARTY_RECORD_TYPE: &str = "parties.party";
const ACCOUNTS_MODULE_ID: &str = "crm.customer-accounts";
const ACCOUNT_RECORD_TYPE: &str = "accounts.account";
const CONTACT_POINTS_MODULE_ID: &str = "crm.contact-points";
const CONTACT_POINT_RECORD_TYPE: &str = "contact-points.contact_point";
const PARTY_RELATIONSHIPS_MODULE_ID: &str = "crm.party-relationships";
const PARTY_RELATIONSHIP_RECORD_TYPE: &str = "party-relationships.party_relationship";
const MAXIMUM_CONTRIBUTIONS_PER_ROOT: usize = 5_000;

#[derive(Clone)]
pub struct Customer360QueryAdapter {
    store: PostgresDataStore,
    visibility: Arc<dyn QueryVisibilityAuthorizer>,
}

impl std::fmt::Debug for Customer360QueryAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Customer360QueryAdapter")
            .field("store", &self.store)
            .field("visibility", &"dyn QueryVisibilityAuthorizer")
            .finish()
    }
}

impl Customer360QueryAdapter {
    pub fn new(store: PostgresDataStore, visibility: Arc<dyn QueryVisibilityAuthorizer>) -> Self {
        Self { store, visibility }
    }
}

pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    Ok(vec![query_capability_definition()?])
}

pub fn query_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: support::configured_identifier(CapabilityId::try_new(GET_CAPABILITY))?,
        capability_version: support::configured_identifier(CapabilityVersion::try_new(
            support::CONTRACT_VERSION,
        ))?,
        owner_module_id: support::configured_identifier(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            GET_REQUEST_SCHEMA,
            vec![DataClass::Personal],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            GET_RESPONSE_SCHEMA,
            vec![DataClass::Personal],
        )?),
        risk: CapabilityRisk::Low,
        mutation: false,
        requires_idempotency: false,
        requires_approval: false,
        authorization_policy_id: GET_CAPABILITY.to_owned(),
        rate_limit_policy_id: None,
    })
}

impl QuerySemanticValidator for Customer360QueryAdapter {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            if definition.capability_id.as_str() != GET_CAPABILITY {
                return Err(unsupported_query());
            }
            let command: wire::GetCustomer360Request = decode_input(request, GET_REQUEST_SCHEMA)?;
            let party_ref = command.party_ref.ok_or_else(|| {
                SdkError::invalid_argument(
                    "customer_360.party_ref",
                    "Customer 360 Party reference is required",
                )
            })?;
            validate_party_id(&party_ref.party_id)?;
            Ok(())
        })
    }
}

impl QueryExecutor for Customer360QueryAdapter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        Box::pin(async move {
            if definition.capability_id.as_str() != GET_CAPABILITY {
                return Err(unsupported_query());
            }
            let output = self.execute_get(&request).await?;
            Ok(QueryExecutionResult { output })
        })
    }
}

impl Customer360QueryAdapter {
    async fn execute_get(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::GetCustomer360Request = decode_input(request, GET_REQUEST_SCHEMA)?;
        let party_ref = command.party_ref.ok_or_else(|| {
            SdkError::invalid_argument(
                "customer_360.party_ref",
                "Customer 360 Party reference is required",
            )
        })?;
        let root_party_id = validate_party_id(&party_ref.party_id)?;
        let projection = self
            .load_projection_snapshot(request, root_party_id.as_str())
            .await?;

        let root_visibility = self
            .authorize_source(
                request,
                PARTIES_MODULE_ID,
                PARTY_RECORD_TYPE,
                root_party_id.as_str(),
            )
            .await?;
        if !root_visibility.resource_visible {
            return Err(resource_not_found());
        }

        let mut party = None;
        let mut accounts_output = Vec::new();
        let mut contact_points_output = Vec::new();
        let mut relationships_output = Vec::new();

        for document in projection.documents {
            validate_document_shape(&document)?;
            match document.snapshot.clone() {
                Customer360ContributionSnapshot::Party(snapshot) => {
                    if document.source_resource_id != root_party_id.as_str() {
                        continue;
                    }
                    if party.is_some() {
                        return Err(projection_corrupt(
                            "multiple Party contributions exist for one Customer 360 root",
                        ));
                    }
                    party = Some(wire::Customer360PartySection {
                        party: Some(party_to_wire(&document, snapshot, &root_visibility)?),
                        source: Some(lineage_to_wire(&document)),
                    });
                }
                Customer360ContributionSnapshot::Account(snapshot) => {
                    let decision = self.authorize_document(request, &document).await?;
                    if decision.resource_visible {
                        accounts_output.push(wire::Customer360AccountSection {
                            account: Some(account_to_wire(&document, snapshot, &decision)?),
                            source: Some(lineage_to_wire(&document)),
                        });
                    }
                }
                Customer360ContributionSnapshot::ContactPoint(snapshot) => {
                    let decision = self.authorize_document(request, &document).await?;
                    if decision.resource_visible {
                        contact_points_output.push(wire::Customer360ContactPointSection {
                            contact_point: Some(contact_point_to_wire(
                                &document, snapshot, &decision,
                            )?),
                            source: Some(lineage_to_wire(&document)),
                        });
                    }
                }
                Customer360ContributionSnapshot::PartyRelationship(snapshot) => {
                    let decision = self.authorize_document(request, &document).await?;
                    if decision.resource_visible {
                        relationships_output.push(wire::Customer360PartyRelationshipSection {
                            party_relationship: Some(party_relationship_to_wire(
                                &document, snapshot, &decision,
                            )?),
                            source: Some(lineage_to_wire(&document)),
                        });
                    }
                }
            }
        }

        let party = party.ok_or_else(resource_not_found)?;
        accounts_output.sort_by(|left, right| {
            section_source_id(&left.source).cmp(section_source_id(&right.source))
        });
        contact_points_output.sort_by(|left, right| {
            section_source_id(&left.source).cmp(section_source_id(&right.source))
        });
        relationships_output.sort_by(|left, right| {
            section_source_id(&left.source).cmp(section_source_id(&right.source))
        });

        support::protobuf_payload(
            MODULE_ID,
            GET_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::GetCustomer360Response {
                customer_360: Some(wire::Customer360 {
                    root_party_ref: Some(customer::PartyRef {
                        party_id: root_party_id.as_str().to_owned(),
                    }),
                    party: Some(party),
                    accounts: accounts_output,
                    contact_points: contact_points_output,
                    party_relationships: relationships_output,
                    freshness: Some(projection.freshness),
                }),
            },
        )
    }

    async fn authorize_document(
        &self,
        request: &QueryRequest,
        document: &Customer360ContributionDocument,
    ) -> Result<QueryVisibilityDecision, SdkError> {
        self.authorize_source(
            request,
            &document.source_owner_module_id,
            &document.source_resource_type,
            &document.source_resource_id,
        )
        .await
    }

    async fn authorize_source(
        &self,
        request: &QueryRequest,
        owner_module_id: &str,
        record_type: &str,
        record_id: &str,
    ) -> Result<QueryVisibilityDecision, SdkError> {
        let mut visibility_request = request.clone();
        visibility_request.owner_module_id =
            ModuleId::try_new(owner_module_id).map_err(config_error)?;
        let resource = RecordRef {
            record_type: RecordType::try_new(record_type).map_err(config_error)?,
            record_id: RecordId::try_new(record_id).map_err(config_error)?,
        };
        self.visibility
            .authorize_visibility(&visibility_request, &resource)
            .await
    }

    async fn load_projection_snapshot(
        &self,
        request: &QueryRequest,
        root_party_id: &str,
    ) -> Result<LoadedProjection, SdkError> {
        let query_limit = i64::try_from(MAXIMUM_CONTRIBUTIONS_PER_ROOT + 1)
            .map_err(|_| projection_corrupt("Customer 360 contribution limit overflow"))?;
        let mut transaction = self.store.pool().begin().await.map_err(database_error)?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *transaction)
            .await
            .map_err(database_error)?;
        sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
            .bind(request.context.tenant_id.as_str())
            .execute(&mut *transaction)
            .await
            .map_err(database_error)?;

        let values = sqlx::query_scalar::<_, Value>(
            r#"
            SELECT document
            FROM crm.projection_documents
            WHERE tenant_id = $1
              AND projection_id = $2
              AND resource_type = $3
              AND jsonb_typeof(document -> 'root_party_ids') = 'array'
              AND (document -> 'root_party_ids') @> jsonb_build_array($4::text)
            ORDER BY resource_id ASC
            LIMIT $5
            "#,
        )
        .bind(request.context.tenant_id.as_str())
        .bind(CUSTOMER_360_PROJECTION_ID)
        .bind(CUSTOMER_360_CONTRIBUTION_RESOURCE_TYPE)
        .bind(root_party_id)
        .bind(query_limit)
        .fetch_all(&mut *transaction)
        .await
        .map_err(database_error)?;
        if values.len() > MAXIMUM_CONTRIBUTIONS_PER_ROOT {
            return Err(contribution_limit_exceeded());
        }

        let checkpoint = sqlx::query(
            r#"
            SELECT
              ((EXTRACT(EPOCH FROM last_occurred_at) * 1000000)::bigint * 1000)
                AS last_occurred_at_unix_nanos,
              last_event_id,
              applied_event_count
            FROM crm.projection_checkpoints
            WHERE tenant_id = $1 AND projection_id = $2
            "#,
        )
        .bind(request.context.tenant_id.as_str())
        .bind(CUSTOMER_360_PROJECTION_ID)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_error)?;
        transaction.commit().await.map_err(database_error)?;

        let checkpoint = checkpoint.ok_or_else(resource_not_found)?;
        let last_event_id: String = checkpoint
            .try_get("last_event_id")
            .map_err(|error| projection_corrupt(error.to_string()))?;
        let applied_event_count: i64 = checkpoint
            .try_get("applied_event_count")
            .map_err(|error| projection_corrupt(error.to_string()))?;
        let last_event_occurred_at_unix_nanos: i64 = checkpoint
            .try_get("last_occurred_at_unix_nanos")
            .map_err(|error| projection_corrupt(error.to_string()))?;
        if applied_event_count < 0 || last_event_occurred_at_unix_nanos <= 0 {
            return Err(projection_corrupt(
                "Customer 360 projection checkpoint is invalid",
            ));
        }
        let applied_event_count = u64::try_from(applied_event_count)
            .map_err(|_| projection_corrupt("Customer 360 applied event count overflow"))?;
        let documents = values
            .into_iter()
            .map(|value| Customer360ContributionDocument::from_json(&value))
            .collect::<Result<Vec<_>, _>>()?;
        if documents
            .iter()
            .any(|document| !document.affects_party(root_party_id))
        {
            return Err(projection_corrupt(
                "Customer 360 projection returned a contribution outside the requested root",
            ));
        }
        Ok(LoadedProjection {
            documents,
            freshness: wire::Customer360ProjectionFreshness {
                projection_id: CUSTOMER_360_PROJECTION_ID.to_owned(),
                applied_event_count,
                last_event_id,
                last_event_occurred_at_unix_nanos,
            },
        })
    }
}

#[derive(Debug)]
struct LoadedProjection {
    documents: Vec<Customer360ContributionDocument>,
    freshness: wire::Customer360ProjectionFreshness,
}

fn validate_document_shape(document: &Customer360ContributionDocument) -> Result<(), SdkError> {
    match (&document.contribution_kind, &document.snapshot) {
        (Customer360ContributionKind::Party, Customer360ContributionSnapshot::Party(_))
            if document.source_owner_module_id == PARTIES_MODULE_ID
                && document.source_resource_type == PARTY_RECORD_TYPE
                && document.root_party_ids == vec![document.source_resource_id.clone()] =>
        {
            Ok(())
        }
        (
            Customer360ContributionKind::Account,
            Customer360ContributionSnapshot::Account(snapshot),
        ) if document.source_owner_module_id == ACCOUNTS_MODULE_ID
            && document.source_resource_type == ACCOUNT_RECORD_TYPE
            && document.root_party_ids
                == snapshot
                    .party_associations
                    .iter()
                    .map(|association| association.party_id.clone())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>() =>
        {
            Ok(())
        }
        (
            Customer360ContributionKind::ContactPoint,
            Customer360ContributionSnapshot::ContactPoint(snapshot),
        ) if document.source_owner_module_id == CONTACT_POINTS_MODULE_ID
            && document.source_resource_type == CONTACT_POINT_RECORD_TYPE
            && document.root_party_ids == vec![snapshot.party_id.clone()] =>
        {
            Ok(())
        }
        (
            Customer360ContributionKind::PartyRelationship,
            Customer360ContributionSnapshot::PartyRelationship(snapshot),
        ) if document.source_owner_module_id == PARTY_RELATIONSHIPS_MODULE_ID
            && document.source_resource_type == PARTY_RELATIONSHIP_RECORD_TYPE
            && document.root_party_ids
                == BTreeSet::from([
                    snapshot.from_party_id.clone(),
                    snapshot.to_party_id.clone(),
                ])
                .into_iter()
                .collect::<Vec<_>>() =>
        {
            Ok(())
        }
        _ => Err(projection_corrupt(
            "Customer 360 contribution kind, owner, resource type and snapshot do not agree",
        )),
    }
}

fn party_to_wire(
    document: &Customer360ContributionDocument,
    snapshot: PartyContributionSnapshot,
    visibility: &QueryVisibilityDecision,
) -> Result<parties::Party, SdkError> {
    let kind = match snapshot.kind.as_str() {
        "person" => parties::PartyKind::Person,
        "organization" => parties::PartyKind::Organization,
        _ => return Err(projection_corrupt("Customer 360 Party kind is invalid")),
    };
    Ok(parties::Party {
        party_ref: Some(customer::PartyRef {
            party_id: document.source_resource_id.clone(),
        }),
        kind: if visibility.allows_field("kind") {
            kind as i32
        } else {
            parties::PartyKind::Unspecified as i32
        },
        display_name: if visibility.allows_field("display_name") {
            snapshot.display_name
        } else {
            String::new()
        },
        resource_version: Some(resource_version(document.source_version)),
    })
}

fn account_to_wire(
    document: &Customer360ContributionDocument,
    snapshot: AccountContributionSnapshot,
    visibility: &QueryVisibilityDecision,
) -> Result<accounts::Account, SdkError> {
    let status = match snapshot.status.as_str() {
        "active" => accounts::AccountStatus::Active,
        "inactive" => accounts::AccountStatus::Inactive,
        _ => return Err(projection_corrupt("Customer 360 Account status is invalid")),
    };
    let party_associations = if visibility.allows_field("party_associations") {
        snapshot
            .party_associations
            .into_iter()
            .map(|association| {
                let role = match association.role.as_str() {
                    "primary" => accounts::AccountPartyRole::Primary,
                    "member" => accounts::AccountPartyRole::Member,
                    _ => {
                        return Err(projection_corrupt(
                            "Customer 360 Account Party role is invalid",
                        ));
                    }
                };
                Ok(accounts::AccountPartyAssociation {
                    party_ref: Some(customer::PartyRef {
                        party_id: association.party_id,
                    }),
                    role: role as i32,
                })
            })
            .collect::<Result<Vec<_>, SdkError>>()?
    } else {
        Vec::new()
    };
    Ok(accounts::Account {
        account_ref: Some(customer::AccountRef {
            account_id: document.source_resource_id.clone(),
        }),
        name: if visibility.allows_field("name") {
            snapshot.name
        } else {
            String::new()
        },
        status: if visibility.allows_field("status") {
            status as i32
        } else {
            accounts::AccountStatus::Unspecified as i32
        },
        party_associations,
        resource_version: Some(resource_version(document.source_version)),
    })
}

fn contact_point_to_wire(
    document: &Customer360ContributionDocument,
    snapshot: ContactPointContributionSnapshot,
    visibility: &QueryVisibilityDecision,
) -> Result<contact_points::ContactPoint, SdkError> {
    let kind = match snapshot.kind.as_str() {
        "email" => contact_points::ContactPointKind::Email,
        "phone" => contact_points::ContactPointKind::Phone,
        "postal" => contact_points::ContactPointKind::Postal,
        "web" => contact_points::ContactPointKind::Web,
        "messaging" => contact_points::ContactPointKind::Messaging,
        _ => {
            return Err(projection_corrupt(
                "Customer 360 Contact Point kind is invalid",
            ));
        }
    };
    let status = match snapshot.status.as_str() {
        "active" => contact_points::ContactPointStatus::Active,
        "inactive" => contact_points::ContactPointStatus::Inactive,
        _ => {
            return Err(projection_corrupt(
                "Customer 360 Contact Point status is invalid",
            ));
        }
    };
    let verification_status = match snapshot.verification_status.as_str() {
        "unverified" => contact_points::ContactPointVerificationStatus::Unverified,
        "verified" => contact_points::ContactPointVerificationStatus::Verified,
        _ => {
            return Err(projection_corrupt(
                "Customer 360 Contact Point verification status is invalid",
            ));
        }
    };
    Ok(contact_points::ContactPoint {
        contact_point_ref: Some(customer::ContactPointRef {
            contact_point_id: document.source_resource_id.clone(),
        }),
        party_ref: visibility
            .allows_field("party_ref")
            .then_some(customer::PartyRef {
                party_id: snapshot.party_id,
            }),
        kind: if visibility.allows_field("kind") {
            kind as i32
        } else {
            contact_points::ContactPointKind::Unspecified as i32
        },
        normalized_value: if visibility.allows_field("normalized_value") {
            snapshot.normalized_value
        } else {
            String::new()
        },
        display_value: if visibility.allows_field("display_value") {
            snapshot.display_value
        } else {
            String::new()
        },
        status: if visibility.allows_field("status") {
            status as i32
        } else {
            contact_points::ContactPointStatus::Unspecified as i32
        },
        preferred: visibility.allows_field("preferred") && snapshot.preferred,
        valid_from: if visibility.allows_field("validity") {
            snapshot
                .valid_from_unix_nanos
                .map(|unix_nanos| crm_proto_contracts::crm::core::v1::UnixTime { unix_nanos })
        } else {
            None
        },
        valid_until: if visibility.allows_field("validity") {
            snapshot
                .valid_until_unix_nanos
                .map(|unix_nanos| crm_proto_contracts::crm::core::v1::UnixTime { unix_nanos })
        } else {
            None
        },
        verification: visibility.allows_field("verification").then(|| {
            contact_points::ContactPointVerification {
                status: verification_status as i32,
                evidence_ref: snapshot.verification_evidence_ref,
                verified_at: snapshot
                    .verified_at_unix_nanos
                    .map(|unix_nanos| crm_proto_contracts::crm::core::v1::UnixTime { unix_nanos }),
            }
        }),
        resource_version: Some(resource_version(document.source_version)),
    })
}

fn party_relationship_to_wire(
    document: &Customer360ContributionDocument,
    snapshot: PartyRelationshipContributionSnapshot,
    visibility: &QueryVisibilityDecision,
) -> Result<party_relationships::PartyRelationship, SdkError> {
    let directionality = match snapshot.directionality.as_str() {
        "directional" => party_relationships::PartyRelationshipDirectionality::Directional,
        "reciprocal" => party_relationships::PartyRelationshipDirectionality::Reciprocal,
        _ => {
            return Err(projection_corrupt(
                "Customer 360 Party Relationship directionality is invalid",
            ));
        }
    };
    let status = match snapshot.status.as_str() {
        "active" => party_relationships::PartyRelationshipStatus::Active,
        "inactive" => party_relationships::PartyRelationshipStatus::Inactive,
        _ => {
            return Err(projection_corrupt(
                "Customer 360 Party Relationship status is invalid",
            ));
        }
    };
    Ok(party_relationships::PartyRelationship {
        party_relationship_ref: Some(customer::PartyRelationshipRef {
            party_relationship_id: document.source_resource_id.clone(),
        }),
        from_party_ref: visibility
            .allows_field("from_party_ref")
            .then_some(customer::PartyRef {
                party_id: snapshot.from_party_id,
            }),
        to_party_ref: visibility
            .allows_field("to_party_ref")
            .then_some(customer::PartyRef {
                party_id: snapshot.to_party_id,
            }),
        relationship_type: visibility.allows_field("relationship_type").then_some({
            party_relationships::PartyRelationshipType {
                code: snapshot.relationship_type_code,
                directionality: directionality as i32,
                from_role: snapshot.from_role,
                to_role: snapshot.to_role,
            }
        }),
        status: if visibility.allows_field("status") {
            status as i32
        } else {
            party_relationships::PartyRelationshipStatus::Unspecified as i32
        },
        valid_from: if visibility.allows_field("validity") {
            snapshot
                .valid_from_unix_nanos
                .map(|unix_nanos| crm_proto_contracts::crm::core::v1::UnixTime { unix_nanos })
        } else {
            None
        },
        valid_until: if visibility.allows_field("validity") {
            snapshot
                .valid_until_unix_nanos
                .map(|unix_nanos| crm_proto_contracts::crm::core::v1::UnixTime { unix_nanos })
        } else {
            None
        },
        resource_version: Some(resource_version(document.source_version)),
    })
}

fn resource_version(version: i64) -> customer::CustomerResourceVersion {
    customer::CustomerResourceVersion {
        version,
        ..Default::default()
    }
}

fn lineage_to_wire(document: &Customer360ContributionDocument) -> wire::Customer360SourceLineage {
    wire::Customer360SourceLineage {
        owner_module_id: document.source_owner_module_id.clone(),
        resource_type: document.source_resource_type.clone(),
        resource_id: document.source_resource_id.clone(),
        source_version: document.source_version,
        source_event_id: document.source_event_id.clone(),
    }
}

fn section_source_id(source: &Option<wire::Customer360SourceLineage>) -> &str {
    source
        .as_ref()
        .map(|source| source.resource_id.as_str())
        .unwrap_or("")
}

fn decode_input<M: Message + Default>(
    request: &QueryRequest,
    schema_id: &str,
) -> Result<M, SdkError> {
    let payload = &request.input;
    if payload.owner.as_str() != MODULE_ID
        || payload.schema_id.as_str() != schema_id
        || payload.schema_version.as_str() != support::CONTRACT_VERSION
        || payload.descriptor_hash != support::message_descriptor_hash(schema_id)
        || payload.data_class != DataClass::Personal
        || payload.encoding != PayloadEncoding::Protobuf
        || payload.maximum_size_bytes > support::MAX_PROTOBUF_BYTES
        || payload.validate().is_err()
    {
        return Err(SdkError::new(
            "CUSTOMER_360_QUERY_INPUT_CONTRACT_MISMATCH",
            ErrorCategory::InvalidArgument,
            false,
            "The Customer 360 query input does not match the required contract.",
        ));
    }
    M::decode(payload.bytes.as_slice()).map_err(|_| {
        SdkError::new(
            "CUSTOMER_360_QUERY_INPUT_PROTOBUF_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The Customer 360 query input is not valid Protobuf.",
        )
    })
}

fn validate_party_id(value: &str) -> Result<RecordId, SdkError> {
    RecordId::try_new(value.to_owned()).map_err(|error| {
        SdkError::invalid_argument("customer_360.party_ref.party_id", error.to_string())
    })
}

fn resource_not_found() -> SdkError {
    SdkError::new(
        "QUERY_RESOURCE_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested resource was not found.",
    )
}

fn unsupported_query() -> SdkError {
    SdkError::new(
        "CUSTOMER_360_QUERY_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The Customer 360 query capability is not configured.",
    )
}

fn contribution_limit_exceeded() -> SdkError {
    SdkError::new(
        "CUSTOMER_360_CONTRIBUTION_LIMIT_EXCEEDED",
        ErrorCategory::Unavailable,
        true,
        "The Customer 360 view is temporarily unavailable.",
    )
}

fn database_error(error: sqlx::Error) -> SdkError {
    SdkError::new(
        "CUSTOMER_360_QUERY_DATABASE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The Customer 360 view is temporarily unavailable.",
    )
    .with_internal_reference(error.to_string())
}

fn projection_corrupt(internal: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_360_PROJECTION_INVALID",
        ErrorCategory::Unavailable,
        true,
        "The Customer 360 view is temporarily unavailable.",
    )
    .with_internal_reference(internal.into())
}

fn config_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "CUSTOMER_360_QUERY_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Customer 360 query adapter is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_one_personal_read_only_customer_360_query() {
        let definition = query_capability_definition().unwrap();
        assert_eq!(definition.capability_id.as_str(), GET_CAPABILITY);
        assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
        assert!(!definition.mutation);
        assert!(!definition.requires_idempotency);
        assert_eq!(
            definition.input_contract.allowed_data_classes,
            vec![DataClass::Personal]
        );
    }

    #[test]
    fn strict_shape_rejects_owner_kind_confusion() {
        let document = Customer360ContributionDocument {
            projection_schema_version: "1".to_owned(),
            contribution_kind: Customer360ContributionKind::Party,
            root_party_ids: vec!["party-a".to_owned()],
            source_owner_module_id: ACCOUNTS_MODULE_ID.to_owned(),
            source_resource_type: ACCOUNT_RECORD_TYPE.to_owned(),
            source_resource_id: "party-a".to_owned(),
            source_version: 1,
            source_event_id: "event-a".to_owned(),
            snapshot: Customer360ContributionSnapshot::Party(PartyContributionSnapshot {
                kind: "person".to_owned(),
                display_name: "Ada".to_owned(),
            }),
        };
        assert_eq!(
            validate_document_shape(&document).unwrap_err().code,
            "CUSTOMER_360_PROJECTION_INVALID"
        );
    }
}
