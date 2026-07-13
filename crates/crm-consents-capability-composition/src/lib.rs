#![forbid(unsafe_code)]

//! Application composition for authoritative Consent mutations.
//!
//! The pure `crm-consents` owner never reads Party or Contact Point storage.
//! This composition boundary resolves same-tenant references before delegating
//! to the transactional owner executor, so invalid references cannot create
//! Consent records, audit entries, outbox events or idempotency evidence.

use crm_capability_runtime::{
    CapabilityDefinition, CapabilityExecutionResult, CapabilityRequest, TransactionalCapabilityExecutor,
};
use crm_consents::{CommunicationChannel, PartyReference};
use crm_consents_capability_adapter::{
    CREATE_CAPABILITY, MUTATION_CAPABILITY_IDS, CreateConsentReferenceScope,
    referenced_scope_from_create,
};
use crm_contact_points::ContactPointKind;
use crm_contact_points_capability_adapter::{
    MODULE_ID as CONTACT_POINTS_MODULE_ID, RECORD_TYPE as CONTACT_POINT_RECORD_TYPE,
    contact_point_from_snapshot,
};
use crm_core_data::{PostgresDataStore, RecordGetQuery};
use crm_module_sdk::{
    ErrorCategory, ModuleId, PortFuture, RecordId, RecordType, SdkError, TenantId,
};
use crm_parties_capability_adapter::{
    MODULE_ID as PARTIES_MODULE_ID, RECORD_TYPE as PARTY_RECORD_TYPE,
};
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsentContactPointFacts {
    pub party_id: String,
    pub kind: ContactPointKind,
}

pub trait ConsentReferenceReader: Send + Sync {
    fn party_exists<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        party_ref: &'a PartyReference,
    ) -> PortFuture<'a, Result<bool, SdkError>>;

    fn contact_point<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        contact_point_id: &'a str,
    ) -> PortFuture<'a, Result<Option<ConsentContactPointFacts>, SdkError>>;
}

#[derive(Debug, Clone)]
pub struct PostgresConsentReferenceReader {
    store: PostgresDataStore,
}

impl PostgresConsentReferenceReader {
    pub fn new(store: PostgresDataStore) -> Self {
        Self { store }
    }
}

impl ConsentReferenceReader for PostgresConsentReferenceReader {
    fn party_exists<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        party_ref: &'a PartyReference,
    ) -> PortFuture<'a, Result<bool, SdkError>> {
        Box::pin(async move {
            let owner_module_id = ModuleId::try_new(PARTIES_MODULE_ID).map_err(config_error)?;
            let record_type = RecordType::try_new(PARTY_RECORD_TYPE).map_err(config_error)?;
            let record_id = RecordId::try_new(party_ref.as_str()).map_err(config_error)?;
            Ok(self
                .store
                .get_record_for_query(&RecordGetQuery {
                    tenant_id: tenant_id.clone(),
                    owner_module_id,
                    record_type,
                    record_id,
                })
                .await?
                .is_some())
        })
    }

    fn contact_point<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        contact_point_id: &'a str,
    ) -> PortFuture<'a, Result<Option<ConsentContactPointFacts>, SdkError>> {
        Box::pin(async move {
            let owner_module_id =
                ModuleId::try_new(CONTACT_POINTS_MODULE_ID).map_err(config_error)?;
            let record_type =
                RecordType::try_new(CONTACT_POINT_RECORD_TYPE).map_err(config_error)?;
            let record_id = RecordId::try_new(contact_point_id).map_err(config_error)?;
            let Some(snapshot) = self
                .store
                .get_record_for_query(&RecordGetQuery {
                    tenant_id: tenant_id.clone(),
                    owner_module_id,
                    record_type,
                    record_id,
                })
                .await?
            else {
                return Ok(None);
            };
            let contact_point = contact_point_from_snapshot(&snapshot)?;
            Ok(Some(ConsentContactPointFacts {
                party_id: contact_point.party_ref().as_str().to_owned(),
                kind: contact_point.kind(),
            }))
        })
    }
}

#[derive(Clone)]
pub struct ConsentCapabilityExecutor {
    references: Arc<dyn ConsentReferenceReader>,
    inner: Arc<dyn TransactionalCapabilityExecutor>,
}

impl fmt::Debug for ConsentCapabilityExecutor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConsentCapabilityExecutor")
            .field("references", &"dyn ConsentReferenceReader")
            .field("inner", &"dyn TransactionalCapabilityExecutor")
            .finish()
    }
}

