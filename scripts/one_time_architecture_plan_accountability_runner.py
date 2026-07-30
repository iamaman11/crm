from __future__ import annotations

from pathlib import Path
import runpy


materializer = Path(__file__).with_name("one_time_architecture_plan_accountability.py")
text = materializer.read_text(encoding="utf-8")
block = '''replace_exact(
    "tests/test_repository_navigation.py",
    '"19232f6f3e2ae87aabeb080257c1aac5477a6616",',
    '"dad639c7d269bc802d053f1d99cf0fbf466ce4fb",',
    expected=2,
)
'''
if text.count(block) != 1:
    raise SystemExit("expected one obsolete baseline replacement block")
materializer.write_text(text.replace(block, ""), encoding="utf-8")

navigation_test = materializer.parents[1] / "tests/test_repository_navigation.py"
test_text = navigation_test.read_text(encoding="utf-8")
old_sha = "19232f6f3e2ae87aabeb080257c1aac5477a6616"
new_sha = "dad639c7d269bc802d053f1d99cf0fbf466ce4fb"
if test_text.count(old_sha) != 2:
    raise SystemExit("expected two navigation-test baseline references")
navigation_test.write_text(test_text.replace(old_sha, new_sha), encoding="utf-8")

runpy.run_path(str(materializer), run_name="__main__")
