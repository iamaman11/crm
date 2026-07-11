#![forbid(unsafe_code)]

use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest, PayloadContract};
use crm_core_contracts::{CalendarDate, CurrencyCode, Money};
use crm_core_data::{AuditIntent, EventEvidence, IdempotencyEvidence};
use crm_module_sdk::{
    DataClass, DomainEvent, ErrorCategory, EventType, IdentifierError, ModuleId, PayloadEncoding,
    RecordId, RecordRef, RecordSnapshot, RecordType, ResourceRef, RetentionPolicyId, SchemaId,
    SchemaVersion, SdkError, TenantId, TypedPayload,
};
use crm_proto_contracts::{FILE_DESCRIPTOR_SET, crm::core::v1 as core};
use prost::Message;
use prost_types::{DescriptorProto, FileDescriptorProto, FileDescriptorSet};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

pub const CONTRACT_VERSION: &str = "1.0.0";
pub const MAX_PROTOBUF_BYTES: u64 = 1_048_576;
pub const DEFAULT_RETENTION_POLICY_ID: &str = "standard";
const DEFAULT_IDEMPOTENCY_TTL_NANOS: i64 = 86_400_000_000_000;
const MESSAGE_DESCRIPTOR_HASH_PROFILE: &[u8] = b"crm.protobuf.message-descriptor.sha256/v1";
const EVIDENCE_ID_PROFILE: &[u8] = b"crm.persistence.evidence-id.sha256/v1";

#[derive(Debug, Clone, Copy)]
pub struct PersistedPayloadContract<'a> {
    pub owner: &'a str,
    pub schema_id: &'a str,
    pub schema_version: &'a str,
    pub descriptor_hash: [u8; 32],
    pub maximum_size_bytes: u64,
    pub retention_policy_id: &'a str,
}

static MESSAGE_DESCRIPTOR_HASHES: OnceLock<BTreeMap<String, [u8; 32]>> = OnceLock::new();

pub fn message_descriptor_hash(full_message_name: &str) -> [u8; 32] {
    *MESSAGE_DESCRIPTOR_HASHES
        .get_or_init(build_message_descriptor_hashes)
        .get(full_message_name)
        .unwrap_or_else(|| panic!("generated descriptor set is missing {full_message_name}"))
}

fn build_message_descriptor_hashes() -> BTreeMap<String, [u8; 32]> {
    let descriptor_set = FileDescriptorSet::decode(FILE_DESCRIPTOR_SET)
        .expect("generated Protobuf descriptor set must be valid");
    let files = descriptor_set
        .file
        .into_iter()
        .map(|file| {
            let name = file
                .name
                .clone()
                .expect("generated Protobuf file descriptor must have a name");
            (name, file)
        })
        .collect::<BTreeMap<_, _>>();

    let mut hashes = BTreeMap::new();
    for (file_name, file) in &files {
        let package = file.package.as_deref().unwrap_or_default();
        let mut message_names = Vec::new();
        collect_message_names(package, &file.message_type, &mut message_names);

        let mut closure = BTreeSet::new();
        collect_descriptor_closure(file_name, &files, &mut closure);
        let encoded_closure = closure
            .iter()
            .map(|name| {
                let descriptor = files
                    .get(name)
                    .expect("descriptor dependency must exist in the generated set");
                (name.as_bytes(), descriptor.encode_to_vec())
            })
            .collect::<Vec<_>>();

        for full_message_name in message_names {
            let mut hasher = Sha256::new();
            append_hash_field(&mut hasher, MESSAGE_DESCRIPTOR_HASH_PROFILE);
            append_hash_field(&mut hasher, full_message_name.as_bytes());
            for (dependency_name, encoded_descriptor) in &encoded_closure {
                append_hash_field(&mut hasher, dependency_name);
                append_hash_field(&mut hasher, encoded_descriptor);
            }
            hashes.insert(full_message_name, hasher.finalize().into());
        }
    }
    hashes
}

