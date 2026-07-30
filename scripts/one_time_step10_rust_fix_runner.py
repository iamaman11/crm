from __future__ import annotations

from pathlib import Path

path = Path("scripts/one_time_step10_rust_fix.py")
text = path.read_text(encoding="utf-8")
old = '''    "privacy case fixture version conversion",
)
'''
new = '''    "privacy case fixture version conversion",
    expected=2,
)
'''
if text.count(old) != 1:
    raise SystemExit(f"fixture version cardinality patch count: {text.count(old)}")
text = text.replace(old, new)
path.write_text(text, encoding="utf-8")
exec(compile(text, str(path), "exec"), {"__name__": "__main__", "__file__": str(path)})
