const BATCH_RESULT_SCHEMA_ID: &str = "crm.core.data.batch_mutation_result";
const BATCH_RESULT_SCHEMA_VERSION: &str = "1.0.0";
const BATCH_RESULT_SCHEMA_DESCRIPTOR: &[u8] =
    b"crm.core.data.batch_mutation_result/v1:records,linked_relationships,unlinked_relationships,replayed";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum RecordMutation {
    Create {
        reference: RecordRef,
        payload: TypedPayload,
    },
    Update {
        reference: RecordRef,
        expected_version: i64,
        payload: TypedPayload,
    },
}

impl RecordMutation {
    fn reference(&self) -> &RecordRef {
        match self {
            Self::Create { reference, .. } | Self::Update { reference, .. } => reference,
        }
    }

    fn payload(&self) -> &TypedPayload {
        match self {
            Self::Create { payload, .. } | Self::Update { payload, .. } => payload,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum RelationshipMutation {
    Link {
        relationship: RelationshipRef,
        payload: TypedPayload,
    },
    Unlink {
        relationship: RelationshipRef,
    },
}

impl RelationshipMutation {
    fn relationship(&self) -> &RelationshipRef {
        match self {
            Self::Link { relationship, .. } | Self::Unlink { relationship } => relationship,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventEvidence {
    pub event_id: String,
    pub event: DomainEvent,
    pub aggregate_version: i64,
    pub event_sequence: i64,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchMutationPlan {
    pub context: ModuleExecutionContext,
    pub records: Vec<RecordMutation>,
    pub relationships: Vec<RelationshipMutation>,
    pub events: Vec<EventEvidence>,
    pub idempotency: IdempotencyEvidence,
    pub audits: Vec<AuditEvidence>,
}

impl BatchMutationPlan {
    pub fn validate(&self) -> Result<(), BatchError> {
        self.context.validate().map_err(BatchError::Sdk)?;
        if self.records.is_empty() && self.relationships.is_empty() {
            return Err(BatchError::InvalidPlan(
                "at least one record or relationship mutation is required".to_owned(),
            ));
        }
        if self.events.is_empty() || self.audits.is_empty() {
            return Err(BatchError::InvalidPlan(
                "every batch requires at least one outbox event and audit record".to_owned(),
            ));
        }
        if self.idempotency.scope.is_empty() || self.idempotency.key.is_empty() {
            return Err(BatchError::InvalidPlan(
                "idempotency scope and key must not be empty".to_owned(),
            ));
        }
        if self.idempotency.key != self.context.execution.idempotency_key.as_str() {
            return Err(BatchError::InvalidPlan(
                "idempotency evidence key must match the execution context".to_owned(),
            ));
        }
        if self.idempotency.request_hash.iter().all(|byte| *byte == 0) {
            return Err(BatchError::InvalidPlan(
                "idempotency request hash must not be all zeroes".to_owned(),
            ));
        }
        if self.idempotency.expires_at_unix_nanos
            <= self.context.execution.request_started_at_unix_nanos
        {
            return Err(BatchError::InvalidPlan(
                "idempotency expiry must be later than request start".to_owned(),
            ));
        }

        let mut record_keys = BTreeSet::new();
        for mutation in &self.records {
            mutation.payload().validate().map_err(BatchError::Sdk)?;
            if mutation.payload().owner != self.context.module_id {
                return Err(BatchError::InvalidPlan(format!(
                    "record {} payload owner does not match executing module",
                    mutation.reference().record_id
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
            let key = format!(
                "{}:{}",
                mutation.reference().record_type,
                mutation.reference().record_id
            );
            if !record_keys.insert(key.clone()) {
                return Err(BatchError::InvalidPlan(format!(
                    "record {key} is mutated more than once in one batch"
                )));
            }
        }

        let mut relationship_keys = BTreeSet::new();
        for mutation in &self.relationships {
            if let RelationshipMutation::Link { payload, .. } = mutation {
                payload.validate().map_err(BatchError::Sdk)?;
                if payload.owner != self.context.module_id {
                    return Err(BatchError::InvalidPlan(
                        "relationship payload owner does not match executing module".to_owned(),
                    ));
                }
            }
            let relationship = mutation.relationship();
            let key = relationship_key(relationship);
            if !relationship_keys.insert(key.clone()) {
                return Err(BatchError::InvalidPlan(format!(
                    "relationship {key} is mutated more than once in one batch"
                )));
            }
        }

        let mut event_ids = BTreeSet::new();
        let mut deduplication_keys = BTreeSet::new();
        for evidence in &self.events {
            evidence.event.payload.validate().map_err(BatchError::Sdk)?;
            if evidence.event.payload.owner != self.context.module_id {
                return Err(BatchError::InvalidPlan(
                    "event payload owner does not match executing module".to_owned(),
                ));
            }
            if evidence.event_id.is_empty()
                || evidence.event.deduplication_key.is_empty()
                || evidence.aggregate_version <= 0
                || evidence.event_sequence <= 0
            {
                return Err(BatchError::InvalidPlan(
                    "event identifiers and versions must be positive and non-empty".to_owned(),
                ));
            }
            if !event_ids.insert(evidence.event_id.clone()) {
                return Err(BatchError::InvalidPlan(format!(
                    "duplicate event id {}",
                    evidence.event_id
                )));
            }
            let deduplication_key = format!(
                "{}:{}",
                evidence.event.event_type, evidence.event.deduplication_key
            );
            if !deduplication_keys.insert(deduplication_key.clone()) {
                return Err(BatchError::InvalidPlan(format!(
                    "duplicate event deduplication identity {deduplication_key}"
                )));
            }
        }

        let mut audit_ids = BTreeSet::new();
        for (index, audit) in self.audits.iter().enumerate() {
            if audit.audit_record_id.is_empty()
                || audit.canonicalization_profile.is_empty()
                || audit.audit_sequence <= 0
                || audit.record_hash.iter().all(|byte| *byte == 0)
            {
                return Err(BatchError::InvalidPlan(
                    "audit identifiers, sequence and hash must be valid".to_owned(),
                ));
            }
            if !audit_ids.insert(audit.audit_record_id.clone()) {
                return Err(BatchError::InvalidPlan(format!(
                    "duplicate audit record id {}",
                    audit.audit_record_id
                )));
            }
            if let Some(previous) = index
                .checked_sub(1)
                .and_then(|value| self.audits.get(value))
            {
                if audit.audit_sequence != previous.audit_sequence + 1 {
                    return Err(BatchError::InvalidPlan(
                        "audit records in a batch must use contiguous sequences".to_owned(),
                    ));
                }
                if audit.previous_hash != previous.record_hash {
                    return Err(BatchError::InvalidPlan(
                        "audit records in a batch must form a continuous hash chain".to_owned(),
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchMutationResult {
    pub records: Vec<RecordSnapshot>,
    pub linked_relationships: Vec<RelationshipRef>,
    pub unlinked_relationships: Vec<RelationshipRef>,
    pub replayed: bool,
}

#[derive(Debug)]
pub enum BatchError {
    Database(sqlx::Error),
    Sdk(SdkError),
    InvalidPlan(String),
    Conflict(String),
    IdempotencyKeyReused,
    IdempotencyInProgress,
    InvalidStoredValue(String),
}

impl fmt::Display for BatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "database operation failed: {error}"),
            Self::Sdk(error) => write!(formatter, "SDK validation failed: {error}"),
            Self::InvalidPlan(message) => write!(formatter, "invalid batch plan: {message}"),
            Self::Conflict(message) => write!(formatter, "mutation conflict: {message}"),
            Self::IdempotencyKeyReused => formatter
                .write_str("idempotency key was previously used for a different semantic request"),
            Self::IdempotencyInProgress => {
                formatter.write_str("idempotent request is already in progress")
            }
            Self::InvalidStoredValue(message) => {
                write!(formatter, "invalid stored idempotency response: {message}")
            }
        }
    }
}

impl Error for BatchError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::Sdk(error) => Some(error),
            Self::InvalidPlan(_)
            | Self::Conflict(_)
            | Self::IdempotencyKeyReused
            | Self::IdempotencyInProgress
            | Self::InvalidStoredValue(_) => None,
        }
    }
}

impl From<sqlx::Error> for BatchError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}