fn collect_message_names(prefix: &str, messages: &[DescriptorProto], output: &mut Vec<String>) {
    for message in messages {
        let name = message
            .name
            .as_deref()
            .expect("generated message descriptor must have a name");
        let full_name = if prefix.is_empty() {
            name.to_owned()
        } else {
            format!("{prefix}.{name}")
        };
        output.push(full_name.clone());
        collect_message_names(&full_name, &message.nested_type, output);
    }
}

fn collect_descriptor_closure(
    file_name: &str,
    files: &BTreeMap<String, FileDescriptorProto>,
    output: &mut BTreeSet<String>,
) {
    if !output.insert(file_name.to_owned()) {
        return;
    }
    let file = files
        .get(file_name)
        .expect("generated descriptor closure must reference an existing file");
    for dependency in &file.dependency {
        collect_descriptor_closure(dependency, files, output);
    }
}

pub fn protobuf_contract(
    owner: &str,
    schema_id: &str,
    allowed_data_classes: Vec<DataClass>,
) -> Result<PayloadContract, SdkError> {
    Ok(PayloadContract {
        owner: configured_identifier(ModuleId::try_new(owner))?,
        schema_id: configured_identifier(SchemaId::try_new(schema_id))?,
        schema_version: configured_identifier(SchemaVersion::try_new(CONTRACT_VERSION))?,
        descriptor_hash: message_descriptor_hash(schema_id),
        allowed_data_classes,
        allowed_encodings: vec![PayloadEncoding::Protobuf],
        maximum_size_bytes: MAX_PROTOBUF_BYTES,
    })
}

pub fn protobuf_payload<M: Message>(
    owner: &str,
    schema_id: &str,
    data_class: DataClass,
    message: &M,
) -> Result<TypedPayload, SdkError> {
    let bytes = message.encode_to_vec();
    if bytes.len() as u64 > MAX_PROTOBUF_BYTES {
        return Err(SdkError::new(
            "PROTOBUF_PAYLOAD_TOO_LARGE",
            ErrorCategory::InvalidArgument,
            false,
            "The encoded payload exceeds the permitted size.",
        ));
    }
    let payload = TypedPayload {
        owner: configured_identifier(ModuleId::try_new(owner))?,
        schema_id: configured_identifier(SchemaId::try_new(schema_id))?,
        schema_version: configured_identifier(SchemaVersion::try_new(CONTRACT_VERSION))?,
        descriptor_hash: message_descriptor_hash(schema_id),
        data_class,
        encoding: PayloadEncoding::Protobuf,
        maximum_size_bytes: MAX_PROTOBUF_BYTES,
        retention_policy_id: configured_identifier(RetentionPolicyId::try_new(
            DEFAULT_RETENTION_POLICY_ID,
        ))?,
        bytes,
    };
    payload.validate()?;
    Ok(payload)
}

pub fn persisted_json_payload(
    contract: PersistedPayloadContract<'_>,
    bytes: Vec<u8>,
) -> Result<TypedPayload, SdkError> {
    if bytes.len() as u64 > contract.maximum_size_bytes {
        return Err(stored_data_error("PERSISTED_AGGREGATE_TOO_LARGE"));
    }
    let payload = TypedPayload {
        owner: configured_identifier(ModuleId::try_new(contract.owner))?,
        schema_id: configured_identifier(SchemaId::try_new(contract.schema_id))?,
        schema_version: configured_identifier(SchemaVersion::try_new(contract.schema_version))?,
        descriptor_hash: contract.descriptor_hash,
        data_class: DataClass::Confidential,
        encoding: PayloadEncoding::Json,
        maximum_size_bytes: contract.maximum_size_bytes,
        retention_policy_id: configured_identifier(RetentionPolicyId::try_new(
            contract.retention_policy_id,
        ))?,
        bytes,
    };
    payload.validate()?;
    Ok(payload)
}

