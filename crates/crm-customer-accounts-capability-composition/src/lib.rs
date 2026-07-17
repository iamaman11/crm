#![forbid(unsafe_code)]

//! Pre-authorization application validation for Account Party references.

use crm_capability_runtime::{
    CapabilityDefinition, CapabilityRequest, CapabilitySemanticValidator,
};
use crm_customer_accounts_capability_adapter::{
    CREATE_CAPABILITY, MUTATION_CAPABILITY_IDS, UPDATE_CAPABILITY,
    referenced_party_ids_from_create, referenced_party_ids_from_update,
};
use crm_module_sdk::{ErrorCategory, PortFuture, SdkError};
use crm_party_reference_composition::PartyReferenceReader;
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub struct AccountPartyReferenceSemanticValidator {
    parties: Arc<dyn PartyReferenceReader>,
}

impl fmt::Debug for AccountPartyReferenceSemanticValidator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AccountPartyReferenceSemanticValidator")
            .field("parties", &"dyn PartyReferenceReader")
            .finish()
    }
}

impl AccountPartyReferenceSemanticValidator {
    pub fn new(parties: Arc<dyn PartyReferenceReader>) -> Self {
        Self { parties }
    }
}

impl CapabilitySemanticValidator for AccountPartyReferenceSemanticValidator {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        let references = match definition.capability_id.as_str() {
            CREATE_CAPABILITY => referenced_party_ids_from_create(request),
            UPDATE_CAPABILITY => referenced_party_ids_from_update(request),
            value if MUTATION_CAPABILITY_IDS.contains(&value) => Err(configuration_error()),
            _ => Err(unsupported_capability()),
        };
        Box::pin(async move {
            let references = references?;
            let unique = references
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
        "CUSTOMER_ACCOUNTS_PARTY_REFERENCE_UNAVAILABLE",
        ErrorCategory::InvalidArgument,
        false,
        "One or more referenced Parties are unavailable.",
    )
}

fn configuration_error() -> SdkError {
    SdkError::new(
        "CUSTOMER_ACCOUNTS_REFERENCE_VALIDATION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Account reference validation configuration is invalid.",
    )
}

fn unsupported_capability() -> SdkError {
    SdkError::new(
        "CUSTOMER_ACCOUNTS_COMPOSITION_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The Account mutation capability is not configured for this composition boundary.",
    )
}

pub const CRATE_NAME: &str = "crm-customer-accounts-capability-composition";
