// Tests of the real release script with a local Cloudflare API substitute.
// Synthetic fixtures do not constitute live acceptance evidence.
import test from "node:test";
import assert from "node:assert/strict";
import {mkdtempSync,mkdirSync,copyFileSync,writeFileSync,readFileSync,rmSync,existsSync,cpSync} from "node:fs";
import {tmpdir} from "node:os";
import {join} from "node:path";
import {spawnSync} from "node:child_process";
const sha="a".repeat(40),previous="b".repeat(40),database="35fc7ee9-9710-485f-8fb3-f5a398b229b4";
const requestId="11111111-1111-4111-8111-111111111111";
async function subprocessEntry(){
 const {default:assert}=await import("node:assert/strict");
 const {writeFileSync,readFileSync,readdirSync}=await import("node:fs");
 const {mode,variant,sha,database,audit,requestId,rpcPath,fixtureRequests,fixtureWrites}=JSON.parse(process.env.CRM_RELEASE_TEST_FIXTURE);
 const posts=[];
 const nfcLegacy=variant==="nfc-migration"||variant==="nfc-noncanonical";
 let enabled=mode==="deploy",migrated=variant!=="migration-fails"&&!nfcLegacy,published=false,edgeAttempts=0,role="viewer";
 let auditAttempts=0,telemetryAttempts=0,anonymousAttempts=0;
 let telemetryInFlight=0,maxTelemetryInFlight=0;
 const commands=[];
 let db;
 if(mode==="deploy"){
   const {DatabaseSync}=await import("node:sqlite");
   db=new DatabaseSync(":memory:");
   // The ordinary release starts with the full current SQL schema.
   if(migrated||nfcLegacy)for(const name of readdirSync("apps/cloudflare-crm/migrations").filter(n=>n.endsWith(".sql")).sort()){
     if(nfcLegacy&&name>="0016_nfc_search_name.sql")continue;
     db.exec(readFileSync("apps/cloudflare-crm/migrations/"+name,"utf8"));
   }
   if(nfcLegacy){
     db.exec("INSERT INTO crm_tenants VALUES('tenant-a','A')");
     db.prepare("INSERT INTO crm_party_records(tenant,id,state,version,idem_key,idem_digest,request_id,actor) VALUES(?,?,?,?,?,?,?,?)")
       .run("tenant-a","p",JSON.stringify({display_name:variant==="nfc-noncanonical"?"Cafe\u0301":"Café"}),1,"key","digest","request","actor");
   }
   const child=(await import("node:child_process")).default;
   child.spawnSync=(command,args)=>{
     commands.push([command,...args]);
     if(command==="sha256sum")return{status:0}; // Synthetic sealed files, not a real build.
     assert.equal(command,"npx");assert.equal(args[0],"wrangler");
     if(args[1]==="d1"){
       assert.equal(enabled,false,"Schema changes require closed public entry");
       assert.deepEqual(args,["wrangler","d1","migrations","apply","crm-dev","--remote"]);
       if(variant==="migration-fails")return{status:1};
       for(const name of readdirSync("apps/cloudflare-crm/migrations").sort()){
         if(nfcLegacy&&name<"0016_nfc_search_name.sql")continue;
         db.exec(readFileSync("apps/cloudflare-crm/migrations/"+name,"utf8"));
       }
       migrated=true;
     }else{
       assert.deepEqual(args,["wrangler","deploy","--var","GIT_SHA:"+sha]);
       assert.equal(migrated,true);published=true;
     }
     return{status:0};
   };
   (await import("node:module")).syncBuiltinESMExports();
 }
 globalThis.setTimeout=callback=>{queueMicrotask(callback);return 0;};
 globalThis.fetch=async(url,options={})=>{
   const path=new URL(url).pathname;
   const json=value=>new Response(JSON.stringify(value),{headers:{"content-type":"application/json"}});
   const html=status=>new Response("<!DOCTYPE html><title>Cloudflare edge error</title>",{
     status,headers:{"content-type":"text/html","cf-ray":"fixture-ray-123"}});
   if(new URL(url).hostname==="crm-dev.crm-5bd5e1e1.workers.dev"){
     if(mode==="fail-closed")return new Response("disabled",{status:404});
     if(mode==="deploy"){
       if(!enabled)return new Response("disabled",{status:404});
       if(path==="/api/version"){
         if(!new Headers(options.headers).has("CF-Access-Client-Id")){
           anonymousAttempts++;
           if((variant==="anonymous-reset"&&anonymousAttempts===1)||
              variant==="anonymous-persistent-reset"){
             throw Object.assign(new TypeError("fetch failed"),{cause:{code:"ECONNRESET"}});
           }
           if((variant==="anonymous-edge-404"&&anonymousAttempts===1)||
              (variant==="anonymous-delayed-edge-404"&&anonymousAttempts<=8)||
              variant==="anonymous-persistent-404")
             return new Response("propagating",{status:404,headers:{"cf-ray":"fixture-ray-123"}});
           return new Response(variant==="anonymous-success"?"exposed":"denied",
             {status:variant==="anonymous-success"?200:403});
         }
         if(++edgeAttempts<=2)return new Response("propagating",{status:404});
       }
     }
     return json({source:sha});
   }
   assert.equal(new URL(url).hostname,"api.cloudflare.com","Unexpected network target");
   if(options.method==="POST")posts.push(path);
   const wrap=result=>json({success:true,result});
   if(path.endsWith("/deployments"))return wrap({deployments:[{versions:[{version_id:mode==="deploy"&&!published?"version-old":"version-new",percentage:100}]}]});
   if(path.endsWith("/versions/version-old"))return wrap({resources:{bindings:[
     {name:"GIT_SHA",text:"90a352be2a70b74a64e5a774c9a76b7f02c86a5f"},{name:"DB",id:database}]}});
   if(path.endsWith("/time_travel/bookmark"))return wrap({bookmark:"synthetic-bookmark"});
   if(path.endsWith("/settings"))return wrap({observability:{
     enabled:true,redact_query_string:variant!=="trace-redaction-disabled",
     logs:{enabled:variant!=="trace-logs-disabled",head_sampling_rate:1,
       invocation_logs:variant==="trace-invocation-logs-enabled"},
     traces:{enabled:variant!=="trace-disabled",head_sampling_rate:1}
   }});
   if(path.endsWith("/versions/version-new"))return wrap({resources:{bindings:[
     {name:"GIT_SHA",text:variant==="unrelated"?"c".repeat(40):sha},
     {name:"DB",id:variant==="wrong-database"?"other":database}]}});
   if(path.endsWith("/subdomain")){
     if(mode==="deploy"){
       if(options.method==="POST"){
         const body=JSON.parse(options.body);assert.equal(body.previews_enabled,false);
         if(body.enabled)assert.equal(published,true);
         enabled=body.enabled;
       }
       return wrap({enabled,previews_enabled:false});
     }
     if(options.method==="POST")assert.deepEqual(JSON.parse(options.body),{enabled:false,previews_enabled:false});
     return wrap({enabled:false,previews_enabled:false});
   }
   if(path.endsWith("/telemetry/query")){
     telemetryAttempts++;
     if(mode==="verify-data"&&variant.startsWith("telemetry-429-")&&
        (variant.includes("-persistent-")||telemetryAttempts===1)){
       const headers={"content-type":"application/json","cf-ray":"fixture-ray-123"};
       if(variant.endsWith("-seconds"))headers["retry-after"]="3";
       if(variant.endsWith("-date"))headers["retry-after"]=new Date(Date.now()+3000).toUTCString();
       if(variant.endsWith("-invalid"))headers["retry-after"]="not-a-date";
       return new Response(JSON.stringify({success:false,errors:[{code:10102}]}),{status:429,headers});
     }
     if(mode==="verify-data"&&variant==="telemetry-403")
       return new Response(JSON.stringify({success:false,errors:[{code:10000}]}),
         {status:403,headers:{"content-type":"application/json","cf-ray":"fixture-ray-123"}});
     if(mode==="verify-data"&&variant==="transient-html-telemetry"&&telemetryAttempts===1)return html(503);
     if(mode==="verify-data"&&variant==="persistent-html-telemetry")return html(503);
     if(mode==="verify-data"&&variant==="html-success-telemetry")return html(200);
     if(mode==="verify-data"&&((variant==="transient-invalid-json-telemetry"&&telemetryAttempts===1)||
        variant==="persistent-invalid-json-telemetry"))
       return new Response('{"success":true,"result":',{status:200,
         headers:{"content-type":"application/json","cf-ray":"fixture-ray-123"}});
     const query=JSON.parse(options.body);
     assert.equal(query.queryId,"crm-471-native-batch");
     assert.equal(query.view,"events");
     assert.ok(query.limit>=1&&query.limit<=2000);
     assert.equal(query.parameters.filters[0].value,"crm-dev");
     const traceFor=index=>String(index+1).padStart(32,"0"),events=[];
     const add=(id,metadata,source,workers)=>events.push({
       // Match real Cloudflare pages: distinct span and request events may
       // share $metadata.id; only a repeated *pagination cursor* is invalid.
       $metadata:{id:variant==="large-telemetry-scope"&&id==="span-0"?"request-0":id,...metadata},
       ...(source?{source}:{}),...(workers?{$workers:workers}:{})});
     for(let index=0;index<fixtureRequests.length;index++){
       const entry=fixtureRequests[index],traceId=traceFor(index);
       if(variant!=="missing-logs")add("request-"+index,
         {type:"cf-worker",...(variant==="missing-native-trace-id"&&index===0?{}:{traceId})},
         {event:"crm.native.request",request_id:entry.request_id,operation:entry.path,
          status:entry.status,error_code:variant==="wrong-error-code"?"aborted":entry.error_code??null},
         {scriptVersion:{id:"version-new"}});
       if(variant!=="missing-native-span"&&
          !(variant==="missing-recovery-span"&&index===2)&&
          !(variant==="missing-owner-span"&&index===7))
         add("span-"+index,{type:"span",traceId},{
           name:"D1 prepare",traceId:variant==="wrong-native-trace-id"?"f".repeat(32):traceId,
           ...(variant==="missing-d1-span"?{}:{db:{system:"cloudflare-d1"}})});
       if(variant==="unsafe-invocation-event")add("invocation-"+index,
         {type:"cf-worker-event",traceId},null,
         {event:{request:{headers:{authorization:"synthetic-secret"}}}});
       const leaks={
         "synthetic-private-leak":[0,"Browser Party 1234567890123"],
         "synthetic-cookie-leak":[0,"crm-s3-cookie-1234567890123"],
         "synthetic-header-leak":[0,"crm-s3-header-1234567890123"],
         "synthetic-authorization-leak":[0,"Bearer crm-s3-authorization-1234567890123"],
         "synthetic-jwt-leak":[0,"eyJhbGciOiJub25lIn0.eyJzdWIiOiJjcm0tc3ludGhldGljIn0.signature-1234567890123"],
         "synthetic-query-leak":[0,"crm-s3-query-1234567890123"],
         "synthetic-phone-leak":[8,"+12025550102"],
         "synthetic-enrichment-leak":[13,"browser_registry_1234567890123"],
         "synthetic-credential-handle-leak":[13,"browser-credential-handle-1234567890123"],
         "synthetic-mapping-leak":[15,"browser_mapping_1234567890123"],
         "synthetic-csv-leak":[17,"party_id,kind,display_name,resource_version\\r\\nbrowser-import,person,Browser Import 1234567890123,1\\r\\n"]
       };
       if(leaks[variant]?.[0]===index)add("leak-"+index,{type:"log",traceId},
         {traceId,message:leaks[variant][1]});
     }
     // Generate >2000 events for native pagination and duplicate-cursor proofs.
     if(variant==="large-telemetry-scope"||variant==="telemetry-cursor-repeat")
       for(let i=0;i<2200;i++)add("unrelated-"+i,{type:"log",traceId:"unrelated"},
         {event:"unrelated"});
     let start=0;
     if(query.offset!==undefined){
       const i=events.findIndex(e=>e.$metadata.id===query.offset);
       assert.ok(i>=0,"Cursor must come from prior page");
       start=variant==="telemetry-cursor-repeat"?0:i+1;
     }
     const page=events.slice(start,start+query.limit);
     return wrap({events:{events:page,count:page.length}});
   }
   if(path.endsWith("/query")){
     if(mode==="verify-data"){
       auditAttempts++;
       if(variant==="transient-html-audit"&&auditAttempts===1)return html(502);
     }
     if(mode==="fail-closed"&&variant==="restore-role"){
       const {sql,params}=JSON.parse(options.body);
       assert.deepEqual(params,["service:synthetic.access","crm-e2e-123-1-a"]);
       if(sql.startsWith("UPDATE")){
         assert.ok(sql.includes("AND active=1 AND role='viewer'"));role="admin";
         return wrap([{success:true,results:[]}]);
       }
       assert.equal(sql,"SELECT role FROM crm_users WHERE email=? AND tenant_id=? AND active=1");
       return wrap([{success:true,results:[{role}]}]);
     }
     if(mode==="deploy"){
       const {sql,params=[]}=JSON.parse(options.body);
       const names=readdirSync("apps/cloudflare-crm/migrations").filter(n=>n.endsWith(".sql")).sort();
       const ledger=variant==="migration-fails"?names.slice(0,1):
         variant==="migration-history-drift"?["unrecognized.sql"]:nfcLegacy&&!migrated?
            names.filter(name=>name<"0016_nfc_search_name.sql"):names;
       const results=sql.includes("FROM d1_migrations")?ledger.map(name=>({name})):
         !migrated&&!nfcLegacy?[]:db.prepare(sql).all(...params);
       return wrap([{success:true,results}]);
     }
     const {sql,params=[]}=JSON.parse(options.body);
     // One tenant-scoped D1 readback for exact mutations and recovery receipts.
     // Fail on legacy per-request queries.
     assert.ok(sql.startsWith(
       "SELECT request_id,action,subject_id,actor FROM crm_audit WHERE tenant_id=? AND request_id IN ("),
       "Unexpected serial or unscoped audit readback");
     assert.equal(params[0],"fixture-a");
     assert.equal((sql.match(/\?/g)??[]).length,params.length,
       "Every request ID must use a SQL placeholder");
     const ids=params.slice(1);
     assert.equal(new Set(ids).size,ids.length,"Duplicate audit ID");
     const auditRows=variant==="missing-audit"?[]:fixtureWrites
       .filter(write=>ids.includes(write.request_id))
       .flatMap(write=>write.audit.map(row=>({request_id:write.request_id,...row})));
     if(variant==="recovery-audited"){
       const readId=ids.find(id=>!fixtureWrites.some(write=>write.request_id===id));
       assert.ok(readId,"Recovery request must be in the snapshot");
       auditRows.push({request_id:readId,action:"unexpected.recovery",
         subject_id:"duplicate",actor:"service:synthetic.access"});
     }
     return wrap([{success:true,results:auditRows}]);
   }
   throw Error("Unexpected Cloudflare endpoint "+path);
 };
 // A microtask boundary makes any accidental Promise.all burst observable.
 const originalFetch=globalThis.fetch;
 globalThis.fetch=async (...args)=>{
   const telemetry=new URL(args[0]).pathname.endsWith("/telemetry/query");
   if(!telemetry)return originalFetch(...args);
   telemetryInFlight++;
   maxTelemetryInFlight=Math.max(maxTelemetryInFlight,telemetryInFlight);
   try{await Promise.resolve();return await originalFetch(...args);}
   finally{telemetryInFlight--;}
 };
 process.argv[2]=mode;
 try{await import("./deploy.mjs");}finally{
   writeFileSync("posts.json",JSON.stringify(posts));
   writeFileSync("state.json",JSON.stringify({enabled,migrated,published,edgeAttempts,anonymousAttempts,auditAttempts,telemetryAttempts,maxTelemetryInFlight,commands,role}));
   db?.close();
 }
}
function run(mode,variant="valid"){
 const dir=mkdtempSync(join(tmpdir(),"crm-release-test-"));
 const app=join(dir,"apps/cloudflare-crm"),out=join(app,".release");
 mkdirSync(out,{recursive:true});mkdirSync(join(app,"e2e-evidence"));
 for(const name of ["deploy.mjs","release-readback.mjs"])
   copyFileSync(new URL("../scripts/"+name,import.meta.url),join(dir,name));
 const save=(path,value)=>writeFileSync(path,JSON.stringify(value));
 if(variant!=="no-checkpoint"&&mode!=="deploy")save(join(out,"checkpoint.json"),{git_sha:sha,previous_sha:previous,d1_database:database});
 save(join(out,"deployed.json"),{git_sha:sha,worker_version:"version-new",started_at:1,
   trace_settings:{traces_enabled:true,query_string_redacted:true,logs_enabled:true,
     invocation_logs_enabled:false,trace_sampling_rate:1}});
 if(mode==="verify-data")save(join(out,"e2e-tenants.json"),{tenant_a:"fixture-a",actor_a:"service:synthetic.access"});
 if(variant==="restore-role"||variant==="wrong-role-tenant")save(join(out,"e2e-tenants.json"),{
   source_sha:sha,database,run_id:"123",attempt:"1",actor_a:"service:synthetic.access",
   tenant_a:variant==="wrong-role-tenant"?"unrelated-tenant":"crm-e2e-123-1-a"});
 if(mode==="deploy"){
   rmSync(join(out,"deployed.json"));
   copyFileSync(new URL("../wrangler.jsonc",import.meta.url),join(app,"wrangler.jsonc"));
   cpSync(new URL("../migrations/",import.meta.url),join(app,"migrations"),{recursive:true});
   mkdirSync(join(app,"rust-party/build"),{recursive:true});mkdirSync(join(app,"dist"));
   writeFileSync(join(app,"rust-party/build/index.js"),"synthetic wrapper");
   writeFileSync(join(app,"rust-party/build/index_bg.wasm"),"synthetic wasm");
   writeFileSync(join(app,"dist/index.html"),"synthetic UI");
   writeFileSync(join(out,"source-sha.txt"),sha);
 }
 const rpcPath="/crm.parties.v3.PartyService/CreateParty",audit=[{action:"parties.create",subject_id:"party",
   actor:variant==="wrong-actor"?"forged@example.test":"service:synthetic.access"}];
 const originalAdvanceId="22222222-2222-4222-8222-222222222222";
 const subsequentReplayId="33333333-3333-4333-8333-333333333333";
 const originalCloseId="44444444-4444-4444-8444-444444444444";
 const subsequentLookupId="55555555-5555-4555-8555-555555555555";
 const contactCreateId="66666666-6666-4666-8666-666666666666";
 const taskCreateId="77777777-7777-4777-8777-777777777777";
 const partyRecoveryReadId="88888888-8888-4888-8888-888888888888";
 const contactPhoneId="99999999-9999-4999-8999-999999999999";
 const consentWriteId="aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
 const consentReadId="bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
 const privacyWriteId="cccccccc-cccc-4ccc-8ccc-cccccccccccc";
 const privacyReadId="dddddddd-dddd-4ddd-8ddd-dddddddddddd";
 const profileWriteId="eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee";
 const profileReadId="ffffffff-ffff-4fff-8fff-ffffffffffff";
 const mappingWriteId="18181818-1818-4818-8818-181818181818";
 const mappingReadId="19191919-1919-4919-8919-191919191919";
 const csvWriteId="20202020-2020-4020-8020-202020202020";
 const csvReadId="21212121-2121-4121-8121-212121212121";
 const pipelineWriteId="26262626-2626-4626-8626-262626262626";
 const pipelineReadId="27272727-2727-4727-8727-272727272727";
 const viewWriteId="28282828-2828-4828-8828-282828282828";
 const viewReadId="29292929-2929-4929-8929-292929292929";
 const accountWriteId="34343434-3434-4434-8434-343434343434";
 const accountReadId="35353535-3535-4535-8535-353535353535";
 const contactWriteId="36363636-3636-4636-8636-363636363636";
 const contactReadId="37373737-3737-4737-8737-373737373737";
 const relationshipWriteId="38383838-3838-4838-8838-383838383838";
 const relationshipReadId="39393939-3939-4939-8939-393939393939";
 const taskWriteId="40404040-4040-4040-8040-404040404040";
 const taskReadId="41414141-4141-4141-8141-414141414141";
 const invalidDealId="23232323-2323-4323-8323-232323232323";
 const deniedPartyId="24242424-2424-4424-8424-242424242424";
 const conflictPartyId="25252525-2525-4525-8525-252525252525";
 const advancePath="/crm.sales.v2.DealService/AdvanceStage";
 const closePath="/crm.sales.v2.DealService/CloseDeal";
 const lookupPath="/crm.sales.v2.DealService/GetDeal";
 const contactPath="/crm.contact_points.v2.ContactPointService/CreateContactPoint";
 const taskPath="/crm.activities.v2.TaskService/CreateTask";
 const partyLookupPath="/crm.parties.v3.PartyService/GetParty";
 const consentPath="/crm.consents.v2.ConsentAuthorizationService/SuppressCommunication";
 const consentLookupPath="/crm.consents.v2.ConsentAuthorizationService/GetConsentAuthorization";
 const privacyPath="/crm.customer_privacy.v2.PrivacyCaseService/CreatePrivacyCase";
 const privacyLookupPath="/crm.customer_privacy.v2.PrivacyCaseService/GetPrivacyCase";
 const profilePath="/crm.customer_enrichment.v2.EnrichmentConfigurationService/PublishProviderProfile";
 const profileLookupPath="/crm.customer_enrichment.v2.EnrichmentConfigurationService/GetProviderProfile";
 const mappingPath="/crm.customer_enrichment.v2.EnrichmentConfigurationService/PublishMapping";
 const mappingLookupPath="/crm.customer_enrichment.v2.EnrichmentConfigurationService/GetMapping";
 const csvPath="/crm.customer_data_operations.v2.PartyCsvService/ApplyPartyImport";
 const csvLookupPath="/crm.customer_data_operations.v2.PartyCsvService/PreviewPartyImport";
 const pipelinePath="/crm.sales.v2.DealService/CreatePipeline";
 const pipelineLookupPath="/crm.sales.v2.DealService/GetPipeline";
 const viewPath="/crm.sales.v2.DealService/CreateSalesView";
 const viewLookupPath="/crm.sales.v2.DealService/GetSalesView";
 const accountPath="/crm.accounts.v2.AccountService/CreateAccount";
 const accountLookupPath="/crm.accounts.v2.AccountService/GetAccount";
 const contactLookupPath="/crm.contact_points.v2.ContactPointService/GetContactPoint";
 const relationshipPath="/crm.party_relationships.v2.PartyRelationshipService/CreatePartyRelationship";
 const relationshipLookupPath="/crm.party_relationships.v2.PartyRelationshipService/GetPartyRelationship";
 const taskLookupPath="/crm.activities.v2.TaskService/GetTask";
 const invalidDealPath="/crm.sales.v2.DealService/CreateDeal";
 const conflictPartyPath="/crm.parties.v3.PartyService/UpdateParty";
 const fixtureRequests=[
   {path:rpcPath,status:200,request_id:requestId,source_sha:sha,runtime:"native-workers-rs"},
   {path:advancePath,status:200,request_id:originalAdvanceId,source_sha:sha,runtime:"native-workers-rs"},
   {path:advancePath,status:200,request_id:subsequentReplayId,source_sha:sha,runtime:"native-workers-rs"},
   {path:closePath,status:200,request_id:originalCloseId,source_sha:sha,runtime:"native-workers-rs"},
   {path:lookupPath,status:200,request_id:subsequentLookupId,source_sha:sha,runtime:"native-workers-rs"},
   {path:contactPath,status:200,request_id:contactCreateId,source_sha:sha,runtime:"native-workers-rs"},
   {path:taskPath,status:200,request_id:taskCreateId,source_sha:sha,runtime:"native-workers-rs"},
   {path:partyLookupPath,status:200,request_id:partyRecoveryReadId,source_sha:sha,runtime:"native-workers-rs"},
   {path:contactPath,status:200,request_id:contactPhoneId,source_sha:sha,runtime:"native-workers-rs"},
   {path:consentPath,status:200,request_id:consentWriteId,source_sha:sha,runtime:"native-workers-rs"},
   {path:consentLookupPath,status:200,request_id:consentReadId,source_sha:sha,runtime:"native-workers-rs"},
   {path:privacyPath,status:200,request_id:privacyWriteId,source_sha:sha,runtime:"native-workers-rs"},
   {path:privacyLookupPath,status:200,request_id:privacyReadId,source_sha:sha,runtime:"native-workers-rs"},
   {path:profilePath,status:200,request_id:profileWriteId,source_sha:sha,runtime:"native-workers-rs"},
   {path:profileLookupPath,status:200,request_id:profileReadId,source_sha:sha,runtime:"native-workers-rs"},
   {path:mappingPath,status:200,request_id:mappingWriteId,source_sha:sha,runtime:"native-workers-rs"},
   {path:mappingLookupPath,status:200,request_id:mappingReadId,source_sha:sha,runtime:"native-workers-rs"},
   {path:csvPath,status:200,request_id:csvWriteId,source_sha:sha,runtime:"native-workers-rs"},
   {path:csvLookupPath,status:200,request_id:csvReadId,source_sha:sha,runtime:"native-workers-rs"},
   {path:pipelinePath,status:200,request_id:pipelineWriteId,source_sha:sha,runtime:"native-workers-rs"},
   {path:pipelineLookupPath,status:200,request_id:pipelineReadId,source_sha:sha,runtime:"native-workers-rs"},
   {path:viewPath,status:200,request_id:viewWriteId,source_sha:sha,runtime:"native-workers-rs"},
   {path:viewLookupPath,status:200,request_id:viewReadId,source_sha:sha,runtime:"native-workers-rs"},
   {path:accountPath,status:200,request_id:accountWriteId,source_sha:sha,runtime:"native-workers-rs"},
   {path:accountLookupPath,status:200,request_id:accountReadId,source_sha:sha,runtime:"native-workers-rs"},
   {path:contactPath,status:200,request_id:contactWriteId,source_sha:sha,runtime:"native-workers-rs"},
   {path:contactLookupPath,status:200,request_id:contactReadId,source_sha:sha,runtime:"native-workers-rs"},
   {path:relationshipPath,status:200,request_id:relationshipWriteId,source_sha:sha,runtime:"native-workers-rs"},
   {path:relationshipLookupPath,status:200,request_id:relationshipReadId,source_sha:sha,runtime:"native-workers-rs"},
   {path:taskPath,status:200,request_id:taskWriteId,source_sha:sha,runtime:"native-workers-rs"},
   {path:taskLookupPath,status:200,request_id:taskReadId,source_sha:sha,runtime:"native-workers-rs"},
   {path:invalidDealPath,status:400,request_id:invalidDealId,error_code:"invalid_argument",source_sha:sha,runtime:"native-workers-rs"},
   {path:rpcPath,status:403,request_id:deniedPartyId,error_code:"permission_denied",source_sha:sha,runtime:"native-workers-rs"},
   {path:conflictPartyPath,status:409,request_id:conflictPartyId,error_code:"aborted",source_sha:sha,runtime:"native-workers-rs"}
 ];
 // Match the cardinality observed in the real exact-SHA browser artifact.
 // Extra read-only RPCs must retain individual source/trace evidence.
 if(variant==="large-telemetry-scope")for(let i=fixtureRequests.length;i<338;i++)
   fixtureRequests.push({path:partyLookupPath,status:200,
     request_id:"abababab-abab-4bab-8bab-"+i.toString(16).padStart(12,"0"),
     source_sha:sha,runtime:"native-workers-rs"});
 const fixtureWrites=[
   {path:rpcPath,request_id:requestId,audit},
   {path:advancePath,request_id:originalAdvanceId,
     audit:[{action:"sales_deals.advance",subject_id:"deal",actor:"service:synthetic.access"}]},
   {path:closePath,request_id:originalCloseId,
     audit:[{action:"sales_deals.close",subject_id:"deal",actor:"service:synthetic.access"}]},
   {path:contactPath,request_id:contactCreateId,
     audit:[{action:"contact_points.create",subject_id:"contact",actor:"service:synthetic.access"}]},
   {path:taskPath,request_id:taskCreateId,
     audit:[{action:"activity_tasks.create",subject_id:"task",actor:"service:synthetic.access"}]},
   {path:contactPath,request_id:contactPhoneId,
     audit:[{action:"contact_points.create",subject_id:"phone",actor:"service:synthetic.access"}]},
   {path:consentPath,request_id:consentWriteId,
     audit:[{action:"consents.operator_suppress",subject_id:"consent",actor:"service:synthetic.access"}]},
   {path:privacyPath,request_id:privacyWriteId,
     audit:[{action:"customer_privacy.create",subject_id:"privacy",actor:"service:synthetic.access"}]},
   {path:profilePath,request_id:profileWriteId,
     audit:[{action:"customer_enrichment.profile_publish",subject_id:"profile",actor:"service:synthetic.access"}]},
   {path:mappingPath,request_id:mappingWriteId,
     audit:[{action:"customer_enrichment.mapping_publish",subject_id:"mapping",actor:"service:synthetic.access"}]},
   {path:csvPath,request_id:csvWriteId,
     audit:[{action:"parties.create",subject_id:"csv-party",actor:"service:synthetic.access"}]},
   {path:pipelinePath,request_id:pipelineWriteId,
     audit:[{action:"sales_pipeline.create",subject_id:"pipeline",actor:"service:synthetic.access"}]},
   {path:viewPath,request_id:viewWriteId,
     audit:[{action:"sales_view.create",subject_id:"view",actor:"service:synthetic.access"}]},
   {path:accountPath,request_id:accountWriteId,
     audit:[{action:"accounts.create",subject_id:"account",actor:"service:synthetic.access"}]},
   {path:contactPath,request_id:contactWriteId,
     audit:[{action:"contact_points.create",subject_id:"contact-point",actor:"service:synthetic.access"}]},
   {path:relationshipPath,request_id:relationshipWriteId,
     audit:[{action:"party_relationships.create",subject_id:"relationship",actor:"service:synthetic.access"}]},
   {path:taskPath,request_id:taskWriteId,
     audit:[{action:"activity_tasks.create",subject_id:"task",actor:"service:synthetic.access"}]}
 ];
 const recovery=[
   {kind:"replay",original_request_id:originalAdvanceId,subsequent_request_id:subsequentReplayId},
   {kind:"lookup",original_request_id:originalCloseId,subsequent_request_id:subsequentLookupId}
 ];
 const ownerRecovery=[
   {owner:"party",kind:"lookup",original_request_id:requestId,subsequent_request_id:partyRecoveryReadId},
   {owner:"consent",kind:"lookup",original_request_id:consentWriteId,subsequent_request_id:consentReadId},
   {owner:"privacy",kind:"lookup",original_request_id:privacyWriteId,subsequent_request_id:privacyReadId},
   {owner:"enrichment-profile",kind:"lookup",original_request_id:profileWriteId,subsequent_request_id:profileReadId},
   {owner:"enrichment-mapping",kind:"lookup",original_request_id:mappingWriteId,subsequent_request_id:mappingReadId},
   {owner:"party-csv-import",kind:"lookup",original_request_id:csvWriteId,subsequent_request_id:csvReadId},
   {owner:"sales-pipeline",kind:"lookup",original_request_id:pipelineWriteId,subsequent_request_id:pipelineReadId},
   {owner:"sales-view",kind:"lookup",original_request_id:viewWriteId,subsequent_request_id:viewReadId},
   {owner:"account",kind:"lookup",original_request_id:accountWriteId,subsequent_request_id:accountReadId},
   {owner:"contact-point",kind:"lookup",original_request_id:contactWriteId,subsequent_request_id:contactReadId},
   {owner:"relationship",kind:"lookup",original_request_id:relationshipWriteId,subsequent_request_id:relationshipReadId},
   {owner:"task",kind:"lookup",original_request_id:taskWriteId,subsequent_request_id:taskReadId}
 ];
 save(join(app,"e2e-evidence/typed-browser.json"),{result:"PASS",source_sha:sha,features:variant==="wrong-features"?["native-ci"]:["native"],
   tenant:"fixture-a",started_at:2,steps:["synthetic fixture"],requests:fixtureRequests,
   writes:fixtureWrites,
   recovery:variant==="missing-recovery"?[]:
     variant==="wrong-recovery-request"?[recovery[0],{...recovery[1],subsequent_request_id:originalCloseId}]:recovery,
   owner_recovery:variant==="missing-owner-recovery"?[]:variant==="wrong-owner-recovery"?
     [{...ownerRecovery[0],subsequent_request_id:originalAdvanceId}]:ownerRecovery,
   synthetic_private_probes:variant==="missing-private-probes"?[]:[
     {kind:"party-name",request_id:requestId,value:"Browser Party 1234567890123"},
     {kind:"cookie",request_id:requestId,value:"crm-s3-cookie-1234567890123"},
     {kind:"header",request_id:requestId,value:"crm-s3-header-1234567890123"},
     {kind:"authorization",request_id:requestId,value:"Bearer crm-s3-authorization-1234567890123"},
     {kind:"jwt",request_id:requestId,value:"eyJhbGciOiJub25lIn0.eyJzdWIiOiJjcm0tc3ludGhldGljIn0.signature-1234567890123"},
     {kind:"query-string",request_id:requestId,value:"crm-s3-query-1234567890123"},
     {kind:"contact-email",request_id:contactCreateId,value:"browser-1234567890123@example.org"},
     {kind:"contact-phone",request_id:contactPhoneId,value:"+12025550102"},
     {kind:"task-subject",request_id:taskCreateId,value:"Browser Task 1234567890123"},
     {kind:"enrichment-provider-key",request_id:profileWriteId,value:"browser_registry_1234567890123"},
     {kind:"enrichment-license",request_id:profileWriteId,value:"Browser test license"},
     {kind:"enrichment-credential-handle",request_id:profileWriteId,value:"browser-credential-handle-1234567890123"},
     {kind:"enrichment-mapping-key",request_id:mappingWriteId,value:"browser_mapping_1234567890123"},
     {kind:"csv-party-name",request_id:csvWriteId,value:"Browser Import 1234567890123"},
     {kind:"csv-payload",request_id:csvWriteId,
       value:"party_id,kind,display_name,resource_version\r\nbrowser-import,person,Browser Import 1234567890123,1\r\n"}
   ]});
 try{
   const result=spawnSync(process.execPath,["--input-type=module","-e","("+subprocessEntry.toString()+")()"],{cwd:dir,encoding:"utf8",
     env:{...process.env,GITHUB_SHA:sha,GITHUB_REF:"refs/heads/dev",GITHUB_EVENT_NAME:"push",
       GITHUB_RUN_ID:"123",GITHUB_RUN_ATTEMPT:"1",
       CLOUDFLARE_ACCOUNT_ID:"5bd5e1e1e813f061afc259e91334262d",CRM_D1_DATABASE_ID:database,
       CLOUDFLARE_API_TOKEN:"synthetic-token",CRM_E2E_A_CLIENT_ID:"synthetic.access",CRM_E2E_A_CLIENT_SECRET:"synthetic-secret",
       CRM_RELEASE_TEST_FIXTURE:JSON.stringify({mode,variant,sha,database,audit,requestId,rpcPath,fixtureRequests,fixtureWrites})}});
   return {status:result.status,error:result.stderr,stdout:result.stdout,posts:JSON.parse(readFileSync(join(dir,"posts.json"),"utf8")),
     state:JSON.parse(readFileSync(join(dir,"state.json"),"utf8")),deployed:existsSync(join(out,"deployed.json")),
     stopped:existsSync(join(out,"failed-release.json")),accepted:existsSync(join(out,"accepted.json")),checkpoint:existsSync(join(out,"checkpoint.json"))};
 }finally{rmSync(dir,{recursive:true,force:true});}
}
test("failed candidate stops public entry and never redeploys its predecessor",()=>{
 const result=run("fail-closed");
 assert.equal(result.status,0,result.error);assert.equal(result.stopped,true);
 assert.equal(result.posts.length,1);assert.ok(result.posts[0].endsWith("/subdomain"));
});
test("failure before a release checkpoint does not change the deployed app",()=>{
 const result=run("fail-closed","no-checkpoint");
 assert.equal(result.status,0,result.error);assert.deepEqual(result.posts,[]);
});
for(const variant of ["unrelated","wrong-database"])test("failure cannot stop "+variant+" deployment",()=>{
 const result=run("fail-closed",variant);
 assert.notEqual(result.status,0);assert.deepEqual(result.posts,[]);assert.equal(result.stopped,false);
});
test("matching browser, D1 and version-specific Logs evidence passes the release validator",()=>{
 const result=run("verify-data");
 assert.equal(result.status,0,result.error);assert.equal(result.accepted,true);
 assert.equal(result.state.auditAttempts,1,
   "All write and recovery proofs must share one D1 snapshot");
 assert.equal(result.state.maxTelemetryInFlight,1,
   "Cloudflare Telemetry must use one native cursor reader");
});
test("338 exact RPC readbacks retain request and span evidence without a burst",()=>{
 const result=run("verify-data","large-telemetry-scope");
 assert.equal(result.status,0,result.error);
 assert.equal(result.accepted,true);
 assert.equal(result.state.maxTelemetryInFlight,1);
 assert.ok(result.stdout.includes('"requests":338'));
 assert.ok(result.state.telemetryAttempts>=2&&result.state.telemetryAttempts<10,
   "338 RPCs and native spans must use only a few cursor pages");
});
test("repeating a Cloudflare telemetry cursor cannot pass release acceptance",()=>{
 const result=run("verify-data","telemetry-cursor-repeat");
 assert.notEqual(result.status,0);assert.equal(result.accepted,false);
 assert.match(result.error,/CRM_TELEMETRY_CURSOR_REPEATED/);
 assert.ok(result.state.telemetryAttempts<=3);
});
test("Retry-After delta-seconds, HTTP-date and invalid headers are parsed safely",async()=>{
 const {parseRetryAfterMs}=await import("../scripts/release-readback.mjs");
 const now=Date.parse("Fri, 20 Nov 2026 18:00:00 GMT");
 assert.equal(parseRetryAfterMs("3",now),3000);
 assert.equal(parseRetryAfterMs("Fri, 20 Nov 2026 18:00:04 GMT",now),4000);
 assert.equal(parseRetryAfterMs("Fri, 20 Nov 2026 17:59:59 GMT",now),0);
 for(const invalid of [null,"","not-a-date","-1","3.5","999999999999999999999999"])
   assert.equal(parseRetryAfterMs(invalid,now),null);
});
for(const variant of [
 "telemetry-429-transient-seconds","telemetry-429-transient-date",
 "telemetry-429-transient-invalid","telemetry-429-transient-missing"
])test("transient Telemetry 429 recovers without retrying writes: "+variant,()=>{
 const result=run("verify-data",variant);
 assert.equal(result.status,0,result.error);
 assert.equal(result.accepted,true);
 assert.equal(result.state.maxTelemetryInFlight,1);
 assert.equal(result.posts.filter(p=>p.endsWith("/subdomain")).length,0);
 const retry=result.stdout.split("\n").filter(x=>x.includes("crm.cf-readback-retry"))
   .map(x=>JSON.parse(x)).find(x=>x.http_status===429);
 assert.ok(retry,"A 429 must be observable");
 if(variant.endsWith("-seconds"))assert.ok(retry.delay_ms>=3000,
   "A valid Retry-After must not be shortened");
});
for(const variant of ["telemetry-429-persistent-seconds","telemetry-429-persistent-date"])
 test("persistent Telemetry 429 stays fail-closed after three reads: "+variant,()=>{
   const result=run("verify-data",variant);
   assert.notEqual(result.status,0);
   assert.equal(result.accepted,false);
   assert.match(result.error,/HTTP 429/);
   assert.ok(result.state.telemetryAttempts>=3&&result.state.telemetryAttempts<=6,
     "The cursor reader must stop after at most three attempts");
   assert.equal(result.state.maxTelemetryInFlight,1);
   assert.equal(result.posts.filter(p=>p.endsWith("/subdomain")).length,0);
 });
