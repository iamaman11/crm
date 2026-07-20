from pathlib import Path

path = Path("crates/crm-application-runtime/src/customer_enrichment_provider_registry.rs")
text = path.read_text()
replacements = {
    "let error = registry.resolve_exact(&coordinate()).unwrap_err();": "let error = registry\n            .resolve_exact(&coordinate())\n            .err()\n            .expect(\"disabled coordinate must fail closed\");",
    "let error = catalog.resolve_exact(\"registry_http\", &other).unwrap_err();": "let error = catalog\n            .resolve_exact(\"registry_http\", &other)\n            .err()\n            .expect(\"unknown exact transport coordinate must fail closed\");",
}
for old, new in replacements.items():
    if text.count(old) != 1:
        raise SystemExit(f"expected one provider-registry test anchor: {old}")
    text = text.replace(old, new, 1)
path.write_text(text)
