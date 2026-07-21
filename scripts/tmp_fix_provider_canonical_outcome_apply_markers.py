from pathlib import Path

path = Path("scripts/tmp_apply_provider_canonical_outcome.py")
text = path.read_text()

replacements = [
    (
        "cycle.rejection_replays =\n                                    cycle.rejection_replays.saturating_add(1);",
        "cycle.rejection_replays = cycle.rejection_replays.saturating_add(1);",
        2,
    ),
    (
        "'''#[derive(Debug)]\nenum DeliveryDisposition {",
        "'''#[derive(Debug, Clone, PartialEq)]\nenum DeliveryDisposition {",
        1,
    ),
]

for old, new, expected_count in replacements:
    count = text.count(old)
    if count != expected_count:
        raise SystemExit(
            "canonical-outcome marker count mismatch: "
            f"expected {expected_count}, found {count}: {old!r}"
        )
    text = text.replace(old, new)

path.write_text(text)
