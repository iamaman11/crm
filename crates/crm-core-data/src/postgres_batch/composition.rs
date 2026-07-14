/// A mutation fragment produced by exactly one authoritative owner module.
///
/// Fragments deliberately do not carry idempotency or audit evidence. Those
/// belong to the single governed business operation that composes the owner
/// fragments into one atomic transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerMutationFragment {
    pub owner_module_id: ModuleId,
    pub records: Vec<RecordMutation>,
    pub relationships: Vec<RelationshipMutation>,
    pub events: Vec<EventEvidence>,
}

impl OwnerMutationFragment {
    pub fn validate(&self) -> Result<(), BatchError> {
        if self.records.is_empty() && self.relationships.is_empty() {
            return Err(BatchError::InvalidPlan(format!(
                "owner fragment {} requires at least one record or relationship mutation",
                self.owner_module_id
            )));
        }
        if self.events.is_empty() {
            return Err(BatchError::InvalidPlan(format!(
                "owner fragment {} requires at least one owner outbox event",
                self.owner_module_id
            )));
        }

        let mut record_keys = BTreeSet::new();
        for mutation in &self.records {
            mutation.payload().validate().map_err(BatchError::Sdk)?;
            if mutation.payload().owner != self.owner_module_id {
                return Err(BatchError::InvalidPlan(format!(
                    "record {} payload owner does not match owner fragment {}",
                    mutation.reference().record_id,
                    self.owner_module_id
                )));
            }
            if matches!(
                mutation,
                RecordMutation::Update {
                    expected_version,
                    ..
                } if *expected_version <= 0
            ) {
                return Err(BatchError::InvalidPlan(
                    "record update expected_version must be positive".to_owned(),
                ));
            }
            let key = record_mutation_key(mutation);
            if !record_keys.insert(key.clone()) {
                return Err(BatchError::InvalidPlan(format!(
                    "record {key} is mutated more than once in owner fragment {}",
                    self.owner_module_id
                )));
            }
        }

        let mut relationship_keys = BTreeSet::new();
        for mutation in &self.relationships {
            if let RelationshipMutation::Link { payload, .. } = mutation {
                payload.validate().map_err(BatchError::Sdk)?;
                if payload.owner != self.owner_module_id {
                    return Err(BatchError::InvalidPlan(format!(
                        "relationship payload owner does not match owner fragment {}",
                        self.owner_module_id
                    )));
                }
            }
            let key = relationship_key(mutation.relationship());
            if !relationship_keys.insert(key.clone()) {
                return Err(BatchError::InvalidPlan(format!(
                    "relationship {key} is mutated more than once in owner fragment {}",
                    self.owner_module_id
                )));
            }
        }

        let mut event_ids = BTreeSet::new();
        let mut deduplication_keys = BTreeSet::new();
        for evidence in &self.events {
            evidence.event.payload.validate().map_err(BatchError::Sdk)?;
            if evidence.event.payload.owner != self.owner_module_id {
                return Err(BatchError::InvalidPlan(format!(
                    "event payload owner does not match owner fragment {}",
                    self.owner_module_id
                )));
            }
            validate_event_evidence(evidence)?;
            if !event_ids.insert(evidence.event_id.clone()) {
                return Err(BatchError::InvalidPlan(format!(
                    "duplicate event id {} in owner fragment {}",
                    evidence.event_id, self.owner_module_id
                )));
            }
            let deduplication_key = event_deduplication_identity(evidence);
            if !deduplication_keys.insert(deduplication_key.clone()) {
                return Err(BatchError::InvalidPlan(format!(
                    "duplicate event deduplication identity {deduplication_key} in owner fragment {}",
                    self.owner_module_id
                )));
            }
        }

        Ok(())
    }
}

