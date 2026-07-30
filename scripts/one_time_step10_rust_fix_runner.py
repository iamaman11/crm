from __future__ import annotations

from pathlib import Path

path = Path("scripts/one_time_step10_rust_fix.py")
text = path.read_text(encoding="utf-8")
old = '''replace_exact(
    postgres_test_path,
    "        privacy_case.version(),\\n",
    "        i64::try_from(privacy_case.version()).expect(\\\"privacy case version fits i64\\\"),\\n",
    "privacy case fixture version conversion",
)
'''
new = '''replace_exact(
    postgres_test_path,
    """        CASE_ID,
        privacy_case.version(),
        privacy_case_persisted_payload(&privacy_case).expect("encode access case"),
""",
    """        CASE_ID,
        i64::try_from(privacy_case.version()).expect("privacy case version fits i64"),
        privacy_case_persisted_payload(&privacy_case).expect("encode access case"),
""",
    "privacy case persistence version conversion",
)
'''
if text.count(old) != 1:
    raise SystemExit(f"fixture version scope patch count: {text.count(old)}")
text = text.replace(old, new)
path.write_text(text, encoding="utf-8")
exec(compile(text, str(path), "exec"), {"__name__": "__main__", "__file__": str(path)})
