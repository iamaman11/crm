from pathlib import Path

path = Path("crates/crm-customer-privacy-postgres/src/access_export.rs")
text = path.read_text(encoding="utf-8")
replacements = [
    (
        'const ACCESS_EXPORT_AUDIT_DOMAIN: &[u8] = b"crm.customer-privacy.access-export-audit/v1";\n',
        '',
        'obsolete private audit domain',
    ),
    (
        '''            append_access_export_audit(
                &mut transaction,
                invocation,
                "access_export_prepared",
                &prepared,
                invocation.request_started_at_unix_nanos,
            )
            .await?;
''',
        '',
        'prepared private audit write',
    ),
    (
        '''            append_access_export_audit(
                &mut transaction,
                invocation,
                "access_export_completed",
                &completed,
                result.completed_at_unix_nanos,
            )
            .await?;
''',
        '',
        'completed private audit write',
    ),
]
for old, new, label in replacements:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: found {count}, expected 1")
    text = text.replace(old, new)
start = text.index("async fn append_access_export_audit(")
end = text.index("fn access_export_transaction_id(", start)
text = text[:start] + text[end:]
path.write_text(text, encoding="utf-8")
