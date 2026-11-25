// Canonical manifest inventory; discovered is not linked, and linked is not executed.
import assert from "node:assert/strict";
import {execFileSync} from "node:child_process";
import {globSync, realpathSync, mkdirSync, writeFileSync} from "node:fs";
import {dirname, relative, resolve, sep} from "node:path";
import {fileURLToPath} from "node:url";
import {record, records, string, strings} from "./json-boundary.mts";
import {insideRepo, loadCanonicalManifests} from "./module-manifest.mts";

const repository = fileURLToPath(new URL("../../../", import.meta.url));
const workerDirectory = "apps/cloudflare-crm/rust-party";
function cargo(root: string, args: string[]): unknown {
  return JSON.parse(execFileSync("cargo", args, {cwd:root,encoding:"utf8",maxBuffer:64*1024*1024,
    stdio:["ignore","pipe","inherit"]}));
}
export function workerMetadata(root: string): unknown {
  return cargo(root,["metadata","--locked","--format-version","1","--manifest-path",
    resolve(root,workerDirectory,"Cargo.toml"),"--filter-platform","wasm32-unknown-unknown","--features","native"]);
}
function posix(root: string, path: string): string {return relative(root,path).split(sep).join("/");}
function packageAt(packages: Record<string,unknown>[], path:string) {
  const candidates=packages.filter(pkg=>dirname(string(pkg.manifest_path))===path);
  assert(candidates.length<=1,"ambiguous Cargo crate at "+path);
  return candidates[0];
}
export function buildModuleReport(root: string, native: unknown, workspace: unknown, sourceSha: string,
  entries = loadCanonicalManifests(root)) {
  root=realpathSync(root);
  assert(/^[a-f0-9]{40}$/.test(sourceSha),"invalid source SHA");
  const worker=insideRepo(root,workerDirectory);
  const metadata=record(native), resolution=record(metadata.resolve);
  const packages=records(metadata.packages), nodes=records(resolution.nodes);
  const workerId=string(resolution.root);
  const workerPackage=packages.find(pkg=>pkg.id===workerId);
  const workerNode=nodes.find(node=>node.id===workerId);
  assert(workerPackage&&workerNode&&dirname(string(workerPackage.manifest_path))===worker,"invalid Worker metadata root");
  const resolvedIds=new Set(strings(workerNode.dependencies));
  const direct=new Map<string,string>();
  for(const dep of records(workerPackage.dependencies)){
    if(dep.path===null||dep.path===undefined||dep.kind!==null||dep.target!==null)continue;
    const path=insideRepo(root,string(dep.path));
    assert(!direct.has(path),"duplicate linked crate path: "+path);
    assert(realpathSync(resolve(path,"Cargo.toml")),"missing Cargo.toml");
    const name=string(dep.name);
    const pkg=packageAt(packages,path);
    assert(pkg&&pkg.name===name&&resolvedIds.has(string(pkg.id)),"missing resolved owner: "+name);
    direct.set(path,name);
  }
  const moduleDirs=new Set(entries.map(entry=>dirname(insideRepo(root,entry.path))));
  for(const path of globSync("modules/*/Cargo.toml",{cwd:root})){
    assert(moduleDirs.has(dirname(insideRepo(root,path))),path+": missing sibling module.yaml");
  }
  const workspacePackages=records(record(workspace).packages);
  const providers=new Set<string>();
  const modules=entries.map(({path,manifest})=>{
    const moduleId=string(manifest.module_id), runtime=record(manifest.runtime);
    assert(runtime.kind==="in_process","Cloudflare module must have in-process entrypoint: "+moduleId);
    const entry=string(runtime.entrypoint), crate=insideRepo(root,entry);
    const cratePath=posix(root,crate);
    if(cratePath.startsWith("modules/"))assert.equal(crate,dirname(insideRepo(root,path)),
      moduleId+": domain entrypoint must be its own module directory");
    insideRepo(root,resolve(crate,"Cargo.toml"));
    let pkg=packageAt(packages,crate)??packageAt(workspacePackages,crate);
    if(!pkg){
      // A discovered-but-unlinked crate can be a legitimate standalone/excluded
      // Cargo package. Cargo must identify it; never guess from a directory name.
      pkg=packageAt(records(record(cargo(root,["metadata","--locked","--no-deps","--format-version","1","--manifest-path",resolve(crate,"Cargo.toml")])).packages),crate);
    }
    assert(pkg,"Cargo metadata missing in-process crate: "+entry);
    const crateName=string(pkg.name);
    const link=direct.get(crate);
    if(link!==undefined)assert.equal(link,crateName,"linked Rust crate package mismatch");
    const rpcBindings=records(record(manifest.provides).capabilities).map(capability=>{
      const binding=record(capability.binding), rpc=string(binding.rpc);
      assert.equal(binding.kind,"protobuf_rpc");
      const split=rpc.lastIndexOf(".");
      assert(split>0&&split<rpc.length-1,"invalid protobuf RPC binding");
      const route="/"+rpc.slice(0,split)+"/"+rpc.slice(split+1);
      assert(!providers.has(route),"duplicate RPC owner "+route);
      providers.add(route);
      return {path:route,capability_id:string(capability.id),capability_version:string(capability.version)};
    }).sort((a,b)=>a.path<b.path?-1:a.path>b.path?1:0);
    return {module_id:moduleId,manifest:path,crate_path:cratePath,crate_name:crateName,
      kind:entry.startsWith("crates/")?"composition":"domain",linked:link!==undefined||crate===worker,
      declared_rpc_paths:rpcBindings.map(binding=>binding.path),rpc_bindings:rpcBindings,active_rpc_paths:[]};
  }).sort((a,b)=>a.module_id<b.module_id?-1:a.module_id>b.module_id?1:0);
  return {schema_version:1,source:"modules/*/module.yaml + Worker Cargo.toml",
    runtime_evidence:"NOT_EXECUTED",runtime:null,rpc_policy:{},source_sha:sourceSha,modules,
    active_rpc_paths:[],unclaimed_active_rpc_paths:[],
    counts:{discovered_modules:modules.length,linked_modules:modules.filter(module=>module.linked).length,
      active_rpc_methods:0,declared_rpc_bindings:providers.size}};
}

if(import.meta.main){
  const [output, ...args]=process.argv.slice(2);
  assert(output&&args.length===0,"expected module report output path");
  const sha=execFileSync("git",["rev-parse","HEAD"],{cwd:repository,encoding:"utf8"}).trim();
  const production=workerMetadata(repository);
  const workspace=cargo(repository,["metadata","--locked","--no-deps","--format-version","1"]);
  const report=buildModuleReport(repository,production,workspace,sha);
  mkdirSync(dirname(resolve(output)),{recursive:true});
  writeFileSync(output,JSON.stringify(report,null,2)+"\n");
  console.log(JSON.stringify({event:"crm.module-coverage",result:"PASS",report:output,counts:report.counts}));
}
