use crate::audit::{
    AuditIntent, AuditMaterializationError, MaterializedAuditRecord, materialize_audit_chain,
};
use crate::postgres::{FaultInjection, IdempotencyEvidence, PostgresDataStore};
use crm_module_sdk::{
    DataClass, DomainEvent, ErrorCategory, ModuleExecutionContext, PayloadEncoding, RecordRef,
    RecordSnapshot, RelationshipRef, SdkError, TypedPayload,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Row, Transaction};
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

include!("postgres_batch/model.rs");
include!("postgres_batch/executor.rs");
include!("postgres_batch/records.rs");
include!("postgres_batch/evidence.rs");
include!("postgres_batch/helpers.rs");
include!("postgres_batch/tests.rs");
