// Temporary migration comparison. Native sources are used only after exact
// agreement with the previously accepted static report and packet result.
import assert from "node:assert/strict";
import {execFileSync} from "node:child_process";
import {readFileSync,writeFileSync} from "node:fs";
import {resolve} from "node:path";
import {fileURLToPath} from "node:url";
import {record,records,string,strings} from "./json-boundary.mts";
import {checkPacket} from "./packet-scope.mts";
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
assert.deepEqual(packet.affected_packages,strings(oldPacket.affected_packages),"affected package closure");
assert.deepEqual(packet.selected_workflows.map(w=>w.name),records(oldPacket.selected_workflows).map(w=>string(w.name)),"workflow selection");
const production=workerMetadata(root);
const native=buildModuleReport(root,production,workspace,packet.head_sha);
const previous:unknown=JSON.parse(readFileSync(reportPath,"utf8"));
assert.deepEqual(native,previous,"module ownership/inventory report parity");
const digests=Object.fromEntries(loadCanonicalManifests(root).map(({path,manifest})=>[path,manifestDigest(manifest)]));
assert.deepEqual(digests,JSON.parse(readFileSync(digestPath,"utf8")),"canonical manifest digests");
writeFileSync(reportPath,JSON.stringify(native,null,2)+"\n");
writeFileSync(resolve(metadataPath),JSON.stringify(production)+"\n");
console.log(JSON.stringify({event:"crm.governance-parity",result:"PASS",source_sha:packet.head_sha,
  counts:native.counts,affected_packages:packet.affected_packages,manifest_digests:Object.keys(digests).length}));
