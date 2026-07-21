from pathlib import Path

path = Path("scripts/tmp_apply_provider_canonical_outcome.py")
text = path.read_text()

replacements = [
    (
        """                            if replayed {
                                cycle.rejection_replays =
                                    cycle.rejection_replays.saturating_add(1);
                            }
""",
        """                            if replayed {
                                cycle.rejection_replays = cycle.rejection_replays.saturating_add(1);
                            }
""",
    ),
    (
        "'''#[derive(Debug)]\nenum DeliveryDisposition {\n'''",
        "'''#[derive(Debug, Clone, PartialEq)]\nenum DeliveryDisposition {\n'''",
    ),
]

for old, new in replacements:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"expected exactly one canonical-outcome marker, found {count}: {old[:120]!r}")
    text = text.replace(old, new, 1)

path.write_text(text)
