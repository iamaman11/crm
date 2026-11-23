import assert from "node:assert/strict";
import {spawnSync} from "node:child_process";
import {mkdtempSync,rmSync,writeFileSync} from "node:fs";
import {join,resolve} from "node:path";
import {fileURLToPath} from "node:url";
import test from "node:test";

// Exercise ONLY pinned Oxlint's actual configuration against small forbidden
// source fixtures. No homegrown AST/parser or parallel architecture gate.
const root=resolve(fileURLToPath(new URL("../../../",import.meta.url)));
const ui=join(root,"apps/cloudflare-crm/web/src");
const probes=[
 ["bare-fetch",'export const request=()=>fetch("/api/manual-business");',"no-restricted-globals"],
 ["aliased-fetch",'export const request=fetch;',"no-restricted-globals"],
 ["window-fetch",'export const request=window.fetch;',"no-restricted-properties"],
 ["computed-fetch",'export const request=globalThis["fetch"];',"no-restricted-properties"],
 ["websocket",'export const socket=new WebSocket("wss://invalid.example");',"no-restricted-globals"],
 ["beacon",'export const request=navigator.sendBeacon("/api/manual-business","x");',"no-restricted-properties"],
 ["connect-import",'import {createConnectTransport} from "@connectrpc/connect-web"; export {createConnectTransport};',"no-restricted-imports"],
 ["legacy-client-import",'import {legacy} from "@ultimate-crm/client"; export {legacy};',"no-restricted-imports"],
 ["private-relative-import",'import {createCloudflareCrmClient} from "../../../../../packages/client/src/cloudflare"; export {createCloudflareCrmClient};',"no-relative-parent-imports"],
 ["explicit-any",'export const unsafe={} as any;',"no-explicit-any"],
 ["ts-suppression",'// @ts-ignore\nexport const answer:number="wrong";',"ban-ts-comment"],
 ["dynamic-import",'export const get=(name:string)=>import(name);',"no-dynamic-require"],
 ["eval",'export const run=(source:string)=>eval(source);',"no-eval"],
];
function lint(path){
 const res=spawnSync("pnpm",["exec","oxlint","--no-ignore","-c",".oxlintrc.json",path],{
  cwd:root,encoding:"utf8",timeout:30000,
 });
 if(res.error)throw res.error;
 return {status:res.status,output:(res.stdout||"")+(res.stderr||"")};
}
test("native Oxlint rejects forbidden CRM UI code and permits the typed client",()=>{
 const directory=mkdtempSync(join(ui,"crm-oxlint-fixtures-"));
 try{
  for(const [id,source,rule] of probes){
   const file=join(directory,id+".ts");
   writeFileSync(file,source+"\n");
   const result=lint(file);
   assert.notEqual(result.status,0,id+" must FAIL Oxlint: "+result.output);
   assert.ok(result.output.includes(rule),id+" must fail by "+rule+": "+result.output);
  }
  const safe=join(directory,"typed-client.ts");
  writeFileSync(safe,
   'import {createCloudflareCrmClient} from "@ultimate-crm/client/cloudflare";\nexport {createCloudflareCrmClient};\n');
  const result=lint(safe);
  assert.equal(result.status,0,"valid typed facade must PASS: "+result.output);
 }finally{
  rmSync(directory,{recursive:true,force:true});
 }
});