pub fn persisted_json_bytes<'a>(
    snapshot: &'a RecordSnapshot,
    contract: PersistedPayloadContract<'_>,
) -> Result<&'a [u8], SdkError> {
    let payload = &snapshot.payload;
    if payload.owner.as_str() != contract.owner
        || payload.schema_id.as_str() != contract.schema_id
        || payload.schema_version.as_str() != contract.schema_version
        || payload.descriptor_hash != contract.descriptor_hash
        || payload.data_class != DataClass::Confidential
        || payload.encoding != PayloadEncoding::Json
        || payload.maximum_size_bytes != contract.maximum_size_bytes
        || payload.retention_policy_id.as_str() != contract.retention_policy_id
        || payload.validate().is_err()
    {
        return Err(stored_data_error("PERSISTED_AGGREGATE_CONTRACT_INVALID"));
    }
    Ok(payload.bytes.as_slice())
}

pub fn decode_request<M: Message + Default>(
    request: &CapabilityRequest,
    owner: &str,
    schema_id: &str,
) -> Result<M, SdkError> {
    let payload = &request.input;
    if payload.owner.as_str() != owner
        || payload.schema_id.as_str() != schema_id
        || payload.schema_version.as_str() != CONTRACT_VERSION
        || payload.descriptor_hash != message_descriptor_hash(schema_id)
        || payload.data_class != DataClass::Confidential
        || payload.encoding != PayloadEncoding::Protobuf
        || payload.maximum_size_bytes > MAX_PROTOBUF_BYTES
        || payload.validate().is_err()
    {
        return Err(SdkError::new(
            "CAPABILITY_INPUT_CONTRACT_MISMATCH",
            ErrorCategory::InvalidArgument,
            false,
            "The capability input does not match the required contract.",
        ));
    }
    M::decode(payload.bytes.as_slice()).map_err(|_| {
        SdkError::new(
            "CAPABILITY_INPUT_PROTOBUF_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The capability input is not valid Protobuf.",
        )
    })
}

pub fn capability_idempotency(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<IdempotencyEvidence, SdkError> {
    let expires_at_unix_nanos = request
        .context
        .execution
        .request_started_at_unix_nanos
        .checked_add(DEFAULT_IDEMPOTENCY_TTL_NANOS)
        .ok_or_else(|| {
            SdkError::new(
                "CAPABILITY_IDEMPOTENCY_EXPIRY_INVALID",
                ErrorCategory::Internal,
                false,
                "The capability execution configuration is invalid.",
            )
        })?;
    Ok(IdempotencyEvidence {
        scope: crm_core_data::capability_idempotency_scope(definition),
        key: request.context.execution.idempotency_key.to_string(),
        request_hash: request.input_hash,
        expires_at_unix_nanos,
    })
}

pub fn record_ref(
    record_type: &str,
    record_id: &str,
    record_id_field: &'static str,
) -> Result<RecordRef, SdkError> {
    Ok(RecordRef {
        record_type: configured_identifier(RecordType::try_new(record_type))?,
        record_id: input_identifier(RecordId::try_new(record_id), record_id_field)?,
    })
}

pub struct EventSpec<'a> {
    pub event_type: &'a str,
    pub event_schema_id: &'a str,
    pub aggregate_version: i64,
    pub previous_version: Option<i64>,
}

pub fn event_evidence<M: Message>(
    request: &CapabilityRequest,
    aggregate: RecordRef,
    owner: &str,
    spec: EventSpec<'_>,
    message: &M,
) -> Result<EventEvidence, SdkError> {
    let event_id = stable_evidence_id("event", request, &aggregate, spec.aggregate_version);
    Ok(EventEvidence {
        event_id: event_id.clone(),
        event: DomainEvent {
            event_type: configured_identifier(EventType::try_new(spec.event_type))?,
            aggregate,
            expected_aggregate_version: spec.previous_version,
            deduplication_key: event_id,
            payload: protobuf_payload(
                owner,
                spec.event_schema_id,
                DataClass::Confidential,
                message,
            )?,
        },
        aggregate_version: spec.aggregate_version,
        event_sequence: spec.aggregate_version,
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })
}

