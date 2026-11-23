// A small post-deploy Chromium smoke. Complete business/UX regression lives in
// the existing typed-browser.mjs and the independent post-merge Gate B.
import assert from "node:assert/strict";
import {chromium} from "playwright";

export async function publishedSmoke({base,sourceSha,tenantA,adminIdentity,adminHeaders,
  tenantBHeaders,sql,beforeViewer,directory}) {
  const startedAt=Date.now(),runId=String(startedAt);
  const name="Browser Party "+runId,accountName="Browser Party Account "+runId;
  const deniedName="Browser Denied "+runId;
  const requests=[],writes=[],syntheticPrivateProbes=[],steps=[],headerReads=[],pageErrors=[];
  const record=response=>{
    const path=new URL(response.url()).pathname;
    if(!path.startsWith("/crm."))return;
    const item={path,status:response.status()};
    requests.push(item);
    headerReads.push(response.allHeaders().then(headers=>{
      item.request_id=headers["x-request-id"];
      item.error_code=headers["x-crm-error-code"]??null;
      item.source_sha=headers["x-crm-source-sha"];
      item.runtime=headers["x-crm-runtime"];
    }));
  };
  const browser=await chromium.launch({headless:true});
  let restoreViewer=async()=>{};
  const context=await browser.newContext({extraHTTPHeaders:adminHeaders,timezoneId:"UTC"});
  context.on("response",record);
  const page=await context.newPage();
  page.on("pageerror",error=>pageErrors.push(error.message));
  const responseFor=(targetPage,method)=>targetPage.waitForResponse(response=>
    new URL(response.url()).pathname.endsWith("/"+method)&&response.request().method()==="POST");
  const auditFor=async(requestId)=>{
    const result=await sql(
      "SELECT action,subject_id,actor FROM crm_audit WHERE tenant_id=? AND request_id=?",
      [tenantA,requestId]);
    return result;
  };
  const mutation=async(button,method,action)=>{
    const pending=responseFor(page,method);
    await button.click();
    const response=await pending;
    assert.equal(response.status(),200,method+" failed");
    const headers=await response.allHeaders();
    assert.equal(headers["x-crm-source-sha"],sourceSha);
    assert.equal(headers["x-crm-runtime"],"native-workers-rs");
    const requestId=headers["x-request-id"];
    assert.match(requestId??"",/^[a-f0-9-]{36}$/i);
    const audit=await auditFor(requestId);
    assert.equal(audit.filter(item=>item.action===action).length,1,
      method+" must commit one matching D1 audit row");
    for(const item of audit)assert.equal(item.actor,adminIdentity);
    writes.push({path:new URL(response.url()).pathname,request_id:requestId,audit});
    // Party shows a saved notice. CreateAccount from Party directly replaces
    // the draft URL with the canonical Account card; that is asserted below.
    if(method==="CreateParty")
      await page.getByRole("heading",{name:/^Сохранено/}).last().waitFor();
    return requestId;
  };
  // Retry only demonstrably edge-owned asset/doc 404 on read-only navigation.
  const open=async(targetPage,path,heading)=>{
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
  try {
    await open(page,"/#parties","Люди и организации");
    steps.push("signed-access");
    // Real signed mutation with synthetic-only private values: no real user
    // names, credentials or data are written into privacy proof fixtures.
    const privateCookie="crm-s3-cookie-"+runId;
    const privateHeader="crm-s3-header-"+runId;
    const privateAuthorization="Bearer crm-s3-authorization-"+runId;
    const privateJwt="eyJhbGciOiJub25lIn0.eyJzdWIiOiJjcm0tc3ludGhldGljIn0.signature-"+runId;
    const privateQuery="crm-s3-query-"+runId;
    await context.addCookies([{name:"crm_s3_test_cookie",value:privateCookie,url:base}]);
    let privateRequests=0;
    const writeRoute="**/crm.parties.v3.PartyService/CreateParty";
    const privacyRoute=async route=>{
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
      "CreateParty","parties.create");
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
    const partyId=parties[0].id;
    steps.push("party-create");
    await open(page,"/#parties","Люди и организации");
    const heading=page.getByRole("heading",{name,exact:true});
    await heading.waitFor();
    steps.push("party-reload");
    await page.locator("section").filter({has:heading})
      .getByRole("button",{name:"Создать клиента",exact:true}).click();
    await page.getByRole("heading",{name:"Новый клиент",exact:true}).waitFor();
    assert.equal(new URL(page.url()).hash,"#accounts/new?partyId="+partyId);
    assert.equal(await page.getByLabel("Основной участник",{exact:true}).inputValue(),partyId);
    await page.getByLabel("Название клиента",{exact:true}).fill(accountName);
    await mutation(page.getByRole("button",{name:"Сохранить клиента",exact:true}),
      "CreateAccount","accounts.create");
    const accounts=await sql(
      "SELECT id FROM crm_account_records WHERE tenant=? AND json_extract(state,'$.name')=?",
      [tenantA,accountName]);
    assert.equal(accounts.length,1,"Signed CreateAccount must persist exactly one record");
    await page.getByRole("heading",{name:"Карточка клиента",exact:true}).waitFor();
    assert.equal(new URL(page.url()).hash,"#accounts/"+accounts[0].id);
    steps.push("account-create");
    // An actual persisted-role downgrade, not a fake role header or JS mock.
    restoreViewer=await beforeViewer();
    const denied=await browser.newContext({extraHTTPHeaders:adminHeaders});
    denied.on("response",record);
    try{
      const deniedPage=await denied.newPage();
      deniedPage.on("pageerror",error=>pageErrors.push(error.message));
      await open(deniedPage,"/#parties","Люди и организации");
      await deniedPage.getByRole("button",{name:"Создать участника",exact:true}).click();
      await deniedPage.getByLabel("Имя или название",{exact:true}).fill(deniedName);
      const forbidden=deniedPage.waitForResponse(response=>
        new URL(response.url()).pathname.endsWith("/CreateParty"));
      await deniedPage.getByRole("button",{name:"Сохранить участника",exact:true}).click();
      assert.equal((await forbidden).status(),403);
      await deniedPage.getByRole("heading",{name:/Недостаточно прав/}).waitFor();
      assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_party_records WHERE tenant=? "+
        "AND json_extract(state,'$.display_name')=?",[tenantA,deniedName]))[0].n,0);
    }finally{await denied.close();await restoreViewer();restoreViewer=async()=>{};}
    steps.push("viewer-denial");
    const other=await browser.newContext({extraHTTPHeaders:tenantBHeaders});
    other.on("response",record);
    try{
      const otherPage=await other.newPage();
      otherPage.on("pageerror",error=>pageErrors.push(error.message));
      await open(otherPage,"/#parties","Люди и организации");
      const [list]=await Promise.all([
        responseFor(otherPage,"ListAccounts"),
        otherPage.getByRole("navigation").getByRole("link",{name:"Клиенты",exact:true}).click()
      ]);
      assert.equal(list.status(),200,"Tenant B must load its signed account list");
      assert.equal(new URL(otherPage.url()).hash,"#accounts");
      await otherPage.getByText("Клиентов пока нет.",{exact:true}).waitFor();
      assert.equal(await otherPage.getByRole("heading",{name:accountName,exact:true}).count(),0);
    }finally{await other.close();}
    steps.push("tenant-isolation");
    const anonymous=await browser.newContext();
    try{
      const response=await anonymous.request.get(base+"/api/session",{maxRedirects:0});
      assert.equal(response.status(),302,"Anonymous public request must stop at Cloudflare Access");
      assert.match(new URL(response.headers().location).hostname,/\.cloudflareaccess\.com$/);
    }finally{await anonymous.close();}
    steps.push("anonymous-denial");
    await Promise.all(headerReads);
    assert.deepEqual(pageErrors,[],"Browser page must have no uncaught runtime errors");
    for(const response of requests){
      assert.match(response.request_id??"",/^[a-f0-9-]{36}$/i);
      assert.equal(response.source_sha,sourceSha);
      assert.equal(response.runtime,"native-workers-rs");
    }
    assert.ok(requests.some(item=>item.status===403&&item.path.endsWith("/CreateParty")),
      "Real viewer write denial must be observed");
    const evidence={result:"PASS",source_sha:sourceSha,features:["native","published-smoke"],
      browser:"Chromium",started_at:startedAt,finished_at:Date.now(),tenant:tenantA,
      tested:"published React, signed Access, typed Connect, original Rust D1",
      steps,requests,writes,synthetic_private_probes:syntheticPrivateProbes};
    const {writeFileSync}=await import("node:fs");
    const {resolve}=await import("node:path");
    writeFileSync(resolve(directory,"published-smoke.json"),JSON.stringify(evidence,null,2)+"\n");
    return evidence;
  }catch(error){
    await Promise.allSettled(headerReads);
    const {writeFileSync}=await import("node:fs");
    const {resolve}=await import("node:path");
    writeFileSync(resolve(directory,"failure.json"),JSON.stringify({
      result:"FAIL",source_sha:sourceSha,error:String(error.message),steps,requests,writes
    },null,2)+"\n");
    await page.screenshot({path:resolve(directory,"failure.png"),fullPage:true}).catch(()=>{});
    throw error;
  }finally{
    try{await restoreViewer();}finally{await browser.close();}
  }
}
