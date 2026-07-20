from pathlib import Path

FILES = [
    Path('crates/crm-application-runtime/tests/postgres_customer_enrichment_suggestion_reject.rs'),
    Path('crates/crm-application-runtime/tests/postgres_customer_enrichment_suggestion_accept.rs'),
]

for path in FILES:
    text = path.read_text()
    replacements = [
        (
            'Arc::new(DeterministicRandom::from_bytes(0_u8..=127))',
            'Arc::new(DeterministicRandom::from_bytes(\n                (0_u8..=255).cycle().take(4_096),\n            ))',
        ),
        (
            'Arc::new(DeterministicRandom::from_bytes(128_u8..=255))',
            'Arc::new(DeterministicRandom::from_bytes(\n                (0_u8..=255).cycle().take(4_096),\n            ))',
        ),
        ('"AUTHENTICATION_TENANT_FORBIDDEN"', '"TENANT_FORBIDDEN"'),
    ]
    for old, new in replacements:
        count = text.count(old)
        if count != 1:
            raise SystemExit(f'{path}: expected one exact transport fixture anchor for {old!r}, found {count}')
        text = text.replace(old, new, 1)
    path.write_text(text)
