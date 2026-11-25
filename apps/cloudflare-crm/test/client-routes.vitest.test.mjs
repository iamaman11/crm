// Exercise the public client facade against an in-memory binary Connect transport endpoint.
import {readFileSync} from "node:fs";
import {expect,it,vi} from "vitest";
import {createCloudflareCrmClient} from "@ultimate-crm/client/cloudflare";
import {descriptorMethods,targetMethods} from "../../../scripts/crm-rpc-descriptors.mjs";

it("exposes every active Buf RPC through the production browser client",async()=>{
 const methods=descriptorMethods(readFileSync(process.env.CRM_PROTO_DESCRIPTOR_FILE));
 const expected=targetMethods(methods);
 const observed=[];
 vi.stubGlobal("fetch",async(input,init)=>{
  const request=new Request(input,init);
  expect(request.method).toBe("POST");
  expect(request.headers.get("content-type")).toBe("application/proto");
  observed.push(new URL(request.url).pathname);
  return new Response(new Uint8Array(),{headers:{"content-type":"application/proto"}});
 });
 try{
  const client=createCloudflareCrmClient("https://crm.invalid");
  for(const service of Object.values(client)){
   if(!service||typeof service!=="object")continue;
   for(const call of Object.values(service))await call({});
  }
  expect(observed.sort()).toEqual(expected);
 }finally{vi.unstubAllGlobals();}
});
