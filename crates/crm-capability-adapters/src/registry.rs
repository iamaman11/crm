use crm_capability_runtime::{
    CapabilityDefinition, CapabilityRegistryPort,
};
use crm_module_sdk::{CapabilityId, CapabilityVersion, ErrorCategory, PortFuture, SdkError};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityCatalogError {
    Empty,
    Duplicate {
        capability_id: CapabilityId,
        capability_version: CapabilityVersion,
    },
    InvalidDefinition {
        capability_id: CapabilityId,
        capability_version: CapabilityVersion,
        reason: &'static str,
    },
}

impl fmt::Display for CapabilityCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("capability catalog must not be empty"),
            Self::Duplicate {
                capability_id,
                capability_version,
            } => write!(
                formatter,
                "duplicate capability definition {capability_id}@{capability_version}"
            ),
            Self::InvalidDefinition {
                capability_id,
                capability_version,
                reason,
            } => write!(
                formatter,
                "invalid capability definition {capability_id}@{capability_version}: {reason}"
            ),
        }
    }
}

impl Error for CapabilityCatalogError {}

#[derive(Debug, Clone)]
pub struct CapabilityCatalog {
    definitions: Arc<BTreeMap<(CapabilityId, CapabilityVersion), CapabilityDefinition>>,
}

impl CapabilityCatalog {
    pub fn new(
        definitions: impl IntoIterator<Item = CapabilityDefinition>,
    ) -> Result<Self, CapabilityCatalogError> {
        let mut catalog = BTreeMap::new();
        for definition in definitions {
            validate_definition(&definition)?;
            let key = (
                definition.capability_id.clone(),
                definition.capability_version.clone(),
            );
            if catalog.insert(key.clone(), definition).is_some() {
                return Err(CapabilityCatalogError::Duplicate {
                    capability_id: key.0,
                    capability_version: key.1,
                });
            }
        }
        if catalog.is_empty() {
            return Err(CapabilityCatalogError::Empty);
        }
        Ok(Self {
            definitions: Arc::new(catalog),
        })
    }

    pub fn definition(
        &self,
        capability_id: &CapabilityId,
        capability_version: &CapabilityVersion,
    ) -> Option<&CapabilityDefinition> {
        self.definitions
            .get(&(capability_id.clone(), capability_version.clone()))
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }
}

impl CapabilityRegistryPort for CapabilityCatalog {
    fn resolve<'a>(
        &'a self,
        capability_id: &'a CapabilityId,
        capability_version: &'a CapabilityVersion,
    ) -> PortFuture<'a, Result<Option<CapabilityDefinition>, SdkError>> {
        Box::pin(async move {
            Ok(self
                .definition(capability_id, capability_version)
                .cloned())
        })
    }
}

fn validate_definition(definition: &CapabilityDefinition) -> Result<(), CapabilityCatalogError> {
    let invalid = |reason| CapabilityCatalogError::InvalidDefinition {
        capability_id: definition.capability_id.clone(),
        capability_version: definition.capability_version.clone(),
        reason,
    };

    if definition.authorization_policy_id.is_empty() {
        return Err(invalid("authorization policy must not be empty"));
    }
    if definition.input_contract.owner != definition.owner_module_id {
        return Err(invalid("input contract owner must match capability owner"));
    }
    if definition.input_contract.allowed_data_classes.is_empty()
        || definition.input_contract.allowed_encodings.is_empty()
        || definition.input_contract.maximum_size_bytes == 0
        || definition
            .input_contract
            .descriptor_hash
            .iter()
            .all(|byte| *byte == 0)
    {
        return Err(invalid("input contract is incomplete"));
    }
    if let Some(output) = &definition.output_contract {
        if output.owner != definition.owner_module_id
            || output.allowed_data_classes.is_empty()
            || output.allowed_encodings.is_empty()
            || output.maximum_size_bytes == 0
            || output.descriptor_hash.iter().all(|byte| *byte == 0)
        {
            return Err(invalid("output contract is incomplete"));
        }
    }
    if definition.requires_idempotency && !definition.mutation {
        return Err(invalid("read-only capability cannot require idempotency"));
    }
    if definition.rate_limit_policy_id.as_deref() == Some("") {
        return Err(invalid("rate-limit policy must not be empty"));
    }
    Ok(())
}

pub fn catalog_dependency_error(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CAPABILITY_CATALOG_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The capability catalog is unavailable.",
    )
    .with_internal_reference(reference)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_capability_runtime::{CapabilityRisk, PayloadContract};
    use crm_module_sdk::{
        DataClass, ModuleId, PayloadEncoding, SchemaId, SchemaVersion,
    };

    #[tokio::test]
    async fn resolves_only_exact_version() {
        let definition = definition("1.0.0");
        let catalog = CapabilityCatalog::new([definition.clone()]).unwrap();

        assert_eq!(
            catalog
                .resolve(&definition.capability_id, &definition.capability_version)
                .await
                .unwrap(),
            Some(definition.clone())
        );
        assert!(
            catalog
                .resolve(
                    &definition.capability_id,
                    &CapabilityVersion::try_new("1.1.0").unwrap()
                )
                .await
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn rejects_duplicate_coordinate() {
        let error = CapabilityCatalog::new([definition("1.0.0"), definition("1.0.0")])
            .unwrap_err();
        assert!(matches!(error, CapabilityCatalogError::Duplicate { .. }));
    }

    fn definition(version: &str) -> CapabilityDefinition {
        CapabilityDefinition {
            capability_id: CapabilityId::try_new("crm.sales.deal.create").unwrap(),
            capability_version: CapabilityVersion::try_new(version).unwrap(),
            owner_module_id: ModuleId::try_new("crm.sales").unwrap(),
            input_contract: PayloadContract {
                owner: ModuleId::try_new("crm.sales").unwrap(),
                schema_id: SchemaId::try_new("crm.sales.deal.create").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                descriptor_hash: [1; 32],
                allowed_data_classes: vec![DataClass::Internal],
                allowed_encodings: vec![PayloadEncoding::Json],
                maximum_size_bytes: 4096,
            },
            output_contract: None,
            risk: CapabilityRisk::Medium,
            mutation: true,
            requires_idempotency: true,
            requires_approval: false,
            authorization_policy_id: "sales.deal.create".to_owned(),
            rate_limit_policy_id: None,
        }
    }
}
