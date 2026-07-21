from pathlib import Path

path = Path("scripts/tmp_apply_provider_canonical_outcome.py")
text = path.read_text()

replacements = [
    (
        "cycle.rejection_replays =\n                                    cycle.rejection_replays.saturating_add(1);",
        "cycle.rejection_replays = cycle.rejection_replays.saturating_add(1);",
    ),
    (
        "'''#[derive(Debug)]\nenum DeliveryDisposition {",
        "'''#[derive(Debug, Clone, PartialEq)]\nenum DeliveryDisposition {",
    ),
]

for old, new in replacements:
    count = text.count(old)
    if count != 1:
        raise SystemExit(
            f"expected exactly one canonical-outcome marker, found {count}: {old!r}"
        )
    text = text.replace(old, new, 1)

path.write_text(text)
