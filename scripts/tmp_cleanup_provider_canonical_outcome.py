from pathlib import Path

for value in (
    ".github/workflows/tmp-provider-canonical-outcome.yml",
    "scripts/tmp_apply_provider_canonical_outcome.py",
    "scripts/tmp_fix_provider_canonical_outcome_apply_markers.py",
    "scripts/tmp_cleanup_provider_canonical_outcome.py",
):
    path = Path(value)
    if path.exists():
        path.unlink()
