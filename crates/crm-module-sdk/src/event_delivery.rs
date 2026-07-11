use crate::ports::RecordRef;
use crate::types::{
    ActorId, CorrelationId, EventType, ModuleExecutionContext, ModuleId, SdkError, TenantId,
    TraceId, TypedPayload,
};
use serde::{Deserialize, Serialize};

/// Stable identity for one immutable source event.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventId(String);

impl EventId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        validated_identifier("event_delivery.event_id", value.into()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Published version of an event contract.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventVersion(String);

impl EventVersion {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        validated_identifier("event_delivery.event_version", value.into()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Stable consumer-scoped identity for delivery and retry deduplication.
///
/// The delivery runtime must reuse this identity when retrying the same source
/// event for the same consumer subscription. A different consumer may have a
/// different delivery identity for the same source event.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeliveryId(String);

impl DeliveryId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
        validated_identifier("event_delivery.delivery_id", value.into()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Immutable event envelope delivered by the platform to a governed module.
///
/// This is the inbound counterpart to [`crate::DomainEvent`]. It carries the
/// complete cross-domain lineage required by link modules without exposing a
/// broker client or another module's storage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventDelivery {
    pub delivery_id: DeliveryId,
    pub event_id: EventId,
    pub tenant_id: TenantId,
    pub source_module_id: ModuleId,
    pub consumer_module_id: ModuleId,
    pub source_actor_id: ActorId,
    pub event_type: EventType,
    pub event_version: EventVersion,
    pub aggregate: RecordRef,
    pub aggregate_version: i64,
    pub occurred_at_unix_nanos: i64,
    pub correlation_id: CorrelationId,
    pub trace_id: TraceId,
    pub payload: TypedPayload,
}

impl EventDelivery {
    pub fn validate(&self) -> Result<(), SdkError> {
        if self.aggregate_version <= 0 {
            return Err(SdkError::invalid_argument(
                "event_delivery.aggregate_version",
                "aggregate version must be greater than zero",
            ));
        }
        if self.occurred_at_unix_nanos < 0 {
            return Err(SdkError::invalid_argument(
                "event_delivery.occurred_at_unix_nanos",
                "event occurrence time must not be negative",
            ));
        }
        self.payload.validate()?;
        if self.payload.owner != self.source_module_id {
            return Err(SdkError::invalid_argument(
                "event_delivery.payload.owner",
                "payload owner must match the source module",
            ));
        }
        Ok(())
    }

    /// Validates the immutable delivery plus the host-bound module execution
    /// context used to process it.
    ///
    /// Tenant, consumer module, correlation and trace identities must remain
    /// bound across delivery. The host may use a service principal as the
    /// processing actor, so the execution actor is intentionally not required
    /// to equal the source actor.
    pub fn validate_for_consumer(
        &self,
        context: &ModuleExecutionContext,
    ) -> Result<(), SdkError> {
        self.validate()?;
        context.validate()?;
        if context.module_id != self.consumer_module_id {
            return Err(SdkError::invalid_argument(
                "event_delivery.consumer_module_id",
                "delivery consumer must match the executing module",
            ));
        }
        if context.execution.tenant_id != self.tenant_id {
            return Err(SdkError::invalid_argument(
                "event_delivery.tenant_id",
                "delivery tenant must match the execution tenant",
            ));
        }
        if context.execution.correlation_id != self.correlation_id {
            return Err(SdkError::invalid_argument(
                "event_delivery.correlation_id",
                "delivery correlation identity must be preserved",
            ));
        }
        if context.execution.trace_id != self.trace_id {
            return Err(SdkError::invalid_argument(
                "event_delivery.trace_id",
                "delivery trace identity must be preserved",
            ));
        }
        Ok(())
    }
}

fn validated_identifier(field: &'static str, value: String) -> Result<String, SdkError> {
    if value.is_empty() {
        return Err(SdkError::invalid_argument(field, "identifier must not be empty"));
    }
    if value.len() > crate::MAX_IDENTIFIER_BYTES {
        return Err(SdkError::invalid_argument(
            field,
            format!(
                "identifier must not exceed {} bytes",
                crate::MAX_IDENTIFIER_BYTES
            ),
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(SdkError::invalid_argument(
            field,
            "identifier must not contain control characters",
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, DataClass,
        ExecutionContext, IdempotencyKey, PayloadEncoding, RecordId, RecordType, RequestId,
        RetentionPolicyId, SchemaId, SchemaVersion,
    };

    fn delivery() -> EventDelivery {
        EventDelivery {
            delivery_id: DeliveryId::try_new("delivery-1").unwrap(),
            event_id: EventId::try_new("event-1").unwrap(),
            tenant_id: TenantId::try_new("tenant-1").unwrap(),
            source_module_id: ModuleId::try_new("crm.sales").unwrap(),
            consumer_module_id: ModuleId::try_new("crm.sales-activities-link").unwrap(),
            source_actor_id: ActorId::try_new("actor-1").unwrap(),
            event_type: EventType::try_new("sales.deal.stage_changed").unwrap(),
            event_version: EventVersion::try_new("1.0.0").unwrap(),
            aggregate: RecordRef {
                record_type: RecordType::try_new("sales.deal").unwrap(),
                record_id: RecordId::try_new("deal-1").unwrap(),
            },
            aggregate_version: 2,
            occurred_at_unix_nanos: 100,
            correlation_id: CorrelationId::try_new("correlation-1").unwrap(),
            trace_id: TraceId::try_new("trace-1").unwrap(),
            payload: TypedPayload {
                owner: ModuleId::try_new("crm.sales").unwrap(),
                schema_id: SchemaId::try_new("crm.sales.v1.DealStageChangedEvent").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                descriptor_hash: [1; 32],
                data_class: DataClass::Confidential,
                encoding: PayloadEncoding::Protobuf,
                maximum_size_bytes: 1_024,
                retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
                bytes: vec![1],
            },
        }
    }

    fn consumer_context() -> ModuleExecutionContext {
        ModuleExecutionContext {
            module_id: ModuleId::try_new("crm.sales-activities-link").unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new("tenant-1").unwrap(),
                actor_id: ActorId::try_new("link-service").unwrap(),
                request_id: RequestId::try_new("request-1").unwrap(),
                correlation_id: CorrelationId::try_new("correlation-1").unwrap(),
                causation_id: CausationId::try_new("event-1").unwrap(),
                trace_id: TraceId::try_new("trace-1").unwrap(),
                capability_id: CapabilityId::try_new("link.sales.stage_changed.process").unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                idempotency_key: IdempotencyKey::try_new("delivery-1").unwrap(),
                business_transaction_id: BusinessTransactionId::try_new("transaction-1").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: 101,
            },
        }
    }

    #[test]
    fn accepts_tenant_consumer_and_lineage_bound_delivery() {
        delivery()
            .validate_for_consumer(&consumer_context())
            .expect("valid delivery must be accepted");
    }

    #[test]
    fn rejects_payload_owned_by_a_different_source_module() {
        let mut delivery = delivery();
        delivery.payload.owner = ModuleId::try_new("crm.activities").unwrap();

        let error = delivery.validate().expect_err("owner mismatch must fail");
        assert_eq!(error.category, crate::ErrorCategory::InvalidArgument);
    }

    #[test]
    fn rejects_cross_tenant_consumer_context() {
        let mut context = consumer_context();
        context.execution.tenant_id = TenantId::try_new("tenant-2").unwrap();

        let error = delivery()
            .validate_for_consumer(&context)
            .expect_err("cross-tenant delivery must fail");
        assert_eq!(error.category, crate::ErrorCategory::InvalidArgument);
    }

    #[test]
    fn rejects_trace_lineage_rebinding() {
        let mut context = consumer_context();
        context.execution.trace_id = TraceId::try_new("trace-2").unwrap();

        delivery()
            .validate_for_consumer(&context)
            .expect_err("trace rebinding must fail");
    }
}