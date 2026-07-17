#![forbid(unsafe_code)]

//! Pre-authorization application validation for Contact Point Party references.

use crm_capability_runtime::{
    CapabilityDefinition, CapabilityRequest, CapabilitySemanticValidator,
};
use crm_contact_points_capability_adapter::{
    CREATE_CAPABILITY, MUTATION_CAPABILITY_IDS, referenced_party_id_from_create,
};
use crm_module_sdk::{ErrorCategory, PortFuture, SdkError};
use crm_party_reference_composition::PartyReferenceReader;
use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub struct ContactPointPartyReferenceSemanticValidator {
    parties: Arc<dyn PartyReferenceReader>,
}

impl fmt::Debug for ContactPointPartyReferenceSemanticValidator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ContactPointPartyReferenceSemanticValidator")
            .field("parties", &"dyn PartyReferenceReader")
            .finish()
    }
}

impl ContactPointPartyReferenceSemanticValidator {
    pub fn new(parties: Arc<dyn PartyReferenceReader>) -> Self {
        Self { parties }
    }
}

impl CapabilitySemanticValidator for ContactPointPartyReferenceSemanticValidator {
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
        let reference = referenced_party_id_from_create(request);
        Box::pin(async move {
            let reference = reference?;
            if self
                .parties
                .exists(&request.context.execution.tenant_id, reference.as_str())
                .await?
            {
                Ok(())
            } else {
                Err(reference_unavailable())
            }
        })
    }
}

fn reference_unavailable() -> SdkError {
    SdkError::new(
        "CONTACT_POINTS_PARTY_REFERENCE_UNAVAILABLE",
        ErrorCategory::InvalidArgument,
        false,
        "The referenced Party is unavailable.",
    )
}

fn unsupported_capability() -> SdkError {
    SdkError::new(
        "CONTACT_POINTS_COMPOSITION_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The Contact Point mutation capability is not configured for this composition boundary.",
    )
}

pub const CRATE_NAME: &str = "crm-contact-points-capability-composition";
