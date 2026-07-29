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
