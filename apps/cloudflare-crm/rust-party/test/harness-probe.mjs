// Temporary #471 compatibility probe: real native-ci Rust/WASM, one Wrangler harness, one D1.
// This file stays on experiment/471-wrangler-harness until a measured GO replaces old plumbing.
import assert from "node:assert/strict";
import {generateKeyPairSync, createSign, randomUUID} from "node:crypto";
import {execFileSync} from "node:child_process";
import {writeFileSync, mkdirSync} from "node:fs";
import {resolve} from "node:path";
import {fileURLToPath} from "node:url";
import {createTestHarness} from "wrangler";
import {chromium} from "playwright";
import {binaryPartyV3, decodePartyV3, partyV3Path}
  from "../../../../scripts/crm-connect-binary-fixture.mjs";

const app = resolve(fileURLToPath(new URL("../../", import.meta.url)));
const configPath = resolve(app, "rust-party/wrangler.jsonc");
const sha = execFileSync("git", ["rev-parse", "HEAD"], {cwd:app, encoding:"utf8"}).trim();
assert.equal(process.env.GITHUB_SHA, sha, "probe must use exact checkout SHA");
const {privateKey,publicKey} = generateKeyPairSync("rsa", {modulusLength:2048});
const kid="local-signed-rsa-fixture";
const jwks=JSON.stringify({keys:[{...publicKey.export({format:"jwk"}),kid,alg:"RS256",use:"sig"}]});
const iss="https://royal-limit-ea43.cloudflareaccess.com";
const aud="e0a664cf4f46f0d13d0ac0d8dd621c718c70842a377bf6738d1a24f0d16cedc7";
const sign=(email)=>{
  const now=Math.floor(Date.now()/1000);
  const parts=[{alg:"RS256",kid,typ:"JWT"},
    {iss,aud,iat:now-10,exp:now+3600,sub:email,email}]
    .map(part=>Buffer.from(JSON.stringify(part)).toString("base64url"));
  const raw=parts.join(".");
  return raw+"."+createSign("RSA-SHA256").update(raw).end().sign(privateKey).toString("base64url");
};
const admin=sign("native-a@example.test");
const viewer=sign("native-viewer@example.test");
const otherTenant=sign("native-b@example.test");
const harness=createTestHarness({
  workers:[{
    configPath,
    vars:{CRM_NATIVE_CI_JWKS:jwks,GIT_SHA:sha}
  }]
});
const report={source_sha:sha,harness:"wrangler/createTestHarness",worker:"crm-rust-party-ci-local",
  checks:{}, result:"INCOMPLETE"};
