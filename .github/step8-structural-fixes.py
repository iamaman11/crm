from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)


def replace_last(text: str, old: str, new: str, label: str) -> str:
    position = text.rfind(old)
    if position < 0:
        raise SystemExit(f"{label}: match not found")
    return text[:position] + new + text[position + len(old):]


application = Path("crates/crm-customer-privacy-application/src/execution.rs")
text = application.read_text()
text = replace_once(
    text,
    """    Ready {
        attempt: PrivacyOwnerActionAttempt,
        attempt_replayed: bool,
    },""",
    """    Ready {
        attempt: Box<PrivacyOwnerActionAttempt>,
        attempt_replayed: bool,
    },""",
    "application ready variant",
)
text = replace_once(
    text,
    "            } => (attempt, attempt_replayed),",
    "            } => (*attempt, attempt_replayed),",
    "application ready extraction",
)
text = replace_last(
    text,
    "                attempt,\n                attempt_replayed,",
    "                attempt: Box::new(attempt),\n                attempt_replayed,",
    "application scripted preparation",
)
text = replace_once(
    text,
    "        OwnerScopeRegistry, PlannedPrivacyAction, PrivacyActionPlan, PrivacyCaseKind,\n        PrivacyRetentionDecisionSet, RetentionDecisionReason, ScopeDiscoveryLineage, ScopeResource,",
    "        OwnerScopeRegistry, PrivacyActionPlan, PrivacyCaseKind, PrivacyRetentionDecisionSet,\n        ScopeDiscoveryLineage, ScopeResource,",
    "application test imports",
)
text = replace_once(
    text,
    "    use std::task::{Context, Poll, Wake, Waker};",
    "    use std::task::{Context, Poll, Waker};",
    "application test task imports",
)
text = replace_once(
    text,
    """    struct NoopWake;

    impl Wake for NoopWake {
        fn wake(self: Arc<Self>) {}
    }

""",
    "",
    "application manual no-op waker",
)
text = replace_once(
    text,
    "        let waker = Waker::from(Arc::new(NoopWake));\n        let mut context = Context::from_waker(&waker);",
    "        let mut context = Context::from_waker(Waker::noop());",
    "application standard no-op waker",
)
application.write_text(text)

