// Native command entry point: only shared artifact lifetime and unique fail-closed checks.
import {execFileSync} from "node:child_process";
import {isRecord} from "./json-boundary.mts";
import {globSync, mkdtempSync, rmSync} from "node:fs";
import {tmpdir} from "node:os";
import {join} from "node:path";
import {fileURLToPath} from "node:url";
import {performance} from "node:perf_hooks";

const root = fileURLToPath(new URL("../../../", import.meta.url));
const node = process.execPath;

function checkTsconfig(project: string): void {
  const output = execFileSync("pnpm", ["exec", "tsc", "--project", project, "--showConfig"],
    {cwd: root, encoding: "utf8"});
  const config: unknown = JSON.parse(output);
  if (!isRecord(config) || !isRecord(config.compilerOptions)) {
    throw new Error("CRM_TS_CONFIG_VIOLATION " + project + ": missing compilerOptions");
  }
  const options = config.compilerOptions;
  const required = ["strict", "noUncheckedIndexedAccess", "exactOptionalPropertyTypes",
    "isolatedModules", "verbatimModuleSyntax"];
  const strictFamily = ["noImplicitAny", "noImplicitThis", "strictNullChecks", "strictFunctionTypes",
    "strictBindCallApply", "strictPropertyInitialization", "strictBuiltinIteratorReturn",
    "useUnknownInCatchVariables", "alwaysStrict"];
  const disabled = [...required.filter(key => options[key] !== true),
    ...strictFamily.filter(key => options[key] === false),
    ...["noCheck", "allowJs", "checkJs"].filter(key => options[key] === true)];
  if (disabled.length) throw new Error(`CRM_TS_CONFIG_VIOLATION ${project}: ${disabled.join(", ")}`);
}

// The native runner accepts an empty glob successfully; require a nonempty
// match for each active scope, then delegate execution to node --test.
function runNodeTests(patterns: string[]): void {
  if (!patterns.length) throw new Error("CRM_TEST_SCOPE_EMPTY: no patterns");
  const files = new Set<string>();
  for (const pattern of patterns) {
    const matched = globSync(pattern);
    if (!matched.length) throw new Error("CRM_TEST_SCOPE_EMPTY: " + pattern);
    for (const file of matched) files.add(file);
  }
  execFileSync(node, ["--test", ...Array.from(files).sort()], {stdio: "inherit"});
}

function checkWeb(): void {
  for (const project of ["packages/client/tsconfig.json", "packages/ui/tsconfig.json",
    "apps/cloudflare-crm/tsconfig.json", "apps/cloudflare-crm/tsconfig.tools.json",
    "apps/cloudflare-crm/tsconfig.native-tests.json"]) checkTsconfig(project);
  const temporary = mkdtempSync(join(tmpdir(), "crm-web-check-"));
  try {
    const report = process.env.CRM_MODULE_COVERAGE_REPORT ?? join(temporary, "modules.json");
    const image = process.env.CRM_PROTO_DESCRIPTOR_FILE ?? join(temporary, "crm.binpb");
    const env = {...process.env, CRM_MODULE_COVERAGE_REPORT: report, CRM_PROTO_DESCRIPTOR_FILE: image};
    function run(command: string, ...args: string[]): void {
      const started = performance.now();
      try {
        execFileSync(command, args, {cwd: root, env, stdio: "inherit"});
      } finally {
        console.log(JSON.stringify({event: "crm.web-check.phase", command: [command, ...args],
          elapsed_ms: Math.round(performance.now() - started)}));
      }
    }
    // CI provides the canonical report once; local development uses the same producer.
    if (process.env.CRM_MODULE_COVERAGE_REPORT === undefined) {
      run("python", "scripts/crm-module-coverage.py", "--output", report);
    }
    run("pnpm", "web:contract:lint");
    run("pnpm", "exec", "buf", "build", "proto", "-o", image);
    run("pnpm", "web:contract:compatibility");
    run("pnpm", "web:generate", image);
    run(node, "scripts/crm-rpc-descriptors.mjs", image);
    run("pnpm", "web:test:contracts");
    run("pnpm", "web:lint");
    run("pnpm", "web:typecheck:active");
    run("pnpm", "web:test:client");
    run("pnpm", "--filter", "cloudflare-crm", "test");
    run("pnpm", "--filter", "cloudflare-crm", "build");
  } finally {
    rmSync(temporary, {recursive: true, force: true});
  }
}

try {
  const [mode, ...args] = process.argv.slice(2);
  if (mode === undefined) checkWeb();
  else if (mode === "--node-tests") runNodeTests(args);
  else if (mode === "--verify-tsconfig" && args.length === 1 && args[0] !== undefined) checkTsconfig(args[0]);
  else throw new Error("expected --verify-tsconfig <path> or --node-tests <glob...>");
} catch (error: unknown) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}