test("Telemetry nonretryable 403 fails closed without another POST",()=>{
 const result=run("verify-data","telemetry-403");
 assert.notEqual(result.status,0);
 assert.equal(result.accepted,false);
 assert.match(result.error,/HTTP 403/);
 assert.ok(result.state.telemetryAttempts>=1&&result.state.telemetryAttempts<=2);
});
for(const variant of ["transient-html-audit","transient-html-telemetry","transient-invalid-json-telemetry"])
 test("transient HTML Cloudflare API readback is retried without replaying a mutation: "+variant,()=>{
   const result=run("verify-data",variant);
   assert.equal(result.status,0,result.error);
   assert.equal(result.accepted,true);
   assert.equal(result.posts.filter(p=>p.endsWith("/subdomain")).length,0,
     "readback retries must not change public entry or redeploy");
 });
for(const variant of ["persistent-html-telemetry","html-success-telemetry","persistent-invalid-json-telemetry"])
 test("non-recoverable HTML Cloudflare response remains fail-closed: "+variant,()=>{
   const result=run("verify-data",variant);
   assert.notEqual(result.status,0);
   assert.equal(result.accepted,false);
   assert.match(result.error,/NON_JSON/);
   assert.match(result.error,/cf_ray=fixture-ray-123/);
   // Only the read-only cursor query may retry.
   if(variant==="persistent-invalid-json-telemetry")
     assert.ok(result.state.telemetryAttempts>=3&&result.state.telemetryAttempts<=6);
   if(variant==="html-success-telemetry")
     assert.ok(result.state.telemetryAttempts>=1&&result.state.telemetryAttempts<=2);
   assert.equal(result.state.maxTelemetryInFlight,1);
   assert.ok(!result.error.includes("<!DOCTYPE"),
     "never print an HTML error body or upstream credentials");
 });