/// One governed business mutation composed from multiple authoritative owner
/// fragments and committed atomically by the platform.
///
/// The execution context names the public capability/composition owner. It does
/// not grant that module authority to manufacture another owner's payloads:
/// every nested fragment is independently owner-bound and validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedBatchMutationPlan {
    pub context: ModuleExecutionContext,
    pub owner_fragments: Vec<OwnerMutationFragment>,
    pub idempotency: IdempotencyEvidence,
    pub audits: Vec<AuditIntent>,
}

impl ComposedBatchMutationPlan {
    pub fn validate(&self) -> Result<(), BatchError> {
        self.context.validate().map_err(BatchError::Sdk)?;
        if self.owner_fragments.len() < 2 {
            return Err(BatchError::InvalidPlan(
                "a composed batch requires at least two authoritative owner fragments".to_owned(),
            ));
        }
        validate_composed_idempotency(&self.context, &self.idempotency)?;
        validate_composed_audits(&self.audits)?;

        let mut owners = BTreeSet::new();
        let mut record_keys = BTreeSet::new();
        let mut relationship_keys = BTreeSet::new();
        let mut event_ids = BTreeSet::new();
        let mut deduplication_keys = BTreeSet::new();

        for fragment in &self.owner_fragments {
            fragment.validate()?;
            if !owners.insert(fragment.owner_module_id.as_str().to_owned()) {
                return Err(BatchError::InvalidPlan(format!(
                    "owner fragment {} is present more than once",
                    fragment.owner_module_id
                )));
            }

            for mutation in &fragment.records {
                let key = record_mutation_key(mutation);
                if !record_keys.insert(key.clone()) {
                    return Err(BatchError::InvalidPlan(format!(
                        "record {key} is mutated by more than one owner fragment"
                    )));
                }
            }
            for mutation in &fragment.relationships {
                let key = relationship_key(mutation.relationship());
                if !relationship_keys.insert(key.clone()) {
                    return Err(BatchError::InvalidPlan(format!(
                        "relationship {key} is mutated by more than one owner fragment"
                    )));
                }
            }
            for evidence in &fragment.events {
                if !event_ids.insert(evidence.event_id.clone()) {
                    return Err(BatchError::InvalidPlan(format!(
                        "duplicate composed event id {}",
                        evidence.event_id
                    )));
                }
                let deduplication_key = event_deduplication_identity(evidence);
                if !deduplication_keys.insert(deduplication_key.clone()) {
                    return Err(BatchError::InvalidPlan(format!(
                        "duplicate composed event deduplication identity {deduplication_key}"
                    )));
                }
            }
        }

        Ok(())
    }

    pub(crate) fn record_mutations(&self) -> impl Iterator<Item = &RecordMutation> {
        self.owner_fragments
            .iter()
            .flat_map(|fragment| fragment.records.iter())
    }

    pub(crate) fn relationship_mutations(
        &self,
    ) -> impl Iterator<Item = &RelationshipMutation> {
        self.owner_fragments
            .iter()
            .flat_map(|fragment| fragment.relationships.iter())
    }

    pub(crate) fn event_evidence(&self) -> impl Iterator<Item = &EventEvidence> {
        self.owner_fragments
            .iter()
            .flat_map(|fragment| fragment.events.iter())
    }
}

fn validate_composed_idempotency(
    context: &ModuleExecutionContext,
    idempotency: &IdempotencyEvidence,
) -> Result<(), BatchError> {
    if idempotency.scope.is_empty() || idempotency.key.is_empty() {
        return Err(BatchError::InvalidPlan(
            "idempotency scope and key must not be empty".to_owned(),
        ));
    }
    if idempotency.key != context.execution.idempotency_key.as_str() {
        return Err(BatchError::InvalidPlan(
            "idempotency evidence key must match the execution context".to_owned(),
        ));
    }
    if idempotency.request_hash.iter().all(|byte| *byte == 0) {
        return Err(BatchError::InvalidPlan(
            "idempotency request hash must not be all zeroes".to_owned(),
        ));
    }
    if idempotency.expires_at_unix_nanos <= context.execution.request_started_at_unix_nanos {
        return Err(BatchError::InvalidPlan(
            "idempotency expiry must be later than request start".to_owned(),
        ));
    }
    Ok(())
}

