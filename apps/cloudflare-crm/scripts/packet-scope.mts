// Active CRM packet policy. Git and Cargo provide paths and package graph;
// the only deployed workflow executes all checks, never a custom subset.
import assert from "node:assert/strict";
import {execFileSync} from "node:child_process";
import {readFileSync} from "node:fs";
import {dirname, isAbsolute, matchesGlob, relative, resolve, sep} from "node:path";
import {fileURLToPath} from "node:url";
import {record, records, string, strings} from "./json-boundary.mts";
import {verifyWorkflowInventory, canonicalWorkflow} from "./workflow-policy.mts";

const repository = fileURLToPath(new URL("../../../", import.meta.url));
const workflowName = "Cloudflare CRM Spike CI";
const requiredScopes = ["contracts", "protobuf_api_compatibility", "d1_migrations",
  "d1_runtime_acceptance", "cloudflare_product_plane", "frontend", "operations"].sort();
const broad = ["Cargo.toml", "Cargo.lock", ".github/workflows/**", "affected-scope-policy.json",
  "architecture-policy.json", "architecture-governance.json", "workspace-dependency-policy.json",
  "requirements-dev.txt", "scripts/affected_scope.py", "scripts/repo.py", "scripts/repository_navigation.py",
  "scripts/check_architecture.py", "scripts/check_ci_event_policy.py", "scripts/analyze_workspace.py",
  "apps/cloudflare-crm/scripts/packet-scope.mts", "apps/cloudflare-crm/scripts/workflow-policy.mts"];

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
  // Use Node's native glob implementation with repository-relative paths.
  // Forbid ambiguous escaping and negated patterns in the current positive-list policy.
  assert(!pattern.startsWith("!") && !pattern.includes("\\"), "Unsupported packet glob: " + pattern);
  return matchesGlob(path, pattern);
}
const matches = (path: string, patterns: readonly string[]) => patterns.some(pattern => pathMatches(path, pattern));

export function analyzeScope(root: string, paths: string[], metadata: unknown, scopePolicy: unknown) {
  const policy = record(scopePolicy);
  assert.equal(policy.schema_version, "crm.affected-scope-policy/v1");
  const rules = records(policy.scopes).map(rule => ({
    id: text(rule.id), owner: text(rule.owner), patterns: list(rule.path_patterns),
    workflows: list(rule.required_workflows),
  })).sort((a,b) => a.id < b.id ? -1 : a.id > b.id ? 1 : 0);
  assert.deepEqual(rules.map(rule => rule.id), requiredScopes, "invalid scope categories");
  const neutral = list(policy.neutral_path_patterns);
  const graph = record(metadata);
  const members = new Set(strings(graph.workspace_members));
  const packages = records(graph.packages).filter(pkg => members.has(string(pkg.id))).map(pkg => ({
    name: text(pkg.name), directory: dirname(within(root, string(pkg.manifest_path))).replace(/\\/g,"/"),
    dependencies: records(pkg.dependencies).map(dep => text(dep.name)),
  }));
  assert.equal(packages.length, members.size, "Cargo workspace metadata missing packages");
  assert.equal(new Set(packages.map(pkg => pkg.name)).size, packages.length, "duplicate workspace package names");
  const direct = new Set<string>();
  const unowned: string[] = [];
  const selectedPaths = [...new Set(paths)].sort();
  const ordered = [...packages].sort((a,b) => b.directory.length-a.directory.length);
  for (const path of selectedPaths) {
    const owner = ordered.find(pkg => path === pkg.directory || path.startsWith(pkg.directory + "/"));
    if (owner) direct.add(owner.name); else unowned.push(path);
  }
  const selectedScopes = rules.filter(rule => selectedPaths.some(path => matches(path, rule.patterns)));
  // One canonical dev workflow: all non-Markdown inputs trigger it.
  const selected = selectedPaths.some(path => !path.toLowerCase().endsWith(".md"));
  for (const rule of selectedScopes) {
    assert(rule.workflows.every(name => name === workflowName), "scope " + rule.id + " requires missing required CI workflows");
    assert(selected, "scope " + rule.id + " requires CI path filters to select it");
  }
  const governed = [...rules.flatMap(rule => rule.patterns), ...neutral, ...broad];
  const unknown = unowned.filter(path => !matches(path, governed));
  assert(!unknown.length, "unknown affected scope cannot prove a safe non-Rust workflow closure: " + unknown.join(", "));
  const broadened = selectedPaths.some(path => matches(path,broad));
  const affected = new Set(direct);
  // Transitive impact from Cargo's actual workspace dependency declarations.
  // This is diagnostic only: the one CRM CI always tests the full workspace.
  for (let changed = true; changed;) {
    changed = false;
    for (const pkg of packages) if (!affected.has(pkg.name) && pkg.dependencies.some(dep => affected.has(dep))) {
      affected.add(pkg.name); changed = true;
    }
  }
  return {direct_packages:[...direct].sort(), affected_packages:(broadened ? packages.map(pkg=>pkg.name) : [...affected]).sort(),
    selected_scopes:selectedScopes.map(rule=>rule.id), broadened, unowned_paths:unowned,
    selected_workflows:selected ? [{name:workflowName,path:canonicalWorkflow}] : []};
}

export function verifyPacketPaths(packetValue: unknown, paths: string[], baseRef: string): void {
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
  for (const path of paths.filter(path=>!path.toLowerCase().endsWith(".md"))) {
    assert(!matches(path, forbidden), "forbidden path changed: " + path);
    assert(matches(path, allowed), "path outside packet: " + path);
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
  const report = analyzeScope(root,paths,metadata,json(root,"affected-scope-policy.json"));
  return {event:"crm.packet-check",result:"PASS",base_sha:before,head_sha:head,changed_paths:paths,...report};
}
if (import.meta.main) {
  const before = process.argv[2];
  assert(before && process.argv.length===3, "expected push before SHA");
  console.log(JSON.stringify(checkPacket(repository,before)));
}
