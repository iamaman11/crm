from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    target = Path(path)
    text = target.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one exact match, found {count}")
    target.write_text(text.replace(old, new, 1))


replace_once(
    "crates/crm-core-data/src/postgres_batch/model.rs",
    """    Update {
        reference: RecordRef,
        expected_version: i64,
        payload: TypedPayload,
    },
}""",
    """    Update {
        reference: RecordRef,
        expected_version: i64,
        payload: TypedPayload,
    },
    Delete {
        reference: RecordRef,
        expected_version: i64,
        tombstone: TypedPayload,
    },
}""",
)
replace_once(
    "crates/crm-core-data/src/postgres_batch/model.rs",
    """        match self {
            Self::Create { reference, .. } | Self::Update { reference, .. } => reference,
        }""",
    """        match self {
            Self::Create { reference, .. }
            | Self::Update { reference, .. }
            | Self::Delete { reference, .. } => reference,
        }""",
)
replace_once(
    "crates/crm-core-data/src/postgres_batch/model.rs",
    """        match self {
            Self::Create { payload, .. } | Self::Update { payload, .. } => payload,
        }""",
    """        match self {
            Self::Create { payload, .. } | Self::Update { payload, .. } => payload,
            Self::Delete { tombstone, .. } => tombstone,
        }""",
)
replace_once(
    "crates/crm-core-data/src/postgres_batch/model.rs",
    """            if matches!(
                mutation,
                RecordMutation::Update {
                    expected_version,
                    ..
                } if *expected_version <= 0
            ) {
                return Err(BatchError::InvalidPlan(
                    \"record update expected_version must be positive\".to_owned(),
                ));
            }""",
    """            if matches!(
                mutation,
                RecordMutation::Update {
                    expected_version,
                    ..
                } | RecordMutation::Delete {
                    expected_version,
                    ..
                } if *expected_version <= 0
            ) {
                return Err(BatchError::InvalidPlan(
                    \"record update/delete expected_version must be positive\".to_owned(),
                ));
            }""",
)

replace_once(
    "crates/crm-core-data/src/postgres_batch/records.rs",
    """        RecordMutation::Update {
            reference,
            expected_version,
            payload,
        } => update_record(transaction, context, reference, *expected_version, payload).await,
    }
}""",
    """        RecordMutation::Update {
            reference,
            expected_version,
            payload,
        } => update_record(transaction, context, reference, *expected_version, payload).await,
        RecordMutation::Delete {
            reference,
            expected_version,
            tombstone,
        } => {
            delete_record(
                transaction,
                context,
                reference,
                *expected_version,
                tombstone,
            )
            .await
        }
    }
}""",
)
replace_once(
    "crates/crm-core-data/src/postgres_batch/records.rs",
    """async fn link_relationship(
    transaction: &mut Transaction<'_, Postgres>,
""",
    """async fn delete_record(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
    reference: &RecordRef,
    expected_version: i64,
    tombstone: &TypedPayload,
) -> Result<RecordSnapshot, BatchError> {
    let maximum_size = checked_size(tombstone.maximum_size_bytes, \"record tombstone payload\")?;
    let row = sqlx::query(
        r#\"
        UPDATE crm.records
           SET version = version + 1,
               schema_id = $4,
               schema_version = $5,
               descriptor_hash = $6,
               data_class = $7,
               payload_encoding = $8,
               maximum_payload_size = $9,
               retention_policy_id = $10,
               payload_bytes = $11,
               last_business_transaction_id = $12,
               updated_at = clock_timestamp(),
               deleted_at = clock_timestamp()
         WHERE tenant_id = $1
           AND record_type = $2
           AND record_id = $3
           AND owner_module_id = $13
           AND version = $14
           AND deleted_at IS NULL
        RETURNING version
        \"#,
    )
    .bind(context.execution.tenant_id.as_str())
    .bind(reference.record_type.as_str())
    .bind(reference.record_id.as_str())
    .bind(tombstone.schema_id.as_str())
    .bind(tombstone.schema_version.as_str())
    .bind(tombstone.descriptor_hash.as_slice())
    .bind(data_class_name(tombstone.data_class))
    .bind(payload_encoding_name(tombstone.encoding))
    .bind(maximum_size)
    .bind(tombstone.retention_policy_id.as_str())
    .bind(tombstone.bytes.as_slice())
    .bind(context.execution.business_transaction_id.as_str())
    .bind(context.module_id.as_str())
    .bind(expected_version)
    .fetch_optional(&mut **transaction)
    .await?;
    let row = row.ok_or_else(|| {
        BatchError::Conflict(format!(
            \"record {}:{} does not exist, is not owned by the module, or version {} is stale\",
            reference.record_type, reference.record_id, expected_version
        ))
    })?;
    Ok(RecordSnapshot {
        reference: reference.clone(),
        version: row.try_get(\"version\")?,
        payload: tombstone.clone(),
    })
}

async fn link_relationship(
    transaction: &mut Transaction<'_, Postgres>,
""",
)

replace_once(
    "crates/crm-core-data/src/postgres_batch/executor.rs",
    """    let mut matching = plan.batch.records.iter().filter(|mutation| match mutation {
        RecordMutation::Create { reference, .. } | RecordMutation::Update { reference, .. } => {
            reference == &target.reference
        }
    });""",
    """    let mut matching = plan.batch.records.iter().filter(|mutation| match mutation {
        RecordMutation::Create { reference, .. }
        | RecordMutation::Update { reference, .. }
        | RecordMutation::Delete { reference, .. } => reference == &target.reference,
    });""",
)
replace_once(
    "crates/crm-core-data/src/postgres_batch/executor.rs",
    """        (
            AggregatePresence::MustExist,
            Some(snapshot),
            RecordMutation::Update {
                expected_version, ..
            },
        ) if *expected_version == snapshot.version => Ok(()),""",
    """        (
            AggregatePresence::MustExist,
            Some(snapshot),
            RecordMutation::Update {
                expected_version, ..
            }
            | RecordMutation::Delete {
                expected_version, ..
            },
        ) if *expected_version == snapshot.version => Ok(()),""",
)

replace_once(
    "crates/crm-core-data/src/postgres_batch/tests.rs",
    """    #[test]
    fn rejects_duplicate_audit_record_identity() {""",
    """    #[test]
    fn accepts_optimistic_delete_with_bounded_tombstone() {
        let mut value = plan();
        value.records = vec![RecordMutation::Delete {
            reference: reference(),
            expected_version: 1,
            tombstone: payload(),
        }];

        value.validate().unwrap();
        value.validate_transactional_aggregate().unwrap();
    }

    #[test]
    fn rejects_non_positive_delete_version() {
        let mut value = plan();
        value.records = vec![RecordMutation::Delete {
            reference: reference(),
            expected_version: 0,
            tombstone: payload(),
        }];

        assert!(matches!(value.validate(), Err(BatchError::InvalidPlan(_))));
    }

    #[test]
    fn rejects_duplicate_audit_record_identity() {""",
)