for(const variant of ["wrong-features","missing-audit","missing-logs","wrong-error-code","wrong-actor",
  "trace-redaction-disabled","trace-logs-disabled","trace-disabled",
  "trace-invocation-logs-enabled","missing-native-trace-id",
  "missing-native-span","missing-d1-span","wrong-native-trace-id",
  "unsafe-invocation-event","missing-recovery","wrong-recovery-request",
  "recovery-audited","missing-recovery-span",
  "missing-owner-recovery","wrong-owner-recovery","missing-owner-span",
  "missing-private-probes","synthetic-private-leak",
  "synthetic-cookie-leak","synthetic-header-leak","synthetic-authorization-leak",
  "synthetic-jwt-leak","synthetic-query-leak","synthetic-phone-leak",
  "synthetic-enrichment-leak","synthetic-credential-handle-leak",
  "synthetic-mapping-leak","synthetic-csv-leak"])test("release rejects "+variant,()=>{
 const result=run("verify-data",variant);
 assert.notEqual(result.status,0);assert.equal(result.accepted,false);
});
test("deploy never reopens public entry with unsafe Cloudflare tracing settings",()=>{
  for(const variant of ["trace-redaction-disabled","trace-logs-disabled",
    "trace-invocation-logs-enabled","trace-disabled"]){
    const result=run("deploy",variant);
    assert.notEqual(result.status,0);
    assert.equal(result.deployed,false);
    assert.equal(result.state.published,true);
    assert.equal(result.posts.filter(path=>path.endsWith("/subdomain")).length,0,
      "No public-entry changes after unsafe trace settings readback");
    assert.match(result.error,/CRM_TRACE_PRIVACY_SETTINGS_UNSAFE/);
  }
});
test("unchanged D1 ledger keeps public entry online and signed version checks Access",()=>{
 const result=run("deploy");
 assert.equal(result.status,0,result.error);assert.equal(result.deployed,true);
 assert.equal(result.state.edgeAttempts,3);assert.equal(result.state.enabled,true);
 assert.equal(result.state.migrated,true);assert.equal(result.state.published,true);
 assert.equal(result.state.commands.filter(c=>c[2]==="d1"&&c[3]==="migrations").length,0);
 assert.equal(result.posts.filter(path=>path.endsWith("/subdomain")).length,1,
   "Code-only release must not stop public entry before deployment");
});
test("anonymous transport reset recovers through real deploy script without replaying writes",()=>{
 const result=run("deploy","anonymous-reset");
 assert.equal(result.status,0,result.error);
 assert.equal(result.deployed,true);
 assert.equal(result.state.anonymousAttempts,2);
 assert.equal(result.state.commands.filter(c=>c[2]==="deploy").length,1);
});
test("temporary anonymous edge 404 requires a later positive denial without redeploying",()=>{
 const result=run("deploy","anonymous-edge-404");
 assert.equal(result.status,0,result.error);
 assert.equal(result.deployed,true);
 assert.equal(result.state.anonymousAttempts,2);
 assert.equal(result.state.commands.filter(c=>c[2]==="deploy").length,1);
});
test("delayed Access edge propagation requires a positive denial, not a 404",()=>{
 const result=run("deploy","anonymous-delayed-edge-404");
 assert.equal(result.status,0,result.error);
 assert.equal(result.deployed,true);
 assert.equal(result.state.anonymousAttempts,9);
 assert.equal(result.state.commands.filter(c=>c[2]==="deploy").length,1);
});
for(const variant of ["anonymous-persistent-reset","anonymous-persistent-404","anonymous-success"])
 test("release rejects "+variant+" without replaying deployment",()=>{
   const result=run("deploy",variant);
   assert.notEqual(result.status,0);
   assert.equal(result.deployed,false);
   assert.equal(result.state.anonymousAttempts,
   variant==="anonymous-success"?1:variant==="anonymous-persistent-404"?40:3);
   assert.equal(result.state.commands.filter(c=>c[2]==="deploy").length,1);
 });
