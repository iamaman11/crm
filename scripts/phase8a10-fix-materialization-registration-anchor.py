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
path.write_text(text.replace(old, new, 1))
