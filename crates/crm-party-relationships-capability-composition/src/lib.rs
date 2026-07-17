#![forbid(unsafe_code)]

//! Pre-authorization application validation for Party Relationship endpoints.

use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest, CapabilitySemanticValidator};
use crm_module_sdk::{ErrorCategory, PortFuture, SdkError};
use crm_party_reference_composition::PartyReferenceReader;
use crm_party_relationships_capability_adapter::{
    CREATE_CAPABILITY, MUTATION_CAPABILITY_IDS, referenced_party_ids_from_create,
};
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub struct PartyRelationshipReferenceSemanticValidator {
    parties: Arc<dyn PartyReferenceReader>,
}

impl fmt::Debug for PartyRelationshipReferenceSemanticValidator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PartyRelationshipReferenceSemanticValidator")
            .field("parties", &"dyn PartyReferenceReader")
            .finish()
    }
}

impl PartyRelationshipReferenceSemanticValidator {
    pub fn new(parties: Arc<dyn PartyReferenceReader>) -> Self {
        Self { parties }
    }
}

impl CapabilitySemanticValidator for PartyRelationshipReferenceSemanticValidator {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        if !MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {
            return Box::pin(async { Err(unsupported_capability()) });
        }
        if definition.capability_id.as_str() != CREATE_CAPABILITY {
            return Box::pin(async { Ok(()) });
        }
        let references = referenced_party_ids_from_create(request);
        Box::pin(async move {
            let unique = references?
                .into_iter()
                .map(|reference| reference.as_str().to_owned())
                .collect::<BTreeSet<_>>();
            for party_id in unique {
                if !self
                    .parties
                    .exists(&request.context.execution.tenant_id, &party_id)
                    .await?
                {
                    return Err(reference_unavailable());
                }
            }
            Ok(())
        })
    }
}

fn reference_unavailable() -> SdkError {
    SdkError::new(
        "PARTY_RELATIONSHIPS_PARTY_REFERENCE_UNAVAILABLE",
        ErrorCategory::InvalidArgument,
        false,
        "One or more referenced Parties are unavailable.",
    )
}

fn unsupported_capability() -> SdkError {
    SdkError::new(
        "PARTY_RELATIONSHIPS_COMPOSITION_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The Party Relationship mutation capability is not configured for this composition boundary.",
    )
}

pub const CRATE_NAME: &str = "crm-party-relationships-capability-composition";