impl ConsentCapabilityExecutor {
    pub fn new(
        references: Arc<dyn ConsentReferenceReader>,
        inner: Arc<dyn TransactionalCapabilityExecutor>,
    ) -> Self {
        Self { references, inner }
    }
}

impl TransactionalCapabilityExecutor for ConsentCapabilityExecutor {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: CapabilityRequest,
    ) -> PortFuture<'a, Result<CapabilityExecutionResult, SdkError>> {
        if !MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {
            return Box::pin(async { Err(unsupported_capability()) });
        }
        Box::pin(async move {
            if definition.capability_id.as_str() == CREATE_CAPABILITY {
                let scope = referenced_scope_from_create(&request)?;
                validate_reference_scope(
                    self.references.as_ref(),
                    &request.context.execution.tenant_id,
                    &scope,
                )
                .await?;
            }
            self.inner.execute(definition, request).await
        })
    }
}

pub async fn validate_reference_scope(
    reader: &dyn ConsentReferenceReader,
    tenant_id: &TenantId,
    scope: &CreateConsentReferenceScope,
) -> Result<(), SdkError> {
    if !reader.party_exists(tenant_id, &scope.party_ref).await? {
        return Err(reference_unavailable());
    }

    let Some(contact_point_ref) = scope.contact_point_ref.as_ref() else {
        return Ok(());
    };
    let Some(contact_point) = reader
        .contact_point(tenant_id, contact_point_ref.as_str())
        .await?
    else {
        return Err(reference_unavailable());
    };
    if contact_point.party_id != scope.party_ref.as_str()
        || !channel_is_compatible(scope.channel, contact_point.kind)
    {
        return Err(reference_unavailable());
    }
    Ok(())
}

pub const fn channel_is_compatible(
    channel: CommunicationChannel,
    kind: ContactPointKind,
) -> bool {
    match channel {
        CommunicationChannel::Email => matches!(kind, ContactPointKind::Email),
        CommunicationChannel::Phone | CommunicationChannel::Sms => {
            matches!(kind, ContactPointKind::Phone)
        }
        CommunicationChannel::Postal => matches!(kind, ContactPointKind::Postal),
        CommunicationChannel::Messaging => matches!(kind, ContactPointKind::Messaging),
        CommunicationChannel::Push => true,
    }
}

fn reference_unavailable() -> SdkError {
    SdkError::new(
        "CONSENTS_REFERENCE_UNAVAILABLE",
        ErrorCategory::InvalidArgument,
        false,
        "One or more referenced Consent resources are unavailable.",
    )
}

fn unsupported_capability() -> SdkError {
    SdkError::new(
        "CONSENTS_COMPOSITION_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The Consent mutation capability is not configured for this composition boundary.",
    )
}

