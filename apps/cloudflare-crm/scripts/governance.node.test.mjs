import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {fileURLToPath} from "node:url";
import test from "node:test";
import {canonicalJson, loadCanonicalManifests, loadManifestValidator, manifestDigest, strictManifestYaml,
  validateManifestSet} from "./module-manifest.mts";
import {canonicalWorkflow, verifyCrmWorkflow, verifyWorkflowInventory} from "./workflow-policy.mts";

const root=fileURLToPath(new URL("../../../",import.meta.url));

test("strict authoring YAML preserves the existing rejection surface",()=>{
  const invalid=[
    "a: 1\na: 2\n", "a: &anchor 1\nb: *anchor\n", "a: !custom value\n",
    "base: {a: 1}\nvalue: {<<: {a: 1}}\n", "value: 1.25\n", "value: 1.0\n",
    "value: 1e0\n", "value: .inf\n", "value: 9007199254740992\n", "value: 2026-07-10\n",
    "value: 2026-07-10T10:00:00Z\n", "1: value\n", "é: value\n", "value: '<<'\n",
    "---\na: 1\n---\nb: 2\n",
  ];
  for(const text of invalid)assert.throws(()=>strictManifestYaml(text),text);
  assert.deepEqual(strictManifestYaml('value: "2026-07-10"\ninteger: 9007199254740991\n'),
    {value:"2026-07-10",integer:Number.MAX_SAFE_INTEGER});
  assert.equal(manifestDigest(strictManifestYaml("a: 1\nb: text\n")),
    manifestDigest(strictManifestYaml("b: text\na: 1\n")));
  assert.equal(canonicalJson(strictManifestYaml('"2": 1\n"10": 2\n')),'{"10":2,"2":1}');
});

test("canonical manifest schema and ownership are authoritative",()=>{
  const entries=loadCanonicalManifests(root);
  assert(entries.length>0);
  const validate=loadManifestValidator(JSON.parse(readFileSync(new URL("../../../schemas/module.schema.json",import.meta.url),"utf8")));
  const original=structuredClone(entries[0].manifest);
  assert.throws(()=>validate({...original,unknown_field:1},"fixture"),/additional properties/);
  assert.throws(()=>validate({...original,owner:{team:"sales-platform",contact:"invalid"}},"fixture"),/email/);
  const graph=(id,deps,objects=[])=>({module_id:id,dependencies:{required:deps.map(module_id=>({module_id}))},
    provides:{capabilities:[],events:[],objects}});
  assert.throws(()=>validateManifestSet([{path:"a",manifest:graph("crm.a",["crm.b"])},
    {path:"b",manifest:graph("crm.b",["crm.a"])}]),/cycle/);
  assert.throws(()=>validateManifestSet([{path:"a",manifest:graph("crm.a",["crm.missing"])}]),/required module missing/);
  assert.throws(()=>validateManifestSet([{path:"a",manifest:graph("crm.a",[],["shared.record"])},
    {path:"b",manifest:graph("crm.b",[],["shared.record"])}]),/already owned/);
});

test("the native workflow parser rejects changes to the canonical topology and pinning",()=>{
  verifyWorkflowInventory(root);
  const source=readFileSync(new URL("../../../"+canonicalWorkflow,import.meta.url),"utf8");
  const cases=[
    [source.replace("'**/*.md'","'apps/**'"),/ignore only/],
    [source.replace("    paths-ignore:","    paths:"),/ignore only/],
    [source.replace("cancel-in-progress: false","cancel-in-progress: true"),/must not cancel/],
    [source.replace("      - dev","      - main"),/restricted to branch dev/],
    [source.replace("concurrency:\n","  pull_request:\n    branches: [dev]\nconcurrency:\n"),/duplicate triggers/],
    [source.replace(/(uses: [^@\n]+@)[a-f0-9]{40}/,"$1v7"),/full lowercase commit SHA/],
    [source.replace(/(uses: [^@\n]+@[a-f0-9]{40}) #[^\n]*/,"$1"),/version comment/],
  ];
  for(const [candidate,message] of cases){
    assert.notEqual(candidate,source);
    assert.throws(()=>verifyCrmWorkflow(candidate),message);
  }
});
