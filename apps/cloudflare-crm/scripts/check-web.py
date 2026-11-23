"""Run the active CRM frontend checks with the same workspace tools as CI."""
import os
from pathlib import Path
import shutil
import subprocess
import sys
from tempfile import TemporaryDirectory

root = Path(__file__).resolve().parents[3]
pnpm = shutil.which("pnpm")
if pnpm is None:
    raise SystemExit("pnpm is required")

with TemporaryDirectory() as temporary:
    report = str(Path(temporary) / "module-coverage.json")
    env = {**os.environ, "CRM_MODULE_COVERAGE_REPORT": report}
    subprocess.run([sys.executable, "scripts/crm-module-coverage.py", "--output", report],
                   cwd=root, env=env, check=True)
    for args in (["gen"], ["exec", "node", "scripts/crm-architecture-ast.mjs"],
                 ["--filter", "@ultimate-crm/client", "typecheck"],
                 ["--filter", "@ultimate-crm/ui", "typecheck"],
                 ["--filter", "cloudflare-crm", "typecheck"],
                 ["exec", "vitest", "run", "packages/client/src/cloudflare-v2-contract.test.ts"],
                 ["--filter", "cloudflare-crm", "test"], ["--filter", "cloudflare-crm", "build"]):
        subprocess.run([pnpm, *args], cwd=root, env=env, check=True)
