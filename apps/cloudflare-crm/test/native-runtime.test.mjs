import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {validateWorkerConfig,validateNativeFixtureSource} from "../scripts/architecture-gate.mjs";
const config=JSON.parse(readFileSync(new URL("../wrangler.jsonc",import.meta.url),"utf8"));
const local=JSON.parse(readFileSync(new URL("../rust-party/wrangler.jsonc",import.meta.url),"utf8"));
test("canonical Worker configuration has exactly one D1 and matching local routing",()=>{
 assert.doesNotThrow(()=>validateWorkerConfig(config,local));
});
test("second D1 and mismatched routing are rejected",()=>{
 assert.throws(()=>validateWorkerConfig({...config,d1_databases:[...config.d1_databases,...config.d1_databases]},local),/exactly one native D1/);
 assert.throws(()=>validateWorkerConfig(config,{...local,assets:{...local.assets,run_worker_first:["/api/*"]}}),/local Worker-first/);
});
test("wrong account or database cannot be a release target",()=>{
 assert.throws(()=>validateWorkerConfig({...config,account_id:"another-account"},local),/unexpected Cloudflare account/);
 assert.throws(()=>validateWorkerConfig({...config,d1_databases:[{...config.d1_databases[0],database_id:"another-database"}]},local),/unexpected D1/);
});
test("Wrangler cannot publish a public or preview URL before release verification",()=>{
 assert.throws(()=>validateWorkerConfig({...config,workers_dev:true},local),/explicitly enable/);
 assert.throws(()=>validateWorkerConfig({...config,preview_urls:true},local),/explicitly enable/);
 assert.throws(()=>validateWorkerConfig({...config,routes:["example.org/*"]},local),/explicitly enable/);
});

test("active D1 fixtures cannot use an independent SQLite connection",()=>{
 assert.doesNotThrow(()=>validateNativeFixtureSource('const db=await runtime.getD1Database("DB"); await db.batch(statements);',"fixture.mjs"));
 for(const source of ['import {DatabaseSync} from "node:sqlite";','import SQLite from "better-sqlite3";','db.exec("BEGIN IMMEDIATE")','open("private.sqlite")'])
  assert.throws(()=>validateNativeFixtureSource(source,"fixture.mjs"),/direct SQLite access/);
});

test("native integration must never use the production D1 database",()=>{
 assert.throws(()=>validateWorkerConfig(config,{...local,d1_databases:config.d1_databases}),/isolated test D1/);
 assert.throws(()=>validateWorkerConfig(config,{...local,main:"./fixture-worker.js"}),/built Rust Worker/);
});
