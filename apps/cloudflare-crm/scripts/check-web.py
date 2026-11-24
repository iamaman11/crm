"""Check the current CRM wire contract and frontend with the same entry point as CI."""
import os
import json
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


    # TypeScript --showConfig resolves all inherited compiler options itself.
    # A green tsc can be misleading if noCheck=true is enabled.
    def check_tsconfig(project):
        output = subprocess.run(
            [pnpm, "exec", "tsc", "--project", project, "--showConfig"],
            cwd=root, env=env, text=True, capture_output=True, check=True,
        ).stdout
        options = json.loads(output)["compilerOptions"]
        required = (
            "strict", "noUncheckedIndexedAccess", "exactOptionalPropertyTypes",
            "isolatedModules", "verbatimModuleSyntax",
        )
        strict_family = (
            "noImplicitAny", "noImplicitThis", "strictNullChecks", "strictFunctionTypes",
            "strictBindCallApply", "strictPropertyInitialization",
            "strictBuiltinIteratorReturn", "useUnknownInCatchVariables", "alwaysStrict",
        )
        disabled = [key for key in required if options.get(key) is not True]
        disabled += [key for key in strict_family if options.get(key) is False]
        disabled += [key for key in ("noCheck", "allowJs", "checkJs") if options.get(key) is True]
        if disabled:
            raise SystemExit(f"CRM_TS_CONFIG_VIOLATION {project}: {', '.join(disabled)}")

    if len(sys.argv) > 1:
        if len(sys.argv) != 3 or sys.argv[1] != "--verify-tsconfig":
            raise SystemExit("expected --verify-tsconfig <tsconfig path>")
        check_tsconfig(sys.argv[2])
        raise SystemExit(0)

    for project in (
        "packages/client/tsconfig.json",
        "packages/ui/tsconfig.json",
        "apps/cloudflare-crm/tsconfig.json",
    ):
        check_tsconfig(project)

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
    # pnpm selects the actual CRM workspace dependency closure. Execute tsc
    # directly so a missing package script cannot silently skip typechecking.
    run(pnpm, "--recursive", "--filter", "cloudflare-crm...", "--fail-if-no-match",
        "--workspace-concurrency=2", "exec", "tsc", "-p", "tsconfig.json", "--noEmit")
    run(pnpm, "exec", "vitest", "run", "packages/client/src/cloudflare-v2-contract.test.ts",
        "apps/cloudflare-crm/test/client-routes.test.mjs")
    run(pnpm, "--filter", "cloudflare-crm", "test")
    run(pnpm, "--filter", "cloudflare-crm", "build")