let browser;
const sql=async(db,query,...params)=>{
  const statement=db.prepare(query);
  const result=await (params.length?statement.bind(...params):statement).all();
  assert.equal(result.success,true,query);
  return result.results;
};
const writeReport=()=>{
  const path=resolve(app,"harness-probe-report.json");
  mkdirSync(app,{recursive:true});
  writeFileSync(path,JSON.stringify(report,null,2)+"\n");
  console.log(JSON.stringify({event:"crm.harness-probe",...report}));
};
try{
  const listen=await harness.listen();
  const worker=harness.getWorker("crm-rust-party-ci-local");
  report.checks.workerd_started=true;
  // One isolated D1 binding and the original migration files, not a second storage emulator.
  await worker.applyD1Migrations("DB");
  const env=await worker.getEnv();
  const db=env.DB;
  assert.ok(db,"native D1 binding must exist");
  for(const table of ["crm_tenants","crm_users","crm_party_records","crm_party_receipts","crm_audit"]){
    assert.equal((await sql(db,"SELECT COUNT(*) AS n FROM sqlite_master WHERE type='table' AND name=?",table))[0].n,1,table);
  }
  report.checks.migrations_and_d1=true;
  await db.prepare("INSERT INTO crm_tenants VALUES('tenant-a','A'),('tenant-b','B')").run();
  await db.prepare("INSERT INTO crm_users(email,tenant_id,role,active) VALUES "+
    "('native-a@example.test','tenant-a','admin',1),"+
    "('native-viewer@example.test','tenant-a','viewer',1),"+
    "('native-b@example.test','tenant-b','admin',1)").run();
  const request=async(path,token,body)=>{
    const headers={"content-type":"application/proto","connect-protocol-version":"1"};
    if(token)headers["cf-access-jwt-assertion"]=token;
    return worker.fetch(path,{method:"POST",headers,body});
  };
  const session=async(token)=>worker.fetch("/api/session",{headers:token?{"cf-access-jwt-assertion":token}:{}});
  assert.equal((await session(null)).status,401);
  assert.equal((await session(admin)).status,200);
  assert.equal((await session(otherTenant)).status,200);
  report.checks.signed_access_and_tenant=true;

  // Canonical compiled Buf descriptor fixture, not handwritten JSON/Protobuf.
  const partyId="r471-harness-"+randomUUID();
  const creation={partyId,kind:2,displayName:"  Harness  Native CRM  ",
    precondition:{idempotencyKey:"r471-harness-"+randomUUID()}};
  const path=partyV3Path("CreateParty"), data=binaryPartyV3("CreateParty",creation);
  assert.equal((await request(path,null,data)).status,401);
  assert.equal((await request(path,viewer,data)).status,403);
  const created=await request(path,admin,data);
  assert.equal(created.status,200,"generated Protobuf CreateParty should succeed");
  const originalRequestId=created.headers.get("x-request-id");
  const parsed=decodePartyV3("CreateParty",new Uint8Array(await created.arrayBuffer()));
  assert.equal(parsed.party?.partyId,partyId);
  assert.equal(parsed.party?.displayName,"Harness Native CRM");
  assert.equal(parsed.receipt?.requestId,originalRequestId);
  const replay=await request(path,admin,data);
  assert.equal(replay.status,200);
  assert.equal(decodePartyV3("CreateParty",new Uint8Array(await replay.arrayBuffer())).receipt?.replayed,true);
  const other=await request(partyV3Path("GetParty"),otherTenant,binaryPartyV3("GetParty",{partyId}));
  assert.equal(other.status,404);
  const rows=await sql(db,"SELECT id,tenant,version FROM crm_party_records WHERE id=?",partyId);
  assert.equal(rows.length,1);
  assert.equal(rows[0].tenant,"tenant-a");
  assert.equal((await sql(db,"SELECT COUNT(*) AS n FROM crm_audit WHERE request_id=?",originalRequestId))[0].n,1);
  report.checks.generated_protobuf_real_owner_d1_receipt_replay=true;

  const base=listen.url.href.replace(/\/$/,"");
  browser=await chromium.launch({headless:true});
  const ctx=await browser.newContext({extraHTTPHeaders:{"cf-access-jwt-assertion":admin},timezoneId:"UTC"});
  const page=await ctx.newPage();
  const opened=await page.goto(base+"/#parties");
  assert.ok(opened && opened.status()<400,"Chromium must load real React assets");
  await page.locator("#root").waitFor({timeout:15000});
  await page.locator("[data-crm-route-kind]").waitFor({timeout:15000});
  assert.ok((await page.locator("#root").innerText()).length>0);
  const browserRead=await page.evaluate(async()=>{
    const r=await fetch("/api/version");
    return {status:r.status,source:(await r.json()).source};
  });
  assert.equal(browserRead.status,200);
  assert.equal(browserRead.source,sha);
  report.checks.chromium_react_signed_worker=true;
  await ctx.close();

  report.result="PASS";
}catch(error){
  report.result="FAIL";
  report.failure=error instanceof Error?error.stack:String(error);
  harness.debug();
  throw error;
}finally{
  if(browser)await browser.close();
  await harness.close();
  writeReport();
}
