// Mutation-style self-tests: the gate must reject real prohibited code.
import test from "node:test";
import assert from "node:assert/strict";
import {inspect,inspectFrontend,resolveInternal,assertStrictOptions} from "./crm-architecture-ast.mjs";

const web="apps/cloudflare-crm/web/src/Check.tsx";
const client="packages/client/src/cloudflare.ts";
test("inspect the active CRM frontend and its resolved workspace imports",()=>{
 const files=inspectFrontend();
 assert.ok(files.includes("apps/cloudflare-crm/web/src/app.tsx"));
 assert.ok(files.includes(client));
});
const rejects=(name,source,file=web)=>test(name,()=>assert.throws(
  ()=>inspect(file,source),/CRM_AST_VIOLATION/));
const permits=(name,source,file=web)=>test(name,()=>assert.doesNotThrow(
  ()=>inspect(file,source)));

rejects("manual REST fetch", 'async function load(){return fetch("/api/accounts");}');
rejects("indirect window.fetch", 'const request = window.fetch;');
rejects("computed network property", 'const request = globalThis["fetch"];');
rejects("alternate socket transport", 'const stream = new WebSocket("/api/accounts");');
rejects("legacy gateway import", 'import {GovernedCrmClient} from "../../../packages/client/src/gateway";');
rejects("legacy client barrel import", 'import {GovernedCrmClient} from "@ultimate-crm/client";');
rejects("alternate Connect transport in UI", 'import {createConnectTransport} from "@connectrpc/connect-web";');
rejects("wrong typed client export", 'import {GovernedCrmClient} from "@ultimate-crm/client/cloudflare";');
rejects("computed dynamic import", 'const module = await import(routeName);');
rejects("CommonJS require", 'const transport = require("fake-transport");');
rejects("explicit any bypass", 'const input: any = {};');
rejects("double unknown cast", 'const input = {} as unknown as Account;');
rejects("typescript diagnostic suppression", '// @ts-ignore\nconst input = 1;');
rejects("legacy business REST path", 'const endpoint="/api/parties";');
rejects("legacy transport in client package", 'import {createGrpcWebTransport} from "@connectrpc/connect-web";',client);
rejects("second Connect transport in workspace helper", 'import {createConnectTransport as transport} from "@connectrpc/connect-web";',"packages/client/src/alternate.ts");
rejects("Connect transport through shared UI", 'export {createConnectTransport} from "@connectrpc/connect-web";',"packages/ui/src/alternate.ts");

permits("generated Cloudflare client", 'import {createCloudflareCrmClient} from "@ultimate-crm/client/cloudflare";\nimport {useState} from "react";\nconst x = useState(1);');
permits("generated enum values from the canonical client entrypoint",
  'import {AccountStatus,PartyKind,TaskPriority} from "@ultimate-crm/client/cloudflare";');
rejects("an exported helper is not a generated enum or client factory",
  'import {decodeCrmError} from "@ultimate-crm/client/cloudflare";');
rejects("a generated enum must belong to the public client API",
  'import {MutationCommitOutcome} from "@ultimate-crm/client/cloudflare";');
rejects("an unapproved runtime export cannot bypass the client boundary",
  'import {createAlternateClient} from "@ultimate-crm/client/cloudflare";');
permits("UI presentation only", 'import {Button} from "@ultimate-crm/ui";\nexport function Example(){return <Button>Save</Button>;}');
permits("Connect transport in the client package only", 'import {createConnectTransport} from "@connectrpc/connect-web";\nexport const transport=createConnectTransport({baseUrl:"/",useBinaryFormat:true});',client);

permits("generated client type imports", 'import type {CloudflareCrmClient} from "@ultimate-crm/client/cloudflare";');
test("workspace exports resolve through the installed TypeScript module resolver",()=>{
 assert.equal(resolveInternal("@ultimate-crm/client/cloudflare",web),"packages/client/src/cloudflare.ts");
 assert.equal(resolveInternal("@ultimate-crm/ui",web),"packages/ui/src/index.ts");
});
test("non-exported workspace subpath is rejected by package exports",()=>{
 assert.throws(()=>resolveInternal("@ultimate-crm/client/src/cloudflare",web),/unresolved import/);
});
test("missing relative import cannot pass by source-name guessing",()=>{
 assert.throws(()=>resolveInternal("./this-module-does-not-exist.js",web),/unresolved import/);
});
test("relative paths cannot bypass the public client or UI export",()=>{
 for(const path of ["../../../../packages/client/src/cloudflare.ts","../../../../packages/ui/src/Forms.tsx"])
  assert.throws(()=>resolveInternal(path,web),/relative import bypasses public workspace exports/);
});

test("individual compiler overrides cannot weaken a strict CRM project",()=>{
 const base={strict:true,noUncheckedIndexedAccess:true,exactOptionalPropertyTypes:true};
 for(const override of [{noCheck:true},{allowJs:true},{strictNullChecks:false},{noImplicitAny:false}]){
  assert.throws(()=>assertStrictOptions("future CRM module",{...base,...override}),/TypeScript option/);
 }
});
