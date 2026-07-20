from pathlib import Path

path = Path(
    'crates/crm-customer-enrichment-application-composition/tests/'
    'postgres_application_process.rs'
)
text = path.read_text()

replacements = [
    (
        """            "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = 'tenant-application-a' AND owner_module_id = 'crm.customer-enrichment'",
        )
        .await,
        3
""",
        """            "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = 'tenant-application-a' AND owner_module_id = 'crm.customer-enrichment'",
        )
        .await,
        4
""",
    ),
    (
        """            "SELECT count(*)::bigint FROM crm.outbox_events WHERE tenant_id = 'tenant-application-a' AND event_type LIKE 'customer_enrichment.%'",
        )
        .await,
        4
""",
        """            "SELECT count(*)::bigint FROM crm.outbox_events WHERE tenant_id = 'tenant-application-a' AND event_type LIKE 'customer_enrichment.%'",
        )
        .await,
        5
""",
    ),
    (
        """            "SELECT count(*)::bigint FROM crm.audit_records WHERE tenant_id = 'tenant-application-a' AND capability_id LIKE 'customer_enrichment.%'",
        )
        .await,
        5
""",
        """            "SELECT count(*)::bigint FROM crm.audit_records WHERE tenant_id = 'tenant-application-a' AND capability_id LIKE 'customer_enrichment.%'",
        )
        .await,
        6
""",
    ),
    (
        """            "SELECT count(*)::bigint FROM crm.idempotency_records WHERE tenant_id = 'tenant-application-a'",
        )
        .await,
        5
""",
        """            "SELECT count(*)::bigint FROM crm.idempotency_records WHERE tenant_id = 'tenant-application-a'",
        )
        .await,
        6
""",
    ),
    (
        """            "SELECT count(*)::bigint FROM crm.business_transactions WHERE tenant_id = 'tenant-application-a'",
        )
        .await,
        5
""",
        """            "SELECT count(*)::bigint FROM crm.business_transactions WHERE tenant_id = 'tenant-application-a'",
        )
        .await,
        6
""",
    ),
]

for old, new in replacements:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f'expected one exact count anchor, found {count}')
    text = text.replace(old, new, 1)

path.write_text(text)