postgres = Path("crates/crm-customer-privacy-postgres/src/execution.rs")
text = postgres.read_text()
text = replace_once(
    text,
    "                attempt,\n                attempt_replayed: replayed || !inserted,",
    "                attempt: Box::new(attempt),\n                attempt_replayed: replayed || !inserted,",
    "PostgreSQL ready preparation",
)
text = replace_once(
    text,
    """        SELECT set_config('app.tenant_id', $1, true),
               set_config('app.actor_id', $2, true),
               set_config('app.request_id', $3, true),
               set_config('app.capability_id', $4, true),
               set_config('app.capability_version', $5, true)
""",
    """        SELECT set_config('app.tenant_id', $1, true),
               set_config('app.actor_id', $2, true),
               set_config('app.request_id', $3, true),
               set_config('app.capability_id', $4, true),
               set_config('app.capability_version', $5, true),
               set_config('app.business_transaction_id', $6, true)
""",
    "PostgreSQL complete transaction-local context",
)
text = replace_once(
    text,
    """    .bind(invocation.initiating_capability_version.as_str())
    .execute(&mut **transaction)""",
    """    .bind(invocation.initiating_capability_version.as_str())
    .bind(transaction_id(invocation))
    .execute(&mut **transaction)""",
    "PostgreSQL business transaction binding",
)
text = replace_once(
    text,
    """    let privacy_case = load_case(
        transaction,
        &invocation.tenant_id,
        &invocation.privacy_case_id,
        true,
    )
    .await?
    .ok_or_else(case_not_found)?;
    let plan = load_plan(""",
    """    let initial_case = load_case(
        transaction,
        &invocation.tenant_id,
        &invocation.privacy_case_id,
        false,
    )
    .await?
    .ok_or_else(case_not_found)?;
    let canonical_party_id = initial_case
        .subject_binding()
        .map(|binding| binding.canonical_party_id.clone())
        .ok_or_else(|| execution_conflict("privacy case has no verified canonical Party"))?;
    lock_customer_subject(transaction, &invocation.tenant_id, &canonical_party_id).await?;
    let privacy_case = load_case(
        transaction,
        &invocation.tenant_id,
        &invocation.privacy_case_id,
        true,
    )
    .await?
    .ok_or_else(case_not_found)?;
    if privacy_case != initial_case {
        return Err(execution_conflict(
            "privacy case changed while acquiring the shared subject lock",
        ));
    }
    let plan = load_plan(""",
    "PostgreSQL canonical Party lock and case recheck",
)
text = replace_once(
    text,
    """async fn load_execution_source(
""",
    """async fn lock_customer_subject(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    canonical_party_id: &RecordId,
) -> Result<(), SdkError> {
    postgres_sqlx::query("SELECT crm.lock_customer_subject($1, $2)")
        .bind(tenant_id.as_str())
        .bind(canonical_party_id.as_str())
        .execute(&mut **transaction)
        .await
        .map_err(database_error)?;
    Ok(())
}

async fn load_execution_source(
""",
    "PostgreSQL canonical Party lock helper",
)
text = replace_once(
    text,
    "    update_case_record(transaction, source_case_version, &source.privacy_case).await?;",
    "    update_case_record(transaction, invocation, source_case_version, &source.privacy_case).await?;",
    "PostgreSQL execution-start case transaction lineage",
)
text = replace_once(
    text,
    "            update_case_record(transaction, expected, &source.privacy_case).await?;",
    "            update_case_record(transaction, invocation, expected, &source.privacy_case).await?;",
    "PostgreSQL convergence case transaction lineage",
)
text = replace_once(
    text,
    """async fn update_case_record(
    transaction: &mut Transaction<'_, Postgres>,
    expected_version: u64,
    privacy_case: &PrivacyCase,
) -> Result<(), SdkError> {""",
    """async fn update_case_record(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &OwnerExecutionInvocation,
    expected_version: u64,
    privacy_case: &PrivacyCase,
) -> Result<(), SdkError> {""",
    "PostgreSQL case update invocation",
)
text = replace_once(
    text,
    """            retention_policy_id = $10,
            payload_bytes = $11,
            updated_at = clock_timestamp()
        WHERE tenant_id = $1 AND owner_module_id = $2
          AND record_type = $3 AND record_id = $4
          AND version = $12 AND deleted_at IS NULL
""",
    """            retention_policy_id = $10,
            payload_bytes = $11,
            last_business_transaction_id = $12,
            updated_at = clock_timestamp()
        WHERE tenant_id = $1 AND owner_module_id = $2
          AND record_type = $3 AND record_id = $4
          AND version = $13 AND deleted_at IS NULL
""",
    "PostgreSQL case business transaction persistence",
)
text = replace_once(
    text,
    """    .bind(payload.bytes)
    .bind(checked_i64(expected_version, "expected case version")?)""",
    """    .bind(payload.bytes)
    .bind(transaction_id(invocation))
    .bind(checked_i64(expected_version, "expected case version")?)""",
    "PostgreSQL case business transaction value",
)
text = replace_once(
    text,
    """fn case_not_found() -> SdkError {""",
    """fn transaction_id(invocation: &OwnerExecutionInvocation) -> String {
    let mut bytes = Vec::new();
    for value in [
        invocation.initiating_capability_id.as_str().as_bytes(),
        invocation.initiating_capability_version.as_str().as_bytes(),
        invocation.request_id.as_str().as_bytes(),
    ] {
        append_digest_field(&mut bytes, value);
    }
    let digest = discovery_sha256(&bytes);
    format!("privacy-owner-execution-{}", hex(&digest[..12]))
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn case_not_found() -> SdkError {""",
    "PostgreSQL deterministic transaction identity",
)
postgres.write_text(text)

