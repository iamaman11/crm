// One CRM workflow always tests the full product. Packet admission and unknown
// path ownership remain fail-closed; no custom calculation of test selection.
import assert from "node:assert/strict";
import {execFileSync} from "node:child_process";
import {readFileSync} from "node:fs";
import {dirname, isAbsolute, posix, relative, resolve, sep} from "node:path";
import {fileURLToPath} from "node:url";
import {record, records, string, strings} from "./json-boundary.mts";
import {verifyWorkflowInventory} from "./workflow-policy.mts";

const repository = fileURLToPath(new URL("../../../", import.meta.url));
const workflowName = "Cloudflare CRM Spike CI";
const requiredScopes = ["contracts", "protobuf_api_compatibility", "d1_migrations",
  "d1_runtime_acceptance", "cloudflare_product_plane", "frontend", "operations"].sort();
// The old broadening rules also classified infrastructure paths. Preserve that
// classification, not the now-unused reverse dependency calculation.
const infrastructure = ["Cargo.toml", "Cargo.lock", ".github/workflows/**", "affected-scope-policy.json",
  "architecture-policy.json", "architecture-governance.json", "workspace-dependency-policy.json",
  "requirements-dev.txt", "scripts/affected_scope.py", "scripts/repo.py", "scripts/repository_navigation.py",
  "scripts/check_architecture.py", "scripts/check_ci_event_policy.py", "scripts/analyze_workspace.py"];

function command(root: string, executable: string, args: string[]): string {
  return execFileSync(executable, args, {cwd: root, encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024, stdio: ["ignore", "pipe", "inherit"]});
}
function json(root: string, path: string): Record<string, unknown> {
  return record(JSON.parse(readFileSync(resolve(root, path), "utf8")));
}
function text(value: unknown): string {
  const result = string(value).trim();
  assert(result, "Expected nonempty policy string");
  return result;
}
function list(value: unknown): string[] {
  const result = strings(value).map(text);
  assert(result.length && new Set(result).size === result.length, "Expected nonempty unique policy list");
  return result;
}
function within(root: string, path: string): string {
  const result = relative(root, path);
  assert(!isAbsolute(result) && result !== ".." && !result.startsWith(".." + sep), "Cargo package outside repository");
  return result.split(sep).join("/");
}
export function pathMatches(path: string, pattern: string): boolean {
  assert(!pattern.startsWith("!") && !pattern.includes("\\"), "Unsupported packet glob: " + pattern);
  return posix.matchesGlob(path, pattern);
}
const matches = (path: string, patterns: readonly string[]) => patterns.some(pattern => pathMatches(path, pattern));

export function scopeClassifier(root: string, metadata: unknown, scopePolicy: unknown) {
  const policy = record(scopePolicy);
  assert.equal(policy.schema_version, "crm.affected-scope-policy/v1");
  const rules = records(policy.scopes).map(rule => ({
    id: text(rule.id), owner: text(rule.owner), patterns: list(rule.path_patterns),
    workflows: list(rule.required_workflows),
  })).sort((a,b) => a.id < b.id ? -1 : a.id > b.id ? 1 : 0);
  assert.deepEqual(rules.map(rule => rule.id), requiredScopes, "invalid scope categories");
  // Every declared scope, even if unchanged, must resolve to the actual sole CI.
  for (const rule of rules) assert.deepEqual(rule.workflows, [workflowName],
    "scope " + rule.id + " requires missing required CI workflows");
  const graph = record(metadata);
  const members = new Set(strings(graph.workspace_members));
  const packages = records(graph.packages).filter(pkg => members.has(string(pkg.id))).map(pkg => ({
    name: text(pkg.name), directory: dirname(within(root, string(pkg.manifest_path))).replace(/\\/g,"/"),
  }));
  assert.equal(packages.length, members.size, "Cargo workspace metadata missing packages");
  assert.equal(new Set(packages.map(pkg => pkg.name)).size, packages.length, "duplicate workspace package names");
  const governed = [...rules.flatMap(rule => rule.patterns), ...list(policy.neutral_path_patterns), ...infrastructure];
  return (path: string) => ({
    known: packages.some(pkg => path === pkg.directory || path.startsWith(pkg.directory + "/")) || matches(path, governed),
    scopes: rules.filter(rule => matches(path,rule.patterns)).map(rule => rule.id),
  });
}

export function packetClassifier(packetValue: unknown, baseRef: string) {
  const packet = record(packetValue);
  assert.equal(packet.schema_version, "crm.repository-packet/v1");
  assert.equal(packet.packet_id, "cloudflare-crm-development");
  for (const key of ["packet_id","title","status","objective"]) text(packet[key]);
  assert.deepEqual(record(packet.baseline), {ref:"dev"}, "only canonical dev baseline is permitted");
  assert(!Object.hasOwn(packet, "promotion_baseline"), "stale promotion baseline");
  assert(["dev","origin/dev","refs/heads/dev"].includes(baseRef), "unrecognized packet base");
  assert(Array.isArray(packet.tracking_issues) && packet.tracking_issues.length &&
    packet.tracking_issues.every(value=>Number.isSafeInteger(value)&&value>0), "invalid tracking issues");
  const allowed = list(packet.allowed_paths), forbidden = list(packet.forbidden_paths);
  for (const key of ["deliverables","required_checks","acceptance","non_goals"]) list(packet[key]);
  assert(!allowed.some(pattern=>forbidden.includes(pattern)), "allowed and forbidden paths overlap");
  return (path: string) => path.toLowerCase().endsWith(".md") ? "documentation" :
    matches(path,forbidden) ? "forbidden" : matches(path,allowed) ? "allowed" : "outside";
}

export function verifyPacketPaths(packetValue: unknown, paths: string[], baseRef: string): void {
  const classify = packetClassifier(packetValue,baseRef);
  for (const path of paths) {
    const decision = classify(path);
    assert(decision !== "forbidden", "forbidden path changed: " + path);
    assert(decision !== "outside", "path outside packet: " + path);
  }
}

export function checkPacket(root: string, before: string, baseRef = "origin/dev", workspace?: unknown) {
  assert(/^[a-f0-9]{40}$/.test(before), "invalid push base SHA");
  const head = command(root, "git", ["rev-parse","HEAD"]).trim();
  const paths = (before === "0".repeat(40) ?
    command(root,"git",["ls-tree","-r","--name-only","-z","HEAD"]) :
    command(root,"git",["diff","--no-renames","--name-only","-z",before,head])).split("\0").filter(Boolean).sort();
  verifyWorkflowInventory(root);
  verifyPacketPaths(json(root,"repository-packet.json"), paths, baseRef);
  const metadata: unknown = workspace ?? JSON.parse(command(root,"cargo",["metadata","--locked","--format-version","1","--no-deps"]));
  const classify = scopeClassifier(root,metadata,json(root,"affected-scope-policy.json"));
  const codePaths = paths.filter(path=>!path.toLowerCase().endsWith(".md"));
  const unknown = codePaths.filter(path=>!classify(path).known);
  assert(!unknown.length, "unknown affected scope: " + unknown.join(", "));
  return {event:"crm.packet-check",result:"PASS",base_sha:before,head_sha:head,changed_paths:paths,
    selected_scopes:[...new Set(codePaths.flatMap(path=>classify(path).scopes))].sort()};
}
if (import.meta.main) {
  const before = process.argv[2];
  assert(before && process.argv.length===3, "expected push before SHA");
  console.log(JSON.stringify(checkPacket(repository,before)));
}