pub fn audit_intent(
    request: &CapabilityRequest,
    aggregate: &RecordRef,
    aggregate_version: i64,
    operation: &str,
    result_payload: &[u8],
) -> Result<AuditIntent, SdkError> {
    let mut envelope = BTreeMap::new();
    envelope.insert(
        "actor_id",
        request.context.execution.actor_id.as_str().to_owned(),
    );
    envelope.insert("aggregate_id", aggregate.record_id.as_str().to_owned());
    envelope.insert("aggregate_type", aggregate.record_type.as_str().to_owned());
    envelope.insert("aggregate_version", aggregate_version.to_string());
    envelope.insert(
        "capability_id",
        request.context.execution.capability_id.as_str().to_owned(),
    );
    envelope.insert(
        "capability_version",
        request
            .context
            .execution
            .capability_version
            .as_str()
            .to_owned(),
    );
    envelope.insert("operation", operation.to_owned());
    envelope.insert("request_hash", hex(&request.input_hash));
    envelope.insert("result_hash", sha256_hex(result_payload));
    envelope.insert(
        "tenant_id",
        request.context.execution.tenant_id.as_str().to_owned(),
    );
    envelope.insert(
        "transaction_id",
        request
            .context
            .execution
            .business_transaction_id
            .as_str()
            .to_owned(),
    );
    let canonical_envelope = serde_json::to_vec(&envelope).map_err(|_| {
        SdkError::new(
            "AUDIT_ENVELOPE_SERIALIZATION_FAILED",
            ErrorCategory::Internal,
            false,
            "The audit evidence could not be produced.",
        )
    })?;
    Ok(AuditIntent {
        audit_record_id: stable_evidence_id("audit", request, aggregate, aggregate_version),
        canonicalization_profile: "crm.cjson/v1".to_owned(),
        canonical_envelope,
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })
}

pub fn wire_resource_to_domain(
    value: core::ResourceRef,
    tenant_id: &TenantId,
    field: &'static str,
) -> Result<ResourceRef, SdkError> {
    if value.tenant_id != tenant_id.as_str() {
        return Err(SdkError::invalid_argument(
            field,
            "resource tenant must match the execution tenant",
        ));
    }
    if value.resource_type.is_empty() || value.resource_id.is_empty() {
        return Err(SdkError::invalid_argument(
            field,
            "resource type and id are required",
        ));
    }
    if value.version.is_some_and(|version| version <= 0) {
        return Err(SdkError::invalid_argument(
            field,
            "resource version must be positive when present",
        ));
    }
    Ok(ResourceRef {
        resource_type: value.resource_type,
        resource_id: value.resource_id,
        version: value.version,
    })
}

pub fn domain_resource_to_wire(value: &ResourceRef, tenant_id: &TenantId) -> core::ResourceRef {
    core::ResourceRef {
        tenant_id: tenant_id.as_str().to_owned(),
        resource_type: value.resource_type.clone(),
        resource_id: value.resource_id.clone(),
        version: value.version,
    }
}

pub fn wire_money_to_domain(
    value: core::ExactMoney,
    field: &'static str,
) -> Result<Money, SdkError> {
    let minor_units = value.minor_units.parse::<i128>().map_err(|_| {
        SdkError::invalid_argument(
            field,
            "money minor units must be a canonical base-10 integer",
        )
    })?;
    if minor_units.to_string() != value.minor_units {
        return Err(SdkError::invalid_argument(
            field,
            "money minor units must use canonical base-10 encoding",
        ));
    }
    let currency = CurrencyCode::try_new(value.currency_code)
        .map_err(|error| SdkError::invalid_argument(field, error.to_string()))?;
    Money::new(minor_units, currency)
        .non_negative()
        .map_err(|error| SdkError::invalid_argument(field, error.to_string()))
}

pub fn domain_money_to_wire(value: &Money) -> core::ExactMoney {
    core::ExactMoney {
        minor_units: value.minor_units().to_string(),
        currency_code: value.currency().as_str().to_owned(),
    }
}

