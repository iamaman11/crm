from __future__ import annotations

from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: found {count}, expected 1")
    return text.replace(old, new)


phase8 = Path("docs/PHASE8_DELIVERY_PLAN.md")
text = phase8.read_text(encoding="utf-8")
text = replace_once(
    text,
    "9. repository step 9 — affected-scope expansion for contracts, migrations, PostgreSQL/process and product checks — **next**;\n10. repository step 10 — governed access/export assembly;",
    "9. repository step 9 — affected-scope expansion for contracts, migrations, PostgreSQL/process and product checks — **complete through PR #239**;\n10. repository step 10 — governed access/export assembly — **next**;",
    "Phase 8 binding sequence",
)
text = replace_once(
    text,
    "The inserted prerequisites did not renumber the normative master sequence. A later step must not start while repository step 9 is unfinished.",
    "The inserted prerequisites did not renumber the normative master sequence. A later step must not start while repository step 10 is unfinished.",
    "Phase 8 blocking sentence",
)
phase8.write_text(text, encoding="utf-8")


test = Path("tests/test_architecture_documentation_consistency.py")
text = test.read_text(encoding="utf-8")
old = '''        self.assertIn("## 9. Binding repository continuation", self.phase8)
        for step in range(1, 6):
'''
new = '''        self.assertIn("## 9. Binding repository continuation", self.phase8)
        self.assertIn(
            "9. repository step 9 — affected-scope expansion for contracts, migrations, PostgreSQL/process and product checks — **complete through PR #239**;",
            self.phase8,
        )
        self.assertIn(
            "10. repository step 10 — governed access/export assembly — **next**;",
            self.phase8,
        )
        self.assertNotIn(
            "9. repository step 9 — affected-scope expansion for contracts, migrations, PostgreSQL/process and product checks — **next**;",
            self.phase8,
        )
        self.assertIn(
            "A later step must not start while repository step 10 is unfinished.",
            self.phase8,
        )
        for step in range(1, 6):
'''
text = replace_once(text, old, new, "Phase 8 sequence guard")
test.write_text(text, encoding="utf-8")
