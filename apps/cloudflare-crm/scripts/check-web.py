"""Check the current CRM wire contract and frontend with the same entry point as CI."""
import os
from pathlib import Path
import shutil
import subprocess
import sys
from tempfile import TemporaryDirectory

root = Path(__file__).resolve().parents[3]
pnpm = shutil.which("pnpm")
node = shutil.which("node")
if pnpm is None or node is None:
    raise SystemExit("pnpm and Node are required")

with TemporaryDirectory() as temporary:
    report = os.environ.get("CRM_MODULE_COVERAGE_REPORT", str(Path(temporary) / "modules.json"))
    image = os.environ.get("CRM_PROTO_DESCRIPTOR_FILE", str(Path(temporary) / "crm.binpb"))
    env = {**os.environ, "CRM_MODULE_COVERAGE_REPORT": report, "CRM_PROTO_DESCRIPTOR_FILE": image}

    def run(*args):
        subprocess.run(args, cwd=root, env=env, check=True)

    if "CRM_MODULE_COVERAGE_REPORT" not in os.environ:
        run(sys.executable, "scripts/crm-module-coverage.py", "--output", report)
    run(pnpm, "exec", "buf", "format", "-d", "--exit-code", "proto")
    run(pnpm, "exec", "buf", "lint", "proto")
    run(pnpm, "exec", "buf", "build", "proto", "-o", image)
    run(pnpm, "web:generate", image)
    run(node, "scripts/crm-rpc-descriptors.mjs", image)
    run(node, "--test", "scripts/crm-core-v2.test.mjs", "scripts/crm-cutover-contracts.test.mjs",
        "scripts/crm-rpc-descriptors.test.mjs", "scripts/crm-clean-schema.test.mjs",
        "apps/cloudflare-crm/test/architecture-lint.test.mjs")
    run(pnpm, "web:lint")
    for package in ["@ultimate-crm/client", "@ultimate-crm/ui", "cloudflare-crm"]:
        run(pnpm, "--filter", package, "typecheck")
    run(pnpm, "exec", "vitest", "run", "packages/client/src/cloudflare-v2-contract.test.ts",
        "apps/cloudflare-crm/test/client-routes.test.mjs")
    run(pnpm, "--filter", "cloudflare-crm", "test")
    run(pnpm, "--filter", "cloudflare-crm", "build")
