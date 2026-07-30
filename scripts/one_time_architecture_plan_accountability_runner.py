from __future__ import annotations

from pathlib import Path
import runpy


path = Path(__file__).with_name("one_time_architecture_plan_accountability.py")
text = path.read_text(encoding="utf-8")
old = '''replace_exact(
    "tests/test_repository_navigation.py",
    '"19232f6f3e2ae87aabeb080257c1aac5477a6616",',
    '"dad639c7d269bc802d053f1d99cf0fbf466ce4fb",',
    expected=2,
)
'''
new = '''replace_exact(
    "tests/test_repository_navigation.py",
    '"19232f6f3e2ae87aabeb080257c1aac5477a6616",',
    '"dad639c7d269bc802d053f1d99cf0fbf466ce4fb",',
)
replace_exact(
    "tests/test_repository_navigation.py",
    '                    "19232f6f3e2ae87aabeb080257c1aac5477a6616"\n                ),',
    '                    "dad639c7d269bc802d053f1d99cf0fbf466ce4fb"\n                ),',
)
'''
if text.count(old) != 1:
    raise SystemExit("expected one baseline replacement block")
path.write_text(text.replace(old, new), encoding="utf-8")
runpy.run_path(str(path), run_name="__main__")