fn validate_composed_audits(audits: &[AuditIntent]) -> Result<(), BatchError> {
    if audits.is_empty() {
        return Err(BatchError::InvalidPlan(
            "every composed batch requires at least one audit record".to_owned(),
        ));
    }
    let mut audit_ids = BTreeSet::new();
    for audit in audits {
        audit.validate().map_err(BatchError::InvalidPlan)?;
        if !audit_ids.insert(audit.audit_record_id.clone()) {
            return Err(BatchError::InvalidPlan(format!(
                "duplicate audit record id {}",
                audit.audit_record_id
            )));
        }
    }
    Ok(())
}

fn record_mutation_key(mutation: &RecordMutation) -> String {
    format!(
        "{}:{}",
        mutation.reference().record_type,
        mutation.reference().record_id
    )
}

fn validate_event_evidence(evidence: &EventEvidence) -> Result<(), BatchError> {
    if evidence.event_id.is_empty()
        || evidence.event.deduplication_key.is_empty()
        || evidence.aggregate_version <= 0
        || evidence.event_sequence <= 0
    {
        return Err(BatchError::InvalidPlan(
            "event identifiers and versions must be positive and non-empty".to_owned(),
        ));
    }
    Ok(())
}

fn event_deduplication_identity(evidence: &EventEvidence) -> String {
    format!(
        "{}:{}",
        evidence.event.event_type, evidence.event.deduplication_key
    )
}

#[cfg(test)]
mod composition_tests {
    use super::*;
    use crm_module_sdk::{
        ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId,
        CorrelationId, EventType, ExecutionContext, IdempotencyKey, RecordId, RecordType, RequestId,
        TenantId, TraceId,
    };

