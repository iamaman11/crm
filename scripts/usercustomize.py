"""Temporary diagnostic patch for the Step 18 evidence materializer."""

from __future__ import annotations

from pathlib import Path


path = Path(__file__).with_name("temporary_step18_seed_demo_smoke_evidence_sync.py")
content = path.read_text(encoding="utf-8")

old_verify = '''    combined = "\\n".join(read(path) for path in NORMATIVE)
    for stale in (
        "seed-demo/smoke next",
        "next permitted bounded implementation packet is `seed-demo` and `smoke`",
        "Repository Step 19 remains blocked",
    ):
        if stale in combined:
            raise RuntimeError(f"stale Step 18 claim remains: {stale}")
'''
new_verify = '''    for document_path in NORMATIVE:
        document = read(document_path)
        for stale in (
            "seed-demo/smoke next",
            "next permitted bounded implementation packet is `seed-demo` and `smoke`",
            "Repository Step 19 remains blocked",
        ):
            if stale in document:
                raise RuntimeError(
                    f"{document_path}: stale Step 18 claim remains: {stale}"
                )
'''
count = content.count(old_verify)
if count != 1:
    raise RuntimeError(f"verify diagnostic patch: expected 1 match, found {count}")
content = content.replace(old_verify, new_verify, 1)

old_main = '''    update_project_status()
    update_product_plan()
    update_complexity()
'''
new_main = '''    update_project_status()
    update_product_plan()
    subprocess.run(
        ["git", "add", "docs/PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md"],
        cwd=ROOT,
        check=True,
    )
    update_complexity()
'''
count = content.count(old_main)
if count != 1:
    raise RuntimeError(f"product-plan staging patch: expected 1 match, found {count}")
content = content.replace(old_main, new_main, 1)

path.write_text(content, encoding="utf-8")
