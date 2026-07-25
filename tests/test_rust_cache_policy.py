from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from scripts.check_rust_cache_policy import (
    CACHE_ACTION_SHA,
    check_rust_cache_policy,
)


VALID_WORKFLOW = f"""name: Rust CI

jobs:
  quality:
    steps:
      - name: Resolve Rust cache identity
        id: rust-cache-identity
        run: echo toolchain=test >> "$GITHUB_OUTPUT"
      - name: Restore trusted Rust CI cache
        id: rust-cache-restore
        uses: actions/cache/restore@{CACHE_ACTION_SHA} # v5.0.3
        with:
          path: |
            ~/.cargo/registry/index/
            ~/.cargo/registry/cache/
            ~/.cargo/git/db/
            target/
          key: rust-quality-v1-${{{{ runner.os }}}}-${{{{ runner.arch }}}}-${{{{ steps.rust-cache-identity.outputs.toolchain }}}}-${{{{ hashFiles('Cargo.lock') }}}}
          restore-keys: |
            rust-quality-v1-${{{{ runner.os }}}}-${{{{ runner.arch }}}}-${{{{ steps.rust-cache-identity.outputs.toolchain }}}}-
      - name: Report Rust cache restore
        run: |
          echo "${{{{ steps.rust-cache-restore.outputs.cache-hit }}}}"
          echo "${{{{ steps.rust-cache-restore.outputs.cache-matched-key }}}}"
      - name: Run Clippy
        run: cargo clippy
      - name: Run workspace tests
        run: cargo test
      - name: Save trusted Rust CI cache
        if: github.event_name == 'push' && github.ref == 'refs/heads/main' && success() && steps.rust-cache-restore.outputs.cache-hit != 'true'
        uses: actions/cache/save@{CACHE_ACTION_SHA} # v5.0.3
        with:
          path: |
            ~/.cargo/registry/index/
            ~/.cargo/registry/cache/
            ~/.cargo/git/db/
            target/
          key: ${{{{ steps.rust-cache-restore.outputs.cache-primary-key }}}}
"""


class RustCachePolicyTests(unittest.TestCase):
    def check(self, text: str):
        with TemporaryDirectory() as temporary:
            path = Path(temporary) / "rust.yml"
            path.write_text(text, encoding="utf-8")
            return check_rust_cache_policy(path)

    def test_accepts_main_write_only_lockfile_bound_cache(self) -> None:
        self.assertEqual(self.check(VALID_WORKFLOW), ())

    def test_rejects_save_without_main_only_condition(self) -> None:
        failures = self.check(
            VALID_WORKFLOW.replace(
                "if: github.event_name == 'push' && github.ref == 'refs/heads/main' && success() && steps.rust-cache-restore.outputs.cache-hit != 'true'",
                "if: success()",
            )
        )
        self.assertTrue(any("main-only" in failure.message for failure in failures))

    def test_rejects_credentials_in_cache_paths(self) -> None:
        failures = self.check(
            VALID_WORKFLOW.replace("            target/\n", "            target/\n            ~/.cargo/credentials\n", 1)
        )
        self.assertTrue(any("forbidden cache path" in failure.message for failure in failures))

    def test_rejects_mutable_or_different_cache_action(self) -> None:
        failures = self.check(VALID_WORKFLOW.replace(CACHE_ACTION_SHA, "a" * 40, 1))
        self.assertTrue(any("immutable restore Action" in failure.message for failure in failures))


if __name__ == "__main__":
    unittest.main()
