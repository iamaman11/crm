// A small post-deploy Chromium smoke. Complete business/UX regression lives in
// the existing typed-browser.mts and the independent post-merge Gate B.
import assert from "node:assert/strict";
import { PartyService } from "../../../packages/client/gen/crm/parties/v3/party_pb.js";
import { AccountService } from "../../../packages/client/gen/crm/accounts/v2/account_pb.js";
import { fixtureRpcPath } from "../../../packages/client/test/cloudflare-rpc-fixture.ts";
import {chromium} from "playwright";
import type {Locator, Page, Route, Response} from "playwright";
import {writeFileSync} from "node:fs";
import {resolve} from "node:path";
import {string} from "../scripts/json-boundary.mts";
import {waitForPublishedRpc} from "../scripts/release-readback.mts";
import {captureBrowserRpcEvidence} from "../rust-party/test/rpc-browser-evidence.mts";
import {reportBrowserFailure,withBrowserCleanup} from "../rust-party/test/browser-boundaries.mts";
import type {BrowserFailurePhase} from "../rust-party/test/browser-boundaries.mts";

type Restore = () => Promise<void>;
type SmokeOptions = {
  base: string;
  sourceSha: string;
  versionKey: string;
  tenantA: string;
  adminIdentity: string;
  adminHeaders: Record<string, string>;
  tenantBHeaders: Record<string, string>;
  sql: (statement: string, params: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
  beforeViewer: () => Promise<Restore>;
  directory: string;
};
const createParty = PartyService.method.createParty;
const createAccount = AccountService.method.createAccount;
const listAccounts = AccountService.method.listAccounts;
type SmokeMethod = typeof createParty | typeof createAccount | typeof listAccounts;

export async function publishedSmoke({base,sourceSha,versionKey,tenantA,adminIdentity,adminHeaders,
  tenantBHeaders,sql,beforeViewer,directory}: SmokeOptions) {
  const startedAt=Date.now(),runId=String(startedAt);
  const name="Browser Party "+runId,accountName="Browser Party Account "+runId;
  const deniedName="Browser Denied "+runId;
  const {requests,recordResponse: record,flushHeaderReads: flushHeaders,assertSource}=captureBrowserRpcEvidence();
  const pageErrors: string[] = [];
  const writes: Array<{path: string; request_id: string; audit: ReadonlyArray<Record<string, unknown>>}> = [];
  const syntheticPrivateProbes=[],steps: string[]=[];
  let restoreViewer: Restore=async()=>{};
  let closeBrowser: Restore=async()=>{};
  let phase: BrowserFailurePhase="browser-setup";
  let primaryFailure: Error|undefined;
  try {
  const browser=await chromium.launch({headless:true});
  closeBrowser=()=>browser.close();
  const versionHeaders={"Cloudflare-Workers-Version-Key":versionKey};
  const context=await browser.newContext({
    extraHTTPHeaders:{...adminHeaders,...versionHeaders},timezoneId:"UTC"
  });
  // Readiness probes are signed GETs and are outside the executed RPC evidence.
  context.on("response",response=>{
    if(response.request().method()==="POST")record(response);
  });
  const page=await context.newPage();
  page.on("pageerror",error=>pageErrors.push(error.message));
  const assertVersionKey=async(response: Response)=>{
    const headers=await response.request().allHeaders();
    assert.equal(headers["cloudflare-workers-version-key"],versionKey,
      "Every release smoke request must retain the same Cloudflare version-affinity key");
  };
  const responseFor=(targetPage: Page,method: SmokeMethod)=>targetPage.waitForResponse(response=>
    new URL(response.url()).pathname===fixtureRpcPath(method)&&response.request().method()==="POST");
  const auditFor=async(requestId: string)=>{
    const result=await sql(
      "SELECT action,subject_id,actor FROM crm_audit WHERE tenant_id=? AND request_id=?",
      [tenantA,requestId]);
    return result;
  };
  const mutation=async(button: Locator,method: typeof createParty | typeof createAccount,action: string)=>{
    const pending=responseFor(page,method);
    await button.click();
    const response=await pending;
    await assertVersionKey(response);
    assert.equal(response.status(),200,method.name+" failed");
    const headers=await response.allHeaders();
    assert.equal(headers["x-crm-source-sha"],sourceSha);
    assert.equal(headers["x-crm-runtime"],"native-workers-rs");
    const requestId=string(headers["x-request-id"]);
    assert.match(requestId??"",/^[a-f0-9-]{36}$/i);
    const audit=await auditFor(requestId);
    assert.equal(audit.filter(item=>item.action===action).length,1,
      method.name+" must commit one matching D1 audit row");
    for(const item of audit)assert.equal(item.actor,adminIdentity);
    writes.push({path:new URL(response.url()).pathname,request_id:requestId,audit});
    // Party shows a saved notice. CreateAccount from Party directly replaces
    // the draft URL with the canonical Account card; that is asserted below.
    if(method===createParty)
      await page.getByRole("heading",{name:/^Сохранено/}).last().waitFor();
    return requestId;
  };
  // Retry only demonstrably edge-owned asset/doc 404 on read-only navigation.
  const open=async(targetPage: Page,path: string,heading: string)=>{
    for(let attempt=1;attempt<=4;attempt++){
      // A same-hash page.goto() returns null in Playwright; use a real document
      // reload to prove the persisted Party remains visible after reopening.
      const response=targetPage.url()===base+path?
        await targetPage.reload():await targetPage.goto(base+path);
      if(response?.status()===404&&!response.headers()["x-crm-runtime"]){
        if(attempt===4)throw Error("CRM_PUBLISHED_EDGE_404_EXHAUSTED");
        continue;
      }
      assert.equal(response?.status(),200,"Published React document must load");
      await targetPage.getByRole("heading",{name:heading,exact:true}).waitFor();
      return;
    }
  };
    // Pin the browser before any mutation. The same run-attempt key is used by
    // the preceding signed readback and every browser context. During a 100%
    // rollout an edge still on the predecessor returns its old SHA here and
    // aborts before writes; once its deployment config advances, affinity keeps
    // the remaining browser requests on the new version. CI serializes deploys.
    phase="signed-readback";
    const preflight=await page.goto(base+"/api/version?browser-preflight="+Date.now());
    assert.ok(preflight,"Browser exact-version preflight must return an HTTP response");
    assert.equal(preflight.status(),200,"Browser exact-version preflight must be signed");
    await assertVersionKey(preflight);
    const preflightBody: unknown=await preflight.json();
    const preflightSource=preflightBody&&typeof preflightBody==="object"&&"source" in preflightBody?
      preflightBody.source:null;
    assert.equal(preflightSource,sourceSha,"Browser must pin the exact candidate source before writes");
    assert.equal(preflight.headers()["x-crm-source-sha"],sourceSha);
    assert.equal(preflight.headers()["x-crm-runtime"],"native-workers-rs");
    steps.push("version-affinity");
    phase="navigation";
    await open(page,"/#parties","Люди и организации");
    steps.push("signed-access");
    phase="party-create";
    // Real signed mutation with synthetic-only private values: no real user
    // names, credentials or data are written into privacy proof fixtures.
    const privateCookie="crm-s3-cookie-"+runId;
    const privateHeader="crm-s3-header-"+runId;
    const privateAuthorization="Bearer crm-s3-authorization-"+runId;
    const privateJwt="eyJhbGciOiJub25lIn0.eyJzdWIiOiJjcm0tc3ludGhldGljIn0.signature-"+runId;
    const privateQuery="crm-s3-query-"+runId;
    await context.addCookies([{name:"crm_s3_test_cookie",value:privateCookie,url:base}]);
    // Probe through Chromium's page network, on the same URL and with the
    // same browser credentials and probe headers as the upcoming write.
    const probePath=fixtureRpcPath(createParty)+"?crm_s3_test="+encodeURIComponent(privateQuery);
    await waitForPublishedRpc(sourceSha,()=>page.evaluate(async ({path,headers})=>{
      const response=await fetch(path,{headers,redirect:"manual",cache:"no-store",
        signal:AbortSignal.timeout(10000)});
      const observed={
        status:response.status,runtime:response.headers.get("x-crm-runtime"),
        source:response.headers.get("x-crm-source-sha"),
        errorCode:response.headers.get("x-crm-error-code"),
      };
      await response.arrayBuffer();
      return observed;
    },{path:probePath,headers:{"cache-control":"no-cache",authorization:privateAuthorization,
      "x-crm-s3-test-probe":privateHeader,"x-crm-s3-test-jwt":privateJwt}}));
    let privateRequests=0;
    const writeRoute="**"+fixtureRpcPath(createParty);
    const privacyRoute=async (route: Route)=>{
      const headers=await route.request().headers();
      assert.ok((headers.cookie??"").includes("crm_s3_test_cookie="+privateCookie));
      privateRequests++;
      const next=new URL(route.request().url());
      next.searchParams.set("crm_s3_test",privateQuery);
      await route.continue({url:next.toString(),headers:{
        ...headers,"x-crm-s3-test-probe":privateHeader,
        authorization:privateAuthorization,"x-crm-s3-test-jwt":privateJwt
      }});
    };
    await page.route(writeRoute,privacyRoute);
    await page.getByRole("button",{name:"Создать участника",exact:true}).click();
    await page.getByLabel("Имя или название",{exact:true}).fill(name);
    const partyRequestId=await mutation(
      page.getByRole("button",{name:"Сохранить участника",exact:true}),
      createParty,"parties.create");
    await page.unroute(writeRoute,privacyRoute);
    assert.equal(privateRequests,1,"Synthetic privacy route must run exactly once");
    await context.clearCookies({name:"crm_s3_test_cookie"});
    syntheticPrivateProbes.push(
      {kind:"party-name",request_id:partyRequestId,value:name},
      {kind:"cookie",request_id:partyRequestId,value:privateCookie},
      {kind:"header",request_id:partyRequestId,value:privateHeader},
      {kind:"authorization",request_id:partyRequestId,value:privateAuthorization},
      {kind:"jwt",request_id:partyRequestId,value:privateJwt},
      {kind:"query-string",request_id:partyRequestId,value:privateQuery});
    const parties=await sql(
      "SELECT id FROM crm_party_records WHERE tenant=? AND json_extract(state,'$.display_name')=?",
      [tenantA,name]);
    assert.equal(parties.length,1,"Signed CreateParty must persist exactly one record");
    const partyId=string(parties[0]?.id);
    steps.push("party-create");
    phase="party-reload";
    await open(page,"/#parties","Люди и организации");
    const heading=page.getByRole("heading",{name,exact:true});
    await heading.waitFor();
    steps.push("party-reload");
    phase="account-create";
    await page.locator("section").filter({has:heading})
      .getByRole("button",{name:"Создать клиента",exact:true}).click();
    await page.getByRole("heading",{name:"Новый клиент",exact:true}).waitFor();
    assert.equal(new URL(page.url()).hash,"#accounts/new?partyId="+partyId);
    assert.equal(await page.getByLabel("Основной участник",{exact:true}).inputValue(),partyId);
    await page.getByLabel("Название клиента",{exact:true}).fill(accountName);
    await mutation(page.getByRole("button",{name:"Сохранить клиента",exact:true}),
      createAccount,"accounts.create");
    const accounts=await sql(
      "SELECT id FROM crm_account_records WHERE tenant=? AND json_extract(state,'$.name')=?",
      [tenantA,accountName]);
    assert.equal(accounts.length,1,"Signed CreateAccount must persist exactly one record");
    await page.getByRole("heading",{name:"Карточка клиента",exact:true}).waitFor();
    assert.equal(new URL(page.url()).hash,"#accounts/"+string(accounts[0]?.id));
    steps.push("account-create");
    phase="viewer-denial";
    // An actual persisted-role downgrade, not a fake role header or JS mock.
    restoreViewer=await beforeViewer();
    const denied=await browser.newContext({extraHTTPHeaders:{...adminHeaders,...versionHeaders}});
    denied.on("response",record);
    await withBrowserCleanup(async()=>{
      const deniedPage=await denied.newPage();
      deniedPage.on("pageerror",error=>pageErrors.push(error.message));
      await open(deniedPage,"/#parties","Люди и организации");
      await deniedPage.getByRole("button",{name:"Создать участника",exact:true}).click();
      await deniedPage.getByLabel("Имя или название",{exact:true}).fill(deniedName);
      const forbidden=deniedPage.waitForResponse(response=>
        new URL(response.url()).pathname===fixtureRpcPath(createParty));
      await deniedPage.getByRole("button",{name:"Сохранить участника",exact:true}).click();
      const forbiddenResponse=await forbidden;
      await assertVersionKey(forbiddenResponse);
      assert.equal(forbiddenResponse.status(),403);
      await deniedPage.getByRole("heading",{name:/Недостаточно прав/}).waitFor();
      assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_party_records WHERE tenant=? "+
        "AND json_extract(state,'$.display_name')=?",[tenantA,deniedName]))[0]?.n,0);
    },async()=>{await flushHeaders();await denied.close();await restoreViewer();restoreViewer=async()=>{};});
    steps.push("viewer-denial");
    phase="tenant-isolation";
    const other=await browser.newContext({extraHTTPHeaders:{...tenantBHeaders,...versionHeaders}});
    other.on("response",record);
    await withBrowserCleanup(async()=>{
      const otherPage=await other.newPage();
      otherPage.on("pageerror",error=>pageErrors.push(error.message));
      await open(otherPage,"/#parties","Люди и организации");
      const [list]=await Promise.all([
        responseFor(otherPage,listAccounts),
        otherPage.getByRole("navigation").getByRole("link",{name:"Клиенты",exact:true}).click()
      ]);
      await assertVersionKey(list);
      assert.equal(list.status(),200,"Tenant B must load its signed account list");
      assert.equal(new URL(otherPage.url()).hash,"#accounts");
      await otherPage.getByText("Клиентов пока нет.",{exact:true}).waitFor();
      assert.equal(await otherPage.getByRole("heading",{name:accountName,exact:true}).count(),0);
    },async()=>{await flushHeaders();await other.close();});
    steps.push("tenant-isolation");
    phase="anonymous-denial";
    const anonymous=await browser.newContext({extraHTTPHeaders:versionHeaders});
    await withBrowserCleanup(async()=>{
      const response=await anonymous.request.get(base+"/api/session",{
        maxRedirects:0,headers:versionHeaders
      });
      assert.equal(response.status(),302,"Anonymous public request must stop at Cloudflare Access");
      assert.match(new URL(string(response.headers().location)).hostname,/\.cloudflareaccess\.com$/);
    },()=>anonymous.close());
    steps.push("anonymous-denial");
    phase="correlation";
    await flushHeaders();
    assert.deepEqual(pageErrors,[],"Browser page must have no uncaught runtime errors");
    assertSource(sourceSha);
    assert.ok(requests.some(item=>item.status===403&&item.path===fixtureRpcPath(createParty)),
      "Real viewer write denial must be observed");
    const evidence={result:"PASS",source_sha:sourceSha,version_affinity_key:versionKey,
      features:["native","published-smoke"],
      browser:"Chromium",started_at:startedAt,finished_at:Date.now(),tenant:tenantA,
      tested:"published React, signed Access, typed Connect, original Rust D1",
      steps,requests,writes,synthetic_private_probes:syntheticPrivateProbes};
    writeFileSync(resolve(directory,"published-smoke.json"),JSON.stringify(evidence,null,2)+"\n");
    return evidence;
  }catch(error){
    await flushHeaders().catch(()=>{});
    primaryFailure=reportBrowserFailure(error,{scenario:"published-smoke",phase,sourceSha,directory,steps,requests});
    throw primaryFailure;
  }finally{
    try {
      await withBrowserCleanup(restoreViewer,closeBrowser);
    } catch(error) {
      if(!primaryFailure) throw reportBrowserFailure(error,{scenario:"published-smoke",phase:"cleanup",sourceSha,directory,steps,requests});
      console.error("published-smoke cleanup failed after browser failure");
    }
  }
}
