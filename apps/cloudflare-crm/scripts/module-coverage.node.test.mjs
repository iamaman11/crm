import assert from "node:assert/strict";
import {execFileSync} from "node:child_process";
import {copyFileSync,mkdirSync,mkdtempSync,readFileSync,realpathSync,rmSync,writeFileSync} from "node:fs";
import {tmpdir} from "node:os";
import {resolve} from "node:path";
import {fileURLToPath} from "node:url";
import test from "node:test";
import {buildModuleReport,workerMetadata} from "./module-coverage.mts";
import {strictManifestYaml} from "./module-manifest.mts";

const repository=fileURLToPath(new URL("../../../",import.meta.url));

test("native Cargo inventory discovers an unlinked valid crate without granting activation",()=>{
  const root=realpathSync(mkdtempSync(resolve(tmpdir(),"crm-module-native-")));
  try{
    mkdirSync(resolve(root,"schemas"));
    copyFileSync(resolve(repository,"schemas/module.schema.json"),resolve(root,"schemas/module.schema.json"));
    function crate(directory,name,worker=false){
      const path=resolve(root,directory);
      mkdirSync(resolve(path,"src"),{recursive:true});
      const manifest=resolve(path,"Cargo.toml");
      writeFileSync(manifest,'[package]\nname="'+name+'"\nversion="0.1.0"\nedition="2024"\n[workspace]\n'+
        (worker?'[features]\nnative=[]\n':""));
      writeFileSync(resolve(path,"src/lib.rs"),"pub fn example() {}\n");
      execFileSync("cargo",["generate-lockfile","--offline","--manifest-path",manifest],{cwd:root,stdio:"pipe"});
    }
    crate("apps/cloudflare-crm/rust-party","fixture-worker",true);
    const directory=resolve(root,"modules/crm-access");
    mkdirSync(directory,{recursive:true});
    const access=strictManifestYaml(readFileSync(resolve(repository,"modules/crm-access/module.yaml"),"utf8"));
    writeFileSync(resolve(directory,"module.yaml"),JSON.stringify(access));
    const native=workerMetadata(root);
    const workspace={packages:[]};
    const sha="a".repeat(40);
    const baseline=buildModuleReport(root,native,workspace,sha);
    assert.equal(baseline.counts.discovered_modules,1);
    assert.equal(baseline.counts.linked_modules,1);
    crate("modules/crm-ci-expansion-probe","crm-ci-expansion-probe");
    assert.throws(()=>buildModuleReport(root,native,workspace,sha),/missing sibling module.yaml/);
    const probe=structuredClone(access);
    probe.module_id="crm.ci-expansion-probe";
    probe.runtime.entrypoint="modules/crm-ci-expansion-probe";
    probe.provides={capabilities:[],events:[],objects:[],ui_extensions:[]};
    probe.storage={record_types:[],private_state_namespaces:[]};
    probe.lifecycle.retained_record_types=[];
    const path=resolve(root,"modules/crm-ci-expansion-probe/module.yaml");
    writeFileSync(path,JSON.stringify(probe));
    const expanded=buildModuleReport(root,native,workspace,sha);
    assert.equal(expanded.counts.discovered_modules,2);
    assert.equal(expanded.counts.linked_modules,1);
    const discovered=expanded.modules.find(row=>row.module_id==="crm.ci-expansion-probe");
    assert.equal(discovered.crate_name,"crm-ci-expansion-probe");
    assert.equal(discovered.linked,false);
    assert.deepEqual(discovered.active_rpc_paths,[]);
    assert.equal(expanded.runtime_evidence,"NOT_EXECUTED");
    probe.provides.capabilities=[{id:"probe.duplicate",version:"1.0.0",binding:access.provides.capabilities[0].binding}];
    writeFileSync(path,JSON.stringify(probe));
    assert.throws(()=>buildModuleReport(root,native,workspace,sha),/duplicate RPC owner/);
  }finally{rmSync(root,{recursive:true,force:true});}
});
