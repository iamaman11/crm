from pathlib import Path

path = Path("scripts/phase8a10-suggestion-lifecycle-query-proof.py")
text = path.read_text()
old = '''for old, new in [("        2\\n", "        3\\n"), ("        2\\n", "        3\\n"), ("        2\\n", "        3\\n"), ("        2\\n", "        3\\n")]:
    text = Path(process).read_text()
    marker = old
    index = text.find(marker, text.find("assert_eq!("))
    if index == -1:
        raise SystemExit("review process count anchor missing")
    Path(process).write_text(text[:index] + new + text[index + len(old):])
'''
new = """for query in [
    "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = 'tenant-a' AND owner_module_id = 'crm.customer-enrichment'",
    "SELECT count(*)::bigint FROM crm.outbox_events WHERE tenant_id = 'tenant-a' AND event_type LIKE 'customer_enrichment.%'",
    "SELECT count(*)::bigint FROM crm.audit_records WHERE tenant_id = 'tenant-a' AND capability_id LIKE 'customer_enrichment.%'",
    "SELECT count(*)::bigint FROM crm.idempotency_records WHERE tenant_id = 'tenant-a' AND (idempotency_scope = 'customer_enrichment.review.seed@1.0.0' OR idempotency_scope = 'capability:customer_enrichment.suggestion.accept:1.0.0')",
    "SELECT count(*)::bigint FROM crm.business_transactions WHERE tenant_id = 'tenant-a' AND capability_id IN ('customer_enrichment.review.seed', 'customer_enrichment.suggestion.accept')",
]:
    replace_once(
        process,
        f'''            "{query}",
        )
        .await,
        2
''',
        f'''            "{query}",
        )
        .await,
        3
''',
    )
"""
if text.count(old) != 1:
    raise SystemExit("expected one broad lifecycle count replacement block")
text = text.replace(old, new, 1)

escaped_old = r'canonical_envelope: format!("{{\"seed\":\"{suffix}\"}}").into_bytes(),'
escaped_new = r'canonical_envelope: format!("{{\\\"seed\\\":\\\"{suffix}\\\"}}").into_bytes(),'
if text.count(escaped_old) != 1:
    raise SystemExit("expected one canonical envelope escape anchor")
text = text.replace(escaped_old, escaped_new, 1)

path.write_text(text)
