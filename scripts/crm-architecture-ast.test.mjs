// Mutation-style self-tests: the gate must reject real prohibited code.
import test from "node:test";
import assert from "node:assert/strict";
import {inspect} from "./crm-architecture-ast.mjs";

const web="apps/cloudflare-crm/web/src/Check.tsx";
const client="packages/client/src/cloudflare.ts";
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

permits("generated Cloudflare client", 'import {createCloudflareCrmClient} from "@ultimate-crm/client/cloudflare";\nimport {useState} from "react";\nconst x = useState(1);');
permits("UI presentation only", 'import {Button} from "@ultimate-crm/ui";\nexport function Example(){return <Button>Save</Button>;}');
permits("Connect transport in the client package only", 'import {createConnectTransport} from "@connectrpc/connect-web";\nexport const transport=createConnectTransport({baseUrl:"/",useBinaryFormat:true});',client);
