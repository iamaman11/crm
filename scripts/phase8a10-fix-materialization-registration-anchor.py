from pathlib import Path

path = Path("scripts/phase8a10-materialization-registration.py")
text = path.read_text()

old = '''replace_once(
    background,
    "        customer_enrichment_provider_process,\\n        customer_enrichment_application_worker,",
    "        customer_enrichment_provider_process,\\n        customer_enrichment_materialization_process,\\n        customer_enrichment_application_worker,",
)
'''
new = '''replace_once(
    background,
    "        export_selection_worker,\\n        customer_enrichment_provider_process,\\n        customer_enrichment_application_worker,",
    "        export_selection_worker,\\n        customer_enrichment_provider_process,\\n        customer_enrichment_materialization_process,\\n        customer_enrichment_application_worker,",
)
'''
if text.count(old) != 1:
    raise SystemExit("expected one ambiguous destructuring anchor in materialization patch")
text = text.replace(old, new, 1)

function_anchor = '''\n\nbackground = "crates/crm-application-runtime/src/background.rs"\n'''
count_function = '''\n\ndef replace_exact_count(path: str, old: str, new: str, expected_count: int) -> None:
    file = Path(path)
    text = file.read_text()
    count = text.count(old)
    if count != expected_count:
        raise SystemExit(
            f"{path}: expected {expected_count} replacement anchors, found {count}: {old[:120]!r}"
        )
    file.write_text(text.replace(old, new))


background = "crates/crm-application-runtime/src/background.rs"
'''
if text.count(function_anchor) != 1:
    raise SystemExit("expected one background declaration anchor")
text = text.replace(function_anchor, count_function, 1)

assert_block = '''replace_once(
    background,
    'assert_eq!(*calls.lock().unwrap(), vec!["provider", "application"]);',
    'assert_eq!(\\n            *calls.lock().unwrap(),\\n            vec!["provider", "materialization", "application"]\\n        );',
)
replace_once(
    background,
    'assert_eq!(*calls.lock().unwrap(), vec!["provider", "application"]);',
    'assert_eq!(\\n            *calls.lock().unwrap(),\\n            vec!["provider", "materialization", "application"]\\n        );',
)
'''
assert_replacement = '''replace_exact_count(
    background,
    'assert_eq!(*calls.lock().unwrap(), vec!["provider", "application"]);',
    'assert_eq!(\\n            *calls.lock().unwrap(),\\n            vec!["provider", "materialization", "application"]\\n        );',
    2,
)
'''
if text.count(assert_block) != 1:
    raise SystemExit("expected duplicated lifecycle assertion patch")
text = text.replace(assert_block, assert_replacement, 1)

recovery_block = '''replace_once(
    background,
    'vec!["provider", "application", "provider", "application"]',
    'vec![\\n                "provider",\\n                "materialization",\\n                "application",\\n                "provider",\\n                "materialization",\\n                "application",\\n            ]',
)
replace_once(
    background,
    'vec!["provider", "application", "provider", "application"]',
    'vec![\\n                "provider",\\n                "materialization",\\n                "application",\\n                "provider",\\n                "materialization",\\n                "application",\\n            ]',
)
'''
recovery_replacement = '''replace_exact_count(
    background,
    'vec!["provider", "application", "provider", "application"]',
    'vec![\\n                "provider",\\n                "materialization",\\n                "application",\\n                "provider",\\n                "materialization",\\n                "application",\\n            ]',
    2,
)
'''
if text.count(recovery_block) != 1:
    raise SystemExit("expected duplicated lifecycle recovery patch")
text = text.replace(recovery_block, recovery_replacement, 1)

path.write_text(text)
