// Cargo owns dependency identity and feature resolution; this check consumes
// its native JSON output and the existing canonical module report.
import assert from "node:assert/strict";
import {record, records, string, strings} from "./json-boundary.mts";
import {execFileSync} from "node:child_process";
import {readFileSync, writeFileSync} from "node:fs";
import {dirname, resolve} from "node:path";
import {fileURLToPath} from "node:url";

const root = fileURLToPath(new URL("../../../", import.meta.url));
const worker = resolve(root, "apps/cloudflare-crm/rust-party");

function indexed(rows: Record<string, unknown>[]): Map<string, Record<string, unknown>> {
  const result = new Map<string, Record<string, unknown>>();
  for (const row of rows) {
    const id = string(row.id);
    assert(!result.has(id), "Duplicate Cargo ID: " + id);
    result.set(id, row);
  }
  return result;
}

function graph(value: unknown) {
  const metadata = record(value);
  const resolution = record(metadata.resolve);
  const packages = indexed(records(metadata.packages));
  const nodes = indexed(records(resolution.nodes));
  return {packages, nodes, root: string(resolution.root)};
}

export function ownerPlan(report: unknown, metadata: unknown, repository: string, workerPath: string): Map<string, string[]> {
  const {packages, nodes, root: rootId} = graph(metadata);
  const workerPackage = packages.get(rootId);
  const workerNode = nodes.get(rootId);
  assert(workerPackage && workerNode, "Missing resolved Worker package");
  assert.equal(dirname(string(workerPackage.manifest_path)), workerPath, "Wrong Cargo metadata root");
  const linkedIds = new Set(strings(workerNode.dependencies));
  const dependencies = new Map<string, Record<string, unknown>>();
  for (const dep of records(workerPackage.dependencies)) {
    // Match the former direct [dependencies] path inventory, not build/dev or target-specific edges.
    if (dep.path === null || dep.path === undefined || dep.kind !== null || dep.target !== null) continue;
    const path = resolve(repository, string(dep.path));
    assert(!dependencies.has(path), "Duplicate normal Worker path dependency: " + path);
    dependencies.set(path, dep);
  }
  const modules = records(record(report).modules);
  const linked = modules.filter(module => {
    assert(typeof module.linked === "boolean", "Invalid module linkage evidence");
    return module.linked;
  });
  assert(linked.length > 0, "No linked original owners");
  // Cargo's consumer builds do not execute dependency-owned tests.
  const owners = new Map<string, string[]>([["crm-core-contracts", []], ["crm-module-sdk", []]]);
  for (const module of linked) {
    const path = resolve(repository, string(module.crate_path));
    const name = string(module.crate_name);
    if (path === workerPath) {
      assert.equal(workerPackage.name, name, "Worker composition package mismatch");
      continue;
    }
    const dep = dependencies.get(path);
    assert(dep && dep.name === name, "Missing declared normal Worker owner dependency: " + name);
    const resolved = Array.from(linkedIds, id => packages.get(id)).filter(pkg =>
      pkg?.name === name && dirname(string(pkg.manifest_path)) === path);
    assert.equal(resolved.length, 1, "Missing or ambiguous resolved Worker owner: " + name);
    assert(typeof dep.uses_default_features === "boolean", "Missing Cargo default-feature flag: " + name);
    const flags: string[] = [];
    if (!dep.uses_default_features) flags.push("--no-default-features");
    const features = strings(dep.features);
    if (features.length) flags.push("--features", features.join(","));
    const previous = owners.get(name);
    // Shared foundations are also tested with their declared owner flags when linked.
    if (previous?.length) assert.deepEqual(previous, flags, "Conflicting owner features: " + name);
    owners.set(name, flags);
  }
  return owners;
}

export function verifyContractGraphs(production: unknown, nativeCi: unknown) {
  function evidence(value: unknown) {
    const {packages, nodes} = graph(value);
    function unique(name: string) {
      const ids = [...packages].filter(([, pkg]) => pkg.name === name).map(([id]) => id);
      assert.equal(ids.length, 1, "Expected one Cargo package: " + name);
      const id = ids[0];
      assert(id !== undefined);
      const node = nodes.get(id);
      assert(node, "Missing resolved Cargo node: " + name);
      return node;
    }
    const contractFeatures = strings(unique("crm-proto-contracts").features);
    assert.deepEqual(contractFeatures, ["sales-activities-link"],
      "Worker must compile only the original link contract feature");
    const generated = unique("crm-connect-contracts");
    const dependencies = strings(generated.dependencies).map(id => {
      const pkg = packages.get(id);
      assert(pkg, "Unknown generated Connect dependency: " + id);
      return string(pkg.name);
    }).sort();
    assert(!dependencies.some(name => name.startsWith("crm-") || name === "worker"),
      "Generated Connect contracts must not depend on CRM owners or Worker");
    return {features: strings(generated.features), dependencies};
  }
  const generated = evidence(production);
  assert.deepEqual(generated, evidence(nativeCi), "Generated contract graph must match production and native-ci");
  return generated;
}

function cargo(args: string[], capture = false): string {
  return execFileSync("cargo", args, {cwd: root, encoding: "utf8",
    stdio: capture ? ["ignore", "pipe", "inherit"] : "inherit", maxBuffer: 64 * 1024 * 1024}) ?? "";
}
function metadata(features: string): unknown {
  return JSON.parse(cargo(["metadata", "--locked", "--format-version", "1",
    "--manifest-path", resolve(worker, "Cargo.toml"),
    "--filter-platform", "wasm32-unknown-unknown", "--features", features], true));
}

function main(): void {
  const reportPath = process.env.CRM_MODULE_COVERAGE_REPORT;
  assert(reportPath, "Canonical module coverage report required");
  const report: unknown = JSON.parse(readFileSync(reportPath, "utf8"));
  // Preserve both existing metadata invocations and use the production result
  // for owner planning too, rather than independently parsing Cargo.toml.
  const metadataPath = process.env.CRM_PRODUCTION_METADATA_FILE;
  const production: unknown = metadataPath === undefined ? metadata("native") :
    JSON.parse(readFileSync(metadataPath, "utf8"));
  const owners = ownerPlan(report, production, root, worker);
  cargo(["test", "--locked", "--workspace", "--all-targets"]);
  cargo(["test", "--locked", "-p", "crm-proto-contracts", "--no-default-features",
    "--features", "sales-activities-link", "--lib"]);
  const generated = verifyContractGraphs(production, metadata("native-ci"));
  console.log(JSON.stringify({event: "crm.worker-contract-features", result: "PASS", features: ["sales-activities-link"]}));
  console.log(JSON.stringify({event: "crm.generated-connect-graph", result: "PASS", ...generated}));
  const evidence: Record<string, {feature_flags: string[]; normal_build_dev: string}> = {};
  for (const [owner, flags] of [...owners].sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0)) {
    if (flags.length) cargo(["test", "--locked", "-p", owner, "--all-targets", ...flags]);
    evidence[owner] = {feature_flags: flags, normal_build_dev: cargo(["tree", "--locked", "-p", owner,
      ...flags, "--edges", "normal,build,dev", "--charset", "ascii"], true)};
  }
  writeFileSync("/tmp/crm-native-owner-dependency-trees.json", JSON.stringify(evidence), "utf8");
}

if (import.meta.main) main();
