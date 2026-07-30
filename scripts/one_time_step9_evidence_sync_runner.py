from __future__ import annotations

from pathlib import Path


path = Path("scripts/one_time_step9_evidence_sync.py")
text = path.read_text(encoding="utf-8")

old = "flags=re.MULTILINE)"
new = "flags=re.MULTILINE | re.DOTALL)"
if text.count(old) != 1:
    raise SystemExit(f"replace_regex flags patch count: {text.count(old)}")
text = text.replace(old, new)

old = '''    text = replace_exact(
        text,
        "Repository step 9 is affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks.",
        "Repository step 10 is governed Customer Privacy access/export assembly.",
        "status next packet",
    )
    text = replace_exact(
        text,
        "Repository step 10 is governed Customer Privacy access/export assembly.",
        "Repository step 11 is owner-specific deletion, anonymization and supported crypto-shred execution.",
        "status following packet",
    )
'''
new = '''    text = replace_exact(
        text,
        "## Next permitted repository packet\\n\\nRepository step 9 is affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks.\\n\\n## Following permitted repository packet\\n\\nRepository step 10 is governed Customer Privacy access/export assembly.",
        "## Next permitted repository packet\\n\\nRepository step 10 is governed Customer Privacy access/export assembly.\\n\\n## Following permitted repository packet\\n\\nRepository step 11 is owner-specific deletion, anonymization and supported crypto-shred execution.",
        "status next and following packets",
    )
'''
if text.count(old) != 1:
    raise SystemExit(f"status packet patch count: {text.count(old)}")
text = text.replace(old, new)

old = '''        "phase8 accepted step 9",
    )
'''
new = '''        "phase8 accepted step 9",
        expected=2,
    )
'''
if text.count(old) != 1:
    raise SystemExit(f"phase8 cardinality patch count: {text.count(old)}")
text = text.replace(old, new)

path.write_text(text, encoding="utf-8")
exec(compile(text, str(path), "exec"), {"__name__": "__main__", "__file__": str(path)})
