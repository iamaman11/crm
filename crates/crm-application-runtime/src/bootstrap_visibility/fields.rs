use std::collections::BTreeSet;

pub(super) fn fields<const N: usize>(values: [&str; N]) -> BTreeSet<String> {
    values.into_iter().map(str::to_owned).collect()
}

pub(super) fn sales_fields() -> BTreeSet<String> {
    fields([
        "name",
        "stage",
        "amount",
        "owner",
        "account",
        "primary_contact",
        "expected_close_date",
        "probability_basis_points",
        "status",
        "close_outcome",
        "created_at",
        "updated_at",
    ])
}

pub(super) fn customer_data_import_job_fields() -> BTreeSet<String> {
    fields(["source", "mapping", "status", "counters", "checkpoint"])
}

pub(super) fn customer_data_import_row_fields() -> BTreeSet<String> {
    fields([
        "row_position",
        "source_identity",
        "status",
        "prepared_party",
        "diagnostics",
        "execution",
        "target_party_ref",
    ])
}

pub(super) fn customer_360_party_fields() -> BTreeSet<String> {
    fields(["display_name"])
}

pub(super) fn customer_360_account_fields() -> BTreeSet<String> {
    fields(["name", "status"])
}

pub(super) fn customer_360_contact_point_fields() -> BTreeSet<String> {
    fields([
        "party_ref",
        "kind",
        "normalized_value",
        "status",
        "preferred",
        "validity",
        "verification",
    ])
}

pub(super) fn customer_360_party_relationship_fields() -> BTreeSet<String> {
    fields(["from_party_ref", "to_party_ref", "status", "validity"])
}

pub(super) fn party_fields() -> BTreeSet<String> {
    fields(["kind", "display_name"])
}

pub(super) fn account_fields() -> BTreeSet<String> {
    fields(["name", "status", "party_associations"])
}

pub(super) fn contact_point_fields() -> BTreeSet<String> {
    fields([
        "party_ref",
        "kind",
        "normalized_value",
        "display_value",
        "status",
        "preferred",
        "validity",
        "verification",
    ])
}

pub(super) fn consent_fields() -> BTreeSet<String> {
    fields([
        "party_ref",
        "contact_point_ref",
        "purpose",
        "channel",
        "effect",
        "legal_basis",
        "jurisdiction",
        "source",
        "evidence_ref",
        "validity",
        "status",
        "resource_version",
    ])
}

pub(super) fn identity_resolution_fields() -> BTreeSet<String> {
    fields([
        "party_pair",
        "evidence_history",
        "status",
        "decision_reason",
    ])
}

pub(super) fn identity_resolution_merge_fields() -> BTreeSet<String> {
    fields([
        "party_pair",
        "decision",
        "survivorship",
        "status",
        "unmerge_decision",
    ])
}

pub(super) fn party_relationship_fields() -> BTreeSet<String> {
    fields([
        "from_party_ref",
        "to_party_ref",
        "relationship_type",
        "status",
        "validity",
    ])
}

pub(super) fn task_fields() -> BTreeSet<String> {
    fields([
        "subject",
        "description",
        "owner",
        "related_resources",
        "priority",
        "status",
        "due_at",
        "reminder_at",
        "completed_at",
        "created_at",
        "updated_at",
    ])
}
