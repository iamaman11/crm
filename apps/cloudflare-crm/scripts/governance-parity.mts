// Temporary migration comparison. Keep the old fail-closed guard active
// until corpus evidence confirms that native path classification does not widen it.
import assert from "node:assert/strict";
import {execFileSync} from "node:child_process";
import {readFileSync,writeFileSync} from "node:fs";
import {resolve} from "node:path";
import {fileURLToPath} from "node:url";
import {record,strings} from "./json-boundary.mts";
import {checkPacket,packetClassifier,scopeClassifier} from "./packet-scope.mts";
import {buildModuleReport,workerMetadata} from "./module-coverage.mts";
import {loadCanonicalManifests,manifestDigest} from "./module-manifest.mts";

const root=fileURLToPath(new URL("../../../",import.meta.url));
const [before,legacyPacket,reportPath,digestPath,metadataPath,...rest]=process.argv.slice(2);
assert(before&&legacyPacket&&reportPath&&digestPath&&metadataPath&&!rest.length,
  "expected push SHA, legacy packet/report/digests and production metadata output");
const workspace:unknown=JSON.parse(execFileSync("cargo",["metadata","--locked","--no-deps","--format-version","1"],
  {cwd:root,encoding:"utf8",maxBuffer:64*1024*1024}));
const packet=checkPacket(root,before,"origin/dev",workspace);
const oldPacket=record(JSON.parse(readFileSync(legacyPacket,"utf8")));
assert.equal(oldPacket.ok,true,"legacy packet must pass");
assert.equal(packet.head_sha,oldPacket.head_sha,"packet source SHA");
assert.equal(packet.base_sha,oldPacket.base_sha,"packet base SHA");
assert.deepEqual(packet.changed_paths.filter(path=>!path.toLowerCase().endsWith(".md")),strings(oldPacket.changed_paths),"packet changed paths");

// Compare actual tracked paths, changed/deleted paths and future boundary probes.
// The Python subprocess uses the SAME workspace metadata, not another Cargo call.
const tracked=execFileSync("git",["ls-files","-z"],{cwd:root,encoding:"utf8",maxBuffer:64*1024*1024}).split("\0").filter(Boolean);
const probes=["README.md","docs/README.md","AGENTS.md","Cargo.toml","crates/future/Cargo.toml",
  "modules/future/module.yaml","modules/future/nested/module.yaml","modules/future/src/new/nested.rs",
  "proto/crm/future/v1/future.proto","proto/crm/future/nested/v1/future.proto",
  "packages/client/src/cloudflare-future.test.ts","packages/client/src/new/cloudflare-future.test.ts",
  "packages/client/src/new/deeper/cloudflare-future.test.ts","scripts/crm-future.test.mjs",
  "scripts/crm-nested/future.test.mjs","apps/cloudflare-crm/test/new.node.test.mjs",
  "apps/cloudflare-crm/scripts/new.mts","database/secret.sql","services/future/src/lib.rs",
  "contracts/new.json","schemas/new.json",".github/actions/future/action.yml",
  "unknown/new.bin","unknown/Cargo.toml","unknown/package.json","unknown/README.md"];
const paths=[...new Set([...tracked,...packet.changed_paths,...probes])].sort();
const legacyCode=`
import json, sys
from pathlib import Path
from scripts.affected_scope import load_scope_policy, package_inventory, path_matches, BROAD_PATH_PATTERNS
from scripts.repository_navigation import load_packet
data=json.load(sys.stdin)
root=Path.cwd()
packet=load_packet(root)
rules,neutral=load_scope_policy(root)
packages=package_inventory(root,data["workspace"])
governed=[p for rule in rules for p in rule.path_patterns]+list(neutral)+list(BROAD_PATH_PATTERNS)
def matches(path,patterns): return any(path_matches(path,pattern) for pattern in patterns)
def classify(path):
    if path.lower().endswith(".md"): return "documentation"
    if matches(path,packet["forbidden_paths"]): return "forbidden"
    if not matches(path,packet["allowed_paths"]): return "outside"
    known=any(path==pkg.root or path.startswith(pkg.root+"/") for pkg in packages.values()) or matches(path,governed)
    return "allowed" if known else "unknown"
print(json.dumps({path:classify(path) for path in data["paths"]}))
`;
const legacy=record(JSON.parse(execFileSync("python",["-c",legacyCode],{cwd:root,encoding:"utf8",
  input:JSON.stringify({paths,workspace}),maxBuffer:64*1024*1024})));
const classifyPacket=packetClassifier(JSON.parse(readFileSync(resolve(root,"repository-packet.json"),"utf8")),"origin/dev");
const classifyScope=scopeClassifier(root,workspace,JSON.parse(readFileSync(resolve(root,"affected-scope-policy.json"),"utf8")));
const narrowed:string[]=[],widened:string[]=[];
for(const path of paths){
  const before=legacy[path], decision=classifyPacket(path);
  const after=decision==="allowed"&&!classifyScope(path).known?"unknown":decision;
  if(after==="allowed"&&before!=="allowed")widened.push(path);
  if(before==="allowed"&&after!=="allowed")narrowed.push(path);
}
console.log(JSON.stringify({event:"crm.packet-corpus",tracked:tracked.length,compared:paths.length,
  widened,narrowed_count:narrowed.length,narrowed_examples:narrowed.slice(0,30)}));
assert.deepEqual(widened,[],"native packet must not widen legacy admission");
// Narrowing is evidence to review before Python removal, not silently approved.
const production=workerMetadata(root);
const entries=loadCanonicalManifests(root);
const native=buildModuleReport(root,production,workspace,packet.head_sha,entries);
assert.deepEqual(native,JSON.parse(readFileSync(reportPath,"utf8")),"module ownership/inventory report parity");
const digests=Object.fromEntries(entries.map(({path,manifest})=>[path,manifestDigest(manifest)]));
assert.deepEqual(digests,JSON.parse(readFileSync(digestPath,"utf8")),"canonical manifest digests");
writeFileSync(reportPath,JSON.stringify(native,null,2)+"\n");
writeFileSync(resolve(metadataPath),JSON.stringify(production)+"\n");
console.log(JSON.stringify({event:"crm.governance-parity",result:"PASS",source_sha:packet.head_sha,
  counts:native.counts,selected_scopes:packet.selected_scopes,manifest_digests:Object.keys(digests).length}));