acceptance = Path("crates/crm-application-runtime/tests/customer_privacy_owner_execution_postgres.rs")
text = acceptance.read_text()
text = replace_once(
    text,
    "            assert_eq!(attempt_replayed, replayed);\n            attempt",
    "            assert_eq!(attempt_replayed, replayed);\n            *attempt",
    "PostgreSQL acceptance ready extraction",
)
text = replace_once(
    text,
    '"CUSTOMER_PRIVACY_OWNER_EXECUTION_CASE_NOT_FOUND"',
    '"CUSTOMER_PRIVACY_CASE_NOT_FOUND"',
    "PostgreSQL acceptance cross-tenant concealment code",
)
acceptance.write_text(text)

reads = Path("crates/crm-customer-privacy-application/src/reads.rs")
text = reads.read_text()
text = replace_once(
    text,
    """        let page_digest = owner_outcome_page_digest_for_items(
            &request.context.tenant_id,
            &privacy_case_id,
            source.action_plan.plan_id(),
            owner_module_filter.as_ref(),
            page_size,
            after.as_ref(),
            &page.outcomes,
            &next_cursor,
        );""",
    """        let page_digest = owner_outcome_page_digest_for_items(
            &PrivacyOwnerOutcomePageDigestContext {
                tenant_id: &request.context.tenant_id,
                privacy_case_id: &privacy_case_id,
                action_plan_id: source.action_plan.plan_id(),
                owner_module_filter: owner_module_filter.as_ref(),
                page_size,
            },
            after.as_ref(),
            &page.outcomes,
            &next_cursor,
        );""",
    "owner outcome digest call",
)
marker = "pub fn owner_outcome_page_digest(\n"
if text.count(marker) != 1:
    raise SystemExit("owner outcome digest context marker is not unique")
context = """#[derive(Debug, Clone, Copy)]
pub struct PrivacyOwnerOutcomePageDigestContext<'a> {
    pub tenant_id: &'a TenantId,
    pub privacy_case_id: &'a RecordId,
    pub action_plan_id: &'a RecordId,
    pub owner_module_filter: Option<&'a ModuleId>,
    pub page_size: u32,
}

"""
text = text.replace(marker, context + marker, 1)
text = replace_once(
    text,
    """    owner_outcome_page_digest_for_items(
        tenant_id,
        privacy_case_id,
        plan_id,
        owner_module_filter,
        page_size,
        None,
        &[],
        "",
    )""",
    """    owner_outcome_page_digest_for_items(
        &PrivacyOwnerOutcomePageDigestContext {
            tenant_id,
            privacy_case_id,
            action_plan_id: plan_id,
            owner_module_filter,
            page_size,
        },
        None,
        &[],
        "",
    )""",
    "owner outcome digest wrapper",
)
text = replace_once(
    text,
    """pub fn owner_outcome_page_digest_for_items(
    tenant_id: &TenantId,
    privacy_case_id: &RecordId,
    plan_id: &RecordId,
    owner_module_filter: Option<&ModuleId>,
    page_size: u32,
    after: Option<&PrivacyOwnerOutcomePosition>,
    outcomes: &[PrivacyOwnerActionOutcome],
    next_cursor: &str,
) -> [u8; 32] {
    let page_size = page_size.to_string();
    let owner = owner_module_filter.map(ModuleId::as_str).unwrap_or("");""",
    """pub fn owner_outcome_page_digest_for_items(
    context: &PrivacyOwnerOutcomePageDigestContext<'_>,
    after: Option<&PrivacyOwnerOutcomePosition>,
    outcomes: &[PrivacyOwnerActionOutcome],
    next_cursor: &str,
) -> [u8; 32] {
    let page_size = context.page_size.to_string();
    let owner = context
        .owner_module_filter
        .map(ModuleId::as_str)
        .unwrap_or("");""",
    "owner outcome digest signature",
)
text = replace_once(
    text,
    """        tenant_id.as_str().as_bytes(),
        privacy_case_id.as_str().as_bytes(),
        plan_id.as_str().as_bytes(),""",
    """        context.tenant_id.as_str().as_bytes(),
        context.privacy_case_id.as_str().as_bytes(),
        context.action_plan_id.as_str().as_bytes(),""",
    "owner outcome digest lineage",
)
reads.write_text(text)