    fn context() -> ModuleExecutionContext {
        ModuleExecutionContext {
            module_id: ModuleId::try_new("crm.identity-resolution").unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new("tenant-1").unwrap(),
                actor_id: ActorId::try_new("actor-1").unwrap(),
                request_id: RequestId::try_new("request-1").unwrap(),
                correlation_id: CorrelationId::try_new("correlation-1").unwrap(),
                causation_id: CausationId::try_new("causation-1").unwrap(),
                trace_id: TraceId::try_new("trace-1").unwrap(),
                capability_id: CapabilityId::try_new("identity_resolution.merge.apply").unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                business_transaction_id: BusinessTransactionId::try_new("transaction-1").unwrap(),
                idempotency_key: IdempotencyKey::try_new("idempotency-1").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: 1,
            },
        }
    }

    fn payload(owner: &str, schema: &str) -> TypedPayload {
        TypedPayload {
            owner: ModuleId::try_new(owner).unwrap(),
            schema_id: SchemaId::try_new(schema).unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            descriptor_hash: [7; 32],
            data_class: DataClass::Personal,
            encoding: PayloadEncoding::Json,
            maximum_size_bytes: 1024,
            retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
            bytes: vec![b'{', b'}'],
        }
    }

    fn reference(record_type: &str, record_id: &str) -> RecordRef {
        RecordRef {
            record_type: RecordType::try_new(record_type).unwrap(),
            record_id: RecordId::try_new(record_id).unwrap(),
        }
    }

    fn event(
        owner: &str,
        event_id: &str,
        event_type: &str,
        aggregate: RecordRef,
    ) -> EventEvidence {
        EventEvidence {
            event_id: event_id.to_owned(),
            event: DomainEvent {
                event_type: EventType::try_new(event_type).unwrap(),
                aggregate,
                expected_aggregate_version: Some(1),
                payload: payload(owner, "owner.event.v1"),
                deduplication_key: format!("{event_id}-dedupe"),
            },
            aggregate_version: 2,
            event_sequence: 2,
            occurred_at_unix_nanos: 10,
        }
    }

    fn fragment(owner: &str, record_type: &str, record_id: &str) -> OwnerMutationFragment {
        let reference = reference(record_type, record_id);
        OwnerMutationFragment {
            owner_module_id: ModuleId::try_new(owner).unwrap(),
            records: vec![RecordMutation::Update {
                reference: reference.clone(),
                expected_version: 1,
                payload: payload(owner, "owner.state.v1"),
            }],
            relationships: Vec::new(),
            events: vec![event(
                owner,
                &format!("event-{record_id}"),
                &format!("{owner}.changed"),
                reference,
            )],
        }
    }

    fn plan() -> ComposedBatchMutationPlan {
        ComposedBatchMutationPlan {
            context: context(),
            owner_fragments: vec![
                fragment("crm.parties", "parties.party", "party-a"),
                fragment(
                    "crm.identity-resolution",
                    "identity_resolution.merge_lineage",
                    "merge-1",
                ),
            ],
            idempotency: IdempotencyEvidence {
                scope: "identity_resolution.merge.apply".to_owned(),
                key: "idempotency-1".to_owned(),
                request_hash: [8; 32],
                expires_at_unix_nanos: 1_000,
            },
            audits: vec![AuditIntent {
                audit_record_id: "audit-1".to_owned(),
                canonicalization_profile: "crm.cjson/v1".to_owned(),
                canonical_envelope: br#"{"operation":"merge"}"#.to_vec(),
                occurred_at_unix_nanos: 11,
            }],
        }
    }

    #[test]
    fn accepts_distinct_owner_bound_fragments() {
        let value = plan();
        value.validate().unwrap();
        assert_eq!(value.record_mutations().count(), 2);
        assert_eq!(value.relationship_mutations().count(), 0);
        assert_eq!(value.event_evidence().count(), 2);
    }

    #[test]
    fn rejects_payload_that_claims_another_owner() {
        let mut value = plan();
        if let RecordMutation::Update { payload, .. } = &mut value.owner_fragments[0].records[0] {
            payload.owner = ModuleId::try_new("crm.identity-resolution").unwrap();
        }
        assert!(matches!(value.validate(), Err(BatchError::InvalidPlan(_))));
    }

    #[test]
    fn rejects_duplicate_record_identity_across_fragments() {
        let mut value = plan();
        value.owner_fragments[1].records[0] = RecordMutation::Update {
            reference: reference("parties.party", "party-a"),
            expected_version: 1,
            payload: payload("crm.identity-resolution", "owner.state.v1"),
        };
        assert!(matches!(value.validate(), Err(BatchError::InvalidPlan(_))));
    }

    #[test]
    fn rejects_duplicate_event_identity_across_fragments() {
        let mut value = plan();
        value.owner_fragments[1].events[0].event_id =
            value.owner_fragments[0].events[0].event_id.clone();
        assert!(matches!(value.validate(), Err(BatchError::InvalidPlan(_))));
    }

    #[test]
    fn rejects_duplicate_event_deduplication_identity_across_fragments() {
        let mut value = plan();
        value.owner_fragments[1].events[0].event.event_type =
            value.owner_fragments[0].events[0].event.event_type.clone();
        value.owner_fragments[1].events[0].event.deduplication_key =
            value.owner_fragments[0].events[0].event.deduplication_key.clone();
        assert!(matches!(value.validate(), Err(BatchError::InvalidPlan(_))));
    }

    #[test]
    fn rejects_duplicate_owner_fragment() {
        let mut value = plan();
        value.owner_fragments[1].owner_module_id = ModuleId::try_new("crm.parties").unwrap();
        value.owner_fragments[1].records[0] = RecordMutation::Update {
            reference: reference("parties.party", "party-b"),
            expected_version: 1,
            payload: payload("crm.parties", "owner.state.v1"),
        };
        value.owner_fragments[1].events[0].event.payload =
            payload("crm.parties", "owner.event.v1");
        assert!(matches!(value.validate(), Err(BatchError::InvalidPlan(_))));
    }
}
