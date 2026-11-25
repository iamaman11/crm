// Check the current CRM wire contract and frontend with the same entry point as CI.
import {spawnSync} from 'node:child_process';
import {mkdtempSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {performance} from 'node:perf_hooks';

const root = fileURLToPath(new URL('../../../', import.meta.url));
const temporary = mkdtempSync(join(tmpdir(), 'crm-web-check-'));
const report = process.env.CRM_MODULE_COVERAGE_REPORT ?? join(temporary, 'modules.json');
const image = process.env.CRM_PROTO_DESCRIPTOR_FILE ?? join(temporary, 'crm.binpb');
const env = {...process.env, CRM_MODULE_COVERAGE_REPORT: report, CRM_PROTO_DESCRIPTOR_FILE: image};
const pnpm = 'pnpm';
const node = process.execPath;

function execute(command, args, capture = false) {
  const result = spawnSync(command, args, {cwd: root, env, encoding: 'utf8',
    stdio: capture ? ['ignore', 'pipe', 'pipe'] : 'inherit'});
  if (result.error) throw result.error;
  if (result.status !== 0) {
    if (capture && result.stderr) process.stderr.write(result.stderr);
    throw new Error(`${command} ${args.join(' ')} failed: ${result.signal ?? result.status}`);
  }
  return result.stdout;
}

function run(command, ...args) {
  const started = performance.now();
  try {
    execute(command, args);
  } finally {
    console.log(JSON.stringify({event: 'crm.web-check.phase', command: [command, ...args],
      elapsed_ms: Math.round(performance.now() - started)}));
  }
}

// TypeScript itself resolves inherited options. A green tsc with noCheck is insufficient.
function checkTsconfig(project) {
  const options = JSON.parse(execute(pnpm, ['exec', 'tsc', '--project', project, '--showConfig'], true)).compilerOptions;
  const required = ['strict', 'noUncheckedIndexedAccess', 'exactOptionalPropertyTypes',
    'isolatedModules', 'verbatimModuleSyntax'];
  const strictFamily = ['noImplicitAny', 'noImplicitThis', 'strictNullChecks', 'strictFunctionTypes',
    'strictBindCallApply', 'strictPropertyInitialization', 'strictBuiltinIteratorReturn',
    'useUnknownInCatchVariables', 'alwaysStrict'];
  const disabled = [...required.filter(key => options[key] !== true),
    ...strictFamily.filter(key => options[key] === false),
    ...['noCheck', 'allowJs', 'checkJs'].filter(key => options[key] === true)];
  if (disabled.length) throw new Error(`CRM_TS_CONFIG_VIOLATION ${project}: ${disabled.join(', ')}`);
}

function main(args) {
  if (args.length) {
    if (args.length !== 2 || args[0] !== '--verify-tsconfig') {
      throw new Error('expected --verify-tsconfig <tsconfig path>');
    }
    checkTsconfig(args[1]);
    return;
  }
  for (const project of ['packages/client/tsconfig.json', 'packages/ui/tsconfig.json',
    'apps/cloudflare-crm/tsconfig.json']) checkTsconfig(project);
  // Transitional local-only producer until the canonical owner report is migrated.
  // CI already provides this report; do not regenerate it.
  if (process.env.CRM_MODULE_COVERAGE_REPORT === undefined) {
    run('python', 'scripts/crm-module-coverage.py', '--output', report);
  }
  run(pnpm, 'exec', 'buf', 'format', '-d', '--exit-code', 'proto');
  run(pnpm, 'exec', 'buf', 'lint', 'proto');
  run(pnpm, 'exec', 'buf', 'build', 'proto', '-o', image);
  // The persisted v1 wire contract is distinct from the evolving development API.
  run(pnpm, 'exec', 'buf', 'breaking', '.',
    '--against', '.git#ref=2c5f20eeb4185d16d2edec957965d8d50ec3d756',
    '--path', 'proto/crm/core/v1/common.proto',
    '--path', 'proto/crm/activities/v1/task.proto',
    '--path', 'proto/crm/sales/v1/deal.proto');
  run(pnpm, 'web:generate', image);
  run(node, 'scripts/crm-rpc-descriptors.mjs', image);
  run(node, '--test', 'scripts/crm-core-v2.test.mjs', 'scripts/crm-cutover-contracts.test.mjs',
    'scripts/crm-rpc-descriptors.test.mjs', 'scripts/crm-clean-schema.test.mjs',
    'apps/cloudflare-crm/test/architecture-lint.test.mjs');
  run(pnpm, 'web:lint');
  run(pnpm, '--recursive', '--filter', 'cloudflare-crm...', '--fail-if-no-match',
    '--workspace-concurrency=2', 'exec', 'tsc', '-p', 'tsconfig.json', '--noEmit');
  run(pnpm, 'exec', 'vitest', 'run', 'packages/client/src/cloudflare-v2-contract.test.ts',
    'apps/cloudflare-crm/test/client-routes.test.mjs',
    'apps/cloudflare-crm/test/automation-route.test.mjs');
  run(pnpm, '--filter', 'cloudflare-crm', 'test');
  run(pnpm, '--filter', 'cloudflare-crm', 'build');
}

try {
  main(process.argv.slice(2));
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
} finally {
  rmSync(temporary, {recursive: true, force: true});
}