test("unknown D1 migration history rejects the release before changing public entry",()=>{
 const result=run("deploy","migration-history-drift");
 assert.notEqual(result.status,0);assert.equal(result.state.published,false);
 assert.equal(result.posts.filter(path=>path.endsWith("/subdomain")).length,0);
});
test("failed migration never publishes or reopens the Worker",()=>{
 const result=run("deploy","migration-fails");
 assert.notEqual(result.status,0);assert.equal(result.deployed,false);
 assert.equal(result.state.enabled,false);assert.equal(result.state.published,false);
 assert.equal(result.state.commands.filter(c=>c[2]==="deploy").length,0);
});
test("failure cleanup restores only the current run's signed E2E actor",()=>{
 const result=run("fail-closed","restore-role");
 assert.equal(result.status,0,result.error);assert.equal(result.stopped,true);
 assert.equal(result.state.role,"admin");
 assert.equal(result.posts.filter(path=>path.endsWith("/query")).length,2);
});
test("failure cleanup refuses to change a role in an unrelated tenant",()=>{
 const result=run("fail-closed","wrong-role-tenant");
 assert.notEqual(result.status,0);assert.equal(result.stopped,true);
 assert.equal(result.posts.filter(path=>path.endsWith("/query")).length,0);
});

test("NFC schema migration copies canonical names without changing receipts or audit",async()=>{
 const {DatabaseSync}=await import("node:sqlite");
 const {readdirSync}=await import("node:fs");
 const {verifyNfcMigrationNames}=await import("../scripts/release-readback.mjs");
 const db=new DatabaseSync(":memory:");
 try{
   const dir=new URL("../migrations/",import.meta.url);
   const migration="0016_nfc_search_name.sql";
   for(const name of readdirSync(dir).filter(n=>n.endsWith(".sql")&&n<migration).sort())
     db.exec(readFileSync(new URL(name,dir),"utf8"));
   db.exec("INSERT INTO crm_tenants VALUES('tenant-a','A')");
   const party=JSON.stringify({display_name:"Café Йошкар-Ола"});
   const account=JSON.stringify({name:"Café Йошкар-Ола"});
   db.prepare("INSERT INTO crm_party_records(tenant,id,state,version,idem_key,idem_digest,request_id,actor) VALUES(?,?,?,?,?,?,?,?)")
     .run("tenant-a","p",party,1,"p-create","digest","request","actor");
   db.prepare("INSERT INTO crm_account_records(tenant,id,primary_party,state,version,idem_key,idem_digest,request_id,actor) VALUES(?,?,?,?,?,?,?,?,?)")
     .run("tenant-a","a","p",account,1,"a-create","digest","request","actor");
   const read=async(sql,params)=>db.prepare(sql).all(...params);
   await verifyNfcMigrationNames(read);
   const counts=()=>["crm_party_receipts","crm_account_receipts","crm_audit"].map(t=>db.prepare("SELECT COUNT(*) AS n FROM "+t).get().n);
   const before=counts();
   db.exec(readFileSync(new URL(migration,dir),"utf8"));
   assert.deepEqual(counts(),before);
   assert.equal(db.prepare("SELECT search_name FROM crm_party_records WHERE id='p'").get().search_name,"Café Йошкар-Ола");
   assert.equal(db.prepare("SELECT search_name FROM crm_account_records WHERE id='a'").get().search_name,"Café Йошкар-Ола");
   assert.throws(()=>db.prepare("INSERT INTO crm_party_records(tenant,id,state,version,idem_key,idem_digest,request_id,actor) VALUES(?,?,?,?,?,?,?,?)")
     .run("tenant-a","missing-key",party,1,"bad-create","digest","request","actor"),/NFC_SEARCH_NAME_REQUIRED/);
   assert.deepEqual(counts(),before);
   assert.equal(db.prepare("SELECT state FROM crm_party_records WHERE id='p'").get().state,party);
   assert.equal(db.prepare("SELECT state FROM crm_account_records WHERE id='a'").get().state,account);
 }finally{db.close();}
});
test("NFC migration preflight fails closed and does not expose names",async()=>{
 const {verifyNfcMigrationNames}=await import("../scripts/release-readback.mjs");
 await assert.rejects(verifyNfcMigrationNames(async()=>[{tenant:"tenant-a",id:"p",name:"private-Cafe\u0301"}]),
  error=>error.message==="CRM_NFC_MIGRATION_NONCANONICAL_EXISTING_NAME crm_party_records");
 let calls=0;
 await verifyNfcMigrationNames(async(sql,params)=>{
   calls++;
   if(calls===1){assert.deepEqual(params,[]);return Array.from({length:200},(_,i)=>({tenant:"t",id:String(i).padStart(3,"0"),name:"Café"}));}
   if(calls===2)assert.deepEqual(params,["t","199"]);
   return [];
 });
 assert.equal(calls,3);
});

test("pending NFC migration runs under the same protected release path",()=>{
 const result=run("deploy","nfc-migration");
 assert.equal(result.status,0,result.error);
 assert.equal(result.deployed,true);
 assert.equal(result.state.migrated,true);
 assert.equal(result.state.enabled,true);
 assert.equal(result.state.commands.filter(c=>c[2]==="d1"&&c[3]==="migrations").length,1);
});
test("noncanonical existing names block release before checkpoint and traffic closure",()=>{
 const result=run("deploy","nfc-noncanonical");
 assert.notEqual(result.status,0);
 assert.match(result.error,/CRM_NFC_MIGRATION_NONCANONICAL_EXISTING_NAME/);
 assert.equal(result.checkpoint,false);
 assert.equal(result.state.enabled,true);
 assert.equal(result.state.published,false);
 assert.equal(result.state.commands.filter(c=>c[2]==="d1"&&c[3]==="migrations").length,0);
 assert.equal(result.posts.filter(p=>p.endsWith("/subdomain")).length,0);
});