fn config_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "CONSENTS_REFERENCE_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Consent reference validation boundary is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_consents::ContactPointReference;
    use std::sync::Mutex;

    #[derive(Debug)]
    struct FakeReader {
        party_exists: Result<bool, &'static str>,
        contact_point: Result<Option<ConsentContactPointFacts>, &'static str>,
        seen_tenants: Mutex<Vec<String>>,
    }

    impl FakeReader {
        fn available(contact_point: Option<ConsentContactPointFacts>) -> Self {
            Self {
                party_exists: Ok(true),
                contact_point: Ok(contact_point),
                seen_tenants: Mutex::new(Vec::new()),
            }
        }
    }

    impl ConsentReferenceReader for FakeReader {
        fn party_exists<'a>(
            &'a self,
            tenant_id: &'a TenantId,
            _party_ref: &'a PartyReference,
        ) -> PortFuture<'a, Result<bool, SdkError>> {
            Box::pin(async move {
                self.seen_tenants
                    .lock()
                    .unwrap()
                    .push(tenant_id.as_str().to_owned());
                self.party_exists.map_err(fake_dependency_error)
            })
        }

        fn contact_point<'a>(
            &'a self,
            tenant_id: &'a TenantId,
            _contact_point_id: &'a str,
        ) -> PortFuture<'a, Result<Option<ConsentContactPointFacts>, SdkError>> {
            Box::pin(async move {
                self.seen_tenants
                    .lock()
                    .unwrap()
                    .push(tenant_id.as_str().to_owned());
                self.contact_point.clone().map_err(fake_dependency_error)
            })
        }
    }

    fn scope(
        channel: CommunicationChannel,
        contact_point_id: Option<&str>,
    ) -> CreateConsentReferenceScope {
        CreateConsentReferenceScope {
            party_ref: PartyReference::try_new("party-1").unwrap(),
            contact_point_ref: contact_point_id
                .map(ContactPointReference::try_new)
                .transpose()
                .unwrap(),
            channel,
        }
    }

    fn tenant() -> TenantId {
        TenantId::try_new("tenant-1").unwrap()
    }

    fn facts(party_id: &str, kind: ContactPointKind) -> ConsentContactPointFacts {
        ConsentContactPointFacts {
            party_id: party_id.to_owned(),
            kind,
        }
    }

    fn fake_dependency_error(message: &'static str) -> SdkError {
        SdkError::new(
            "FAKE_REFERENCE_DEPENDENCY_FAILURE",
            ErrorCategory::Unavailable,
            true,
            "The reference service is unavailable.",
        )
        .with_internal_reference(message)
    }

    #[tokio::test]
    async fn party_wide_scope_requires_only_same_tenant_party_existence() {
        let reader = FakeReader::available(None);
        validate_reference_scope(&reader, &tenant(), &scope(CommunicationChannel::Email, None))
            .await
            .unwrap();
        assert_eq!(reader.seen_tenants.lock().unwrap().as_slice(), ["tenant-1"]);
    }

    #[tokio::test]
    async fn missing_party_and_missing_contact_point_share_the_same_safe_failure() {
        let missing_party = FakeReader {
            party_exists: Ok(false),
            contact_point: Ok(None),
            seen_tenants: Mutex::new(Vec::new()),
        };
        let party_error = validate_reference_scope(
            &missing_party,
            &tenant(),
            &scope(CommunicationChannel::Email, None),
        )
        .await
        .unwrap_err();

        let missing_contact = FakeReader::available(None);
        let contact_error = validate_reference_scope(
            &missing_contact,
            &tenant(),
            &scope(CommunicationChannel::Email, Some("contact-point-1")),
        )
        .await
        .unwrap_err();
        assert_eq!(party_error.code, "CONSENTS_REFERENCE_UNAVAILABLE");
        assert_eq!(contact_error.code, party_error.code);
        assert_eq!(contact_error.safe_message, party_error.safe_message);
    }

    #[tokio::test]
    async fn ownership_mismatch_and_channel_mismatch_fail_closed() {
        for contact_point in [
            facts("party-other", ContactPointKind::Email),
            facts("party-1", ContactPointKind::Phone),
        ] {
            let reader = FakeReader::available(Some(contact_point));
            let error = validate_reference_scope(
                &reader,
                &tenant(),
                &scope(CommunicationChannel::Email, Some("contact-point-1")),
            )
            .await
            .unwrap_err();
            assert_eq!(error.code, "CONSENTS_REFERENCE_UNAVAILABLE");
        }
    }

    #[tokio::test]
    async fn deterministic_channel_mapping_accepts_sms_on_phone_and_messaging_on_messaging() {
        for (channel, kind) in [
            (CommunicationChannel::Sms, ContactPointKind::Phone),
            (
                CommunicationChannel::Messaging,
                ContactPointKind::Messaging,
            ),
        ] {
            let reader = FakeReader::available(Some(facts("party-1", kind)));
            validate_reference_scope(
                &reader,
                &tenant(),
                &scope(channel, Some("contact-point-1")),
            )
            .await
            .unwrap();
        }
    }

    #[tokio::test]
    async fn dependency_failures_are_propagated_not_masked_as_missing_references() {
        let reader = FakeReader {
            party_exists: Err("database offline"),
            contact_point: Ok(None),
            seen_tenants: Mutex::new(Vec::new()),
        };
        let error = validate_reference_scope(
            &reader,
            &tenant(),
            &scope(CommunicationChannel::Email, None),
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, "FAKE_REFERENCE_DEPENDENCY_FAILURE");
    }

    #[test]
    fn push_has_no_false_contact_point_kind_mapping_until_a_push_endpoint_kind_exists() {
        for kind in [
            ContactPointKind::Email,
            ContactPointKind::Phone,
            ContactPointKind::Postal,
            ContactPointKind::Web,
            ContactPointKind::Messaging,
        ] {
            assert!(channel_is_compatible(CommunicationChannel::Push, kind));
        }
    }
}
