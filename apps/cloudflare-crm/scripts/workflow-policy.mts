// Inspect the native YAML AST, not a second indentation-based workflow parser.
import assert from "node:assert/strict";
import {globSync, readFileSync} from "node:fs";
import {resolve} from "node:path";
import {parseDocument, isMap, isScalar, isSeq} from "yaml";
import {isRecord, record, string} from "./json-boundary.mts";

export const canonicalWorkflow = ".github/workflows/cloudflare-crm-spike.yml";
const group = "$" + "{{ github.workflow }}-$" + "{{ github.ref }}";
const ignored = ["*.md", "**/*.md"];
const forbiddenDocumentGates = [
  "tests.test_repository_navigation", "tests.test_architecture_documentation_consistency",
  "generate_repository_navigation.py --check", "docs/ACTIVE_PACKET.md", "docs/generated/REPOSITORY_MAP.md",
];

export function workflowDocument(source: string) {
  const doc = parseDocument(source, {version: "1.2", schema: "core", strict: true, uniqueKeys: true, merge: false});
  assert(!doc.errors.length && !doc.warnings.length, "invalid workflow YAML");
  assert(isMap(doc.contents), "workflow must be a YAML mapping");
  return doc;
}
function sameStrings(value: unknown, expected: string[]): boolean {
  return Array.isArray(value) && value.length === expected.length &&
    expected.every((item, index) => value[index] === item);
}
export function verifyCrmWorkflow(source: string): void {
  const doc = workflowDocument(source);
  const value = record(doc.toJS({maxAliasCount: 0}));
  assert.equal(value.name, "Cloudflare CRM Spike CI", "canonical workflow name");
  const events = record(value.on);
  assert.deepEqual(Object.keys(events), ["push"], "CRM uses one dev pipeline, not duplicate triggers");
  const push = record(events.push);
  assert(sameStrings(push.branches, ["dev"]) && !Object.hasOwn(push, "branches-ignore"),
    "push must be restricted to branch dev");
  assert(sameStrings(push["paths-ignore"], ignored) && !Object.hasOwn(push, "paths"),
    "CRM may ignore only **/*.md documentation");
  assert(!forbiddenDocumentGates.some(gate => source.includes(gate)), "CRM CI must not validate Markdown documentation");
  const concurrency = record(value.concurrency);
  assert.equal(concurrency.group, group, "concurrency group must be workflow + PR number/ref");
  assert.equal(concurrency["cancel-in-progress"], false, "CRM must not cancel an active release");
  const jobs = record(value.jobs);
  assert(isRecord(jobs["cloudflare-crm-spike"]) && isRecord(jobs["deploy-crm-dev"]), "missing build/deploy job");
  const deploy = record(jobs["deploy-crm-dev"]);
  assert(deploy.needs === "cloudflare-crm-spike" ||
    (Array.isArray(deploy.needs) && deploy.needs.includes("cloudflare-crm-spike")),
    "deploy must depend on successful build");

  // The existing human-readable version-comment requirement remains.
  const astJobs = doc.get("jobs", true);
  assert(isMap(astJobs), "missing jobs mapping");
  function checkUses(node: unknown): void {
    assert(isMap(node), "expected job/step mapping");
    const uses = node.get("uses", true);
    if (uses === undefined) return;
    assert(isScalar(uses), "Action reference must be scalar");
    const ref = string(uses.value);
    if (ref.startsWith("./") || ref.startsWith("docker://")) return;
    const at = ref.lastIndexOf("@");
    assert(at > 0 && ref.slice(0, at).includes("/"), "external Action must use owner/repository@ref syntax");
    assert(/^[0-9a-f]{40}$/.test(ref.slice(at + 1)), "external Action must be pinned to a full lowercase commit SHA");
    assert(uses.comment?.trim(), "pinned Action must retain a human-readable version comment");
  }
  for (const job of astJobs.items) {
    checkUses(job.value);
    assert(isMap(job.value), "expected job mapping");
    const steps = job.value.get("steps", true);
    if (steps === undefined) continue;
    assert(isSeq(steps), "expected job steps sequence");
    for (const step of steps.items) checkUses(step);
  }
}

export function verifyWorkflowInventory(root: string): void {
  const workflows = globSync(".github/workflows/*.{yml,yaml}", {cwd: root}).sort();
  assert.deepEqual(workflows, [canonicalWorkflow], "CRM requires exactly one workflow");
  verifyCrmWorkflow(readFileSync(resolve(root, canonicalWorkflow), "utf8"));
}