pub fn wire_date_to_domain(
    value: core::CalendarDate,
    field: &'static str,
) -> Result<CalendarDate, SdkError> {
    let month = u8::try_from(value.month)
        .map_err(|_| SdkError::invalid_argument(field, "calendar month is out of range"))?;
    let day = u8::try_from(value.day)
        .map_err(|_| SdkError::invalid_argument(field, "calendar day is out of range"))?;
    CalendarDate::try_new(value.year, month, day)
        .map_err(|error| SdkError::invalid_argument(field, error.to_string()))
}

pub fn domain_date_to_wire(value: CalendarDate) -> core::CalendarDate {
    core::CalendarDate {
        year: value.year(),
        month: u32::from(value.month()),
        day: u32::from(value.day()),
    }
}

pub fn wire_time_to_nanos(value: core::UnixTime, field: &'static str) -> Result<i64, SdkError> {
    if value.unix_nanos < 0 {
        return Err(SdkError::invalid_argument(
            field,
            "timestamp must not be negative",
        ));
    }
    Ok(value.unix_nanos)
}

pub fn nanos_to_wire_time(value: i64) -> core::UnixTime {
    core::UnixTime { unix_nanos: value }
}

pub fn input_identifier<T>(
    result: Result<T, IdentifierError>,
    field: &'static str,
) -> Result<T, SdkError> {
    result.map_err(|error| SdkError::invalid_argument(field, error.to_string()))
}

pub fn configured_identifier<T>(result: Result<T, IdentifierError>) -> Result<T, SdkError> {
    result.map_err(|error| {
        SdkError::new(
            "CAPABILITY_ADAPTER_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The capability adapter configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

pub fn stored_data_error(code: &'static str) -> SdkError {
    SdkError::new(
        code,
        ErrorCategory::Internal,
        false,
        "Stored aggregate data is invalid.",
    )
}

fn stable_evidence_id(
    kind: &str,
    request: &CapabilityRequest,
    aggregate: &RecordRef,
    aggregate_version: i64,
) -> String {
    let mut hasher = Sha256::new();
    append_hash_field(&mut hasher, EVIDENCE_ID_PROFILE);
    append_hash_field(
        &mut hasher,
        request.context.execution.tenant_id.as_str().as_bytes(),
    );
    append_hash_field(
        &mut hasher,
        request
            .context
            .execution
            .business_transaction_id
            .as_str()
            .as_bytes(),
    );
    append_hash_field(&mut hasher, kind.as_bytes());
    append_hash_field(&mut hasher, aggregate.record_type.as_str().as_bytes());
    append_hash_field(&mut hasher, aggregate.record_id.as_str().as_bytes());
    append_hash_field(&mut hasher, &aggregate_version.to_be_bytes());
    format!("{kind}-{}", hex(&hasher.finalize()))
}

fn sha256_hex(value: &[u8]) -> String {
    hex(&Sha256::digest(value))
}

fn append_hash_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn hex(value: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(value.len() * 2);
    for byte in value {
        output.push(DIGITS[usize::from(byte >> 4)] as char);
        output.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_descriptor_hash_is_message_specific_and_nonzero() {
        let deal = message_descriptor_hash("crm.sales.v1.Deal");
        let task = message_descriptor_hash("crm.activities.v1.Task");
        assert_ne!(deal, task);
        assert_ne!(deal, [0; 32]);
    }

    #[test]
    fn cross_tenant_reference_is_rejected() {
        let error = wire_resource_to_domain(
            core::ResourceRef {
                tenant_id: "tenant-b".to_owned(),
                resource_type: "sales.deal".to_owned(),
                resource_id: "deal-1".to_owned(),
                version: Some(1),
            },
            &TenantId::try_new("tenant-a").unwrap(),
            "resource",
        )
        .unwrap_err();
        assert_eq!(error.category, ErrorCategory::InvalidArgument);
    }
}
