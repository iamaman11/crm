#!/usr/bin/env python3
"""One-shot exact refactor for the Phase 7F metadata transition write boundary."""

from pathlib import Path

PATH = Path("crates/crm-core-data/src/metadata_store.rs")


def replace_once(text: str, old: str, new: str, description: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"expected exactly one {description} match, found {count}")
    return text.replace(old, new, 1)


def main() -> None:
    text = PATH.read_text()
    if "struct MetadataTransitionWrite<'a>" in text:
        print("MetadataTransitionWrite refactor is already applied.")
        return

    text = replace_once(
        text,
        '''            insert_transition(
                &mut transaction,
                context,
                MetadataTransitionAction::Publish,
                state.generation,
                state.rollback_depth,
                None,
                &revision_id,
                occurred_at_unix_nanos,
            )
            .await?;''',
        '''            insert_transition(
                &mut transaction,
                context,
                MetadataTransitionWrite {
                    action: MetadataTransitionAction::Publish,
                    generation: state.generation,
                    rollback_depth: state.rollback_depth,
                    from_revision: None,
                    to_revision: &revision_id,
                    occurred_at_unix_nanos,
                },
            )
            .await?;''',
        "publish transition call",
    )
    text = replace_once(
        text,
        '''        insert_transition(
            &mut transaction,
            context,
            MetadataTransitionAction::Activate,
            next_generation,
            next_depth,
            previous_revision.as_ref(),
            candidate_revision,
            occurred_at_unix_nanos,
        )
        .await?;''',
        '''        insert_transition(
            &mut transaction,
            context,
            MetadataTransitionWrite {
                action: MetadataTransitionAction::Activate,
                generation: next_generation,
                rollback_depth: next_depth,
                from_revision: previous_revision.as_ref(),
                to_revision: candidate_revision,
                occurred_at_unix_nanos,
            },
        )
        .await?;''',
        "activation transition call",
    )
    text = replace_once(
        text,
        '''        insert_transition(
            &mut transaction,
            context,
            MetadataTransitionAction::Rollback,
            next_generation,
            next_depth,
            Some(&replaced_revision),
            &target_revision,
            occurred_at_unix_nanos,
        )
        .await?;''',
        '''        insert_transition(
            &mut transaction,
            context,
            MetadataTransitionWrite {
                action: MetadataTransitionAction::Rollback,
                generation: next_generation,
                rollback_depth: next_depth,
                from_revision: Some(&replaced_revision),
                to_revision: &target_revision,
                occurred_at_unix_nanos,
            },
        )
        .await?;''',
        "rollback transition call",
    )

    old_function = '''async fn insert_transition(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
    action: MetadataTransitionAction,
    generation: u64,
    rollback_depth: usize,
    from_revision: Option<&MetadataRevisionId>,
    to_revision: &MetadataRevisionId,
    occurred_at_unix_nanos: i64,
) -> Result<(), MetadataPersistenceError> {
    let transition_id = transition_id(context, action, generation, to_revision)?;
    sqlx::query(
        r#"
        INSERT INTO crm.metadata_transitions (
          tenant_id,
          transition_id,
          action,
          generation,
          rollback_depth,
          from_revision_id,
          to_revision_id,
          actor_id,
          request_id,
          capability_id,
          capability_version,
          business_transaction_id,
          occurred_at
        )
        VALUES (
          $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
          TIMESTAMPTZ 'epoch' + ($13::bigint / 1000) * INTERVAL '1 microsecond'
        )
        "#,
    )
    .bind(context.execution.tenant_id.as_str())
    .bind(transition_id)
    .bind(action.as_str())
    .bind(database_i64(generation, "generation")?)
    .bind(database_i64(rollback_depth, "rollback depth")?)
    .bind(from_revision.map(|revision| revision.as_bytes().as_slice()))
    .bind(to_revision.as_bytes().as_slice())
    .bind(context.execution.actor_id.as_str())
    .bind(context.execution.request_id.as_str())
    .bind(context.execution.capability_id.as_str())
    .bind(context.execution.capability_version.as_str())
    .bind(context.execution.business_transaction_id.as_str())
    .bind(occurred_at_unix_nanos)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}'''
    new_function = '''struct MetadataTransitionWrite<'a> {
    action: MetadataTransitionAction,
    generation: u64,
    rollback_depth: usize,
    from_revision: Option<&'a MetadataRevisionId>,
    to_revision: &'a MetadataRevisionId,
    occurred_at_unix_nanos: i64,
}

async fn insert_transition(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
    transition: MetadataTransitionWrite<'_>,
) -> Result<(), MetadataPersistenceError> {
    let transition_id = transition_id(
        context,
        transition.action,
        transition.generation,
        transition.to_revision,
    )?;
    sqlx::query(
        r#"
        INSERT INTO crm.metadata_transitions (
          tenant_id,
          transition_id,
          action,
          generation,
          rollback_depth,
          from_revision_id,
          to_revision_id,
          actor_id,
          request_id,
          capability_id,
          capability_version,
          business_transaction_id,
          occurred_at
        )
        VALUES (
          $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
          TIMESTAMPTZ 'epoch' + ($13::bigint / 1000) * INTERVAL '1 microsecond'
        )
        "#,
    )
    .bind(context.execution.tenant_id.as_str())
    .bind(transition_id)
    .bind(transition.action.as_str())
    .bind(database_i64(transition.generation, "generation")?)
    .bind(database_i64(transition.rollback_depth, "rollback depth")?)
    .bind(
        transition
            .from_revision
            .map(|revision| revision.as_bytes().as_slice()),
    )
    .bind(transition.to_revision.as_bytes().as_slice())
    .bind(context.execution.actor_id.as_str())
    .bind(context.execution.request_id.as_str())
    .bind(context.execution.capability_id.as_str())
    .bind(context.execution.capability_version.as_str())
    .bind(context.execution.business_transaction_id.as_str())
    .bind(transition.occurred_at_unix_nanos)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}'''
    text = replace_once(text, old_function, new_function, "insert_transition function")
    PATH.write_text(text)
    print("Applied MetadataTransitionWrite refactor.")


if __name__ == "__main__":
    main()
