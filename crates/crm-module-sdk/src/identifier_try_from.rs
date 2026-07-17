use crate::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    EventType, FieldName, FileId, IdentifierError, IdempotencyKey, ModuleId, RecordId, RecordType,
    RelationshipType, RequestId, RetentionPolicyId, SchemaId, SchemaVersion, StateKey, TenantId,
    TraceId, WorkflowId, WorkflowRunId,
};

macro_rules! impl_identifier_try_from_string {
    ($($identifier:ty),+ $(,)?) => {
        $(
            impl TryFrom<String> for $identifier {
                type Error = IdentifierError;

                fn try_from(value: String) -> Result<Self, Self::Error> {
                    Self::try_new(value)
                }
            }

            impl TryFrom<&str> for $identifier {
                type Error = IdentifierError;

                fn try_from(value: &str) -> Result<Self, Self::Error> {
                    Self::try_new(value)
                }
            }
        )+
    };
}

impl_identifier_try_from_string!(
    TenantId,
    ActorId,
    RequestId,
    CorrelationId,
    CausationId,
    TraceId,
    CapabilityId,
    CapabilityVersion,
    IdempotencyKey,
    BusinessTransactionId,
    ModuleId,
    RecordId,
    RecordType,
    RelationshipType,
    EventType,
    WorkflowId,
    WorkflowRunId,
    FileId,
    SchemaId,
    SchemaVersion,
    RetentionPolicyId,
    StateKey,
    FieldName,
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_try_from_preserves_identifier_validation() {
        assert_eq!(TenantId::try_from("tenant-a").unwrap().as_str(), "tenant-a");
        assert!(TenantId::try_from("").is_err());
        assert_eq!(
            IdempotencyKey::try_from("idempotency-1".to_owned())
                .unwrap()
                .as_str(),
            "idempotency-1"
        );
    }
}
