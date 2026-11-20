// A read-only anonymous GET can be replayed after transport or edge propagation failure.
// Never replay a write, accept a network error as denial, or accept an
// unauthorized success. Each attempt has its own bounded connection.


 // Retry-After applies only to replayable read-only Cloudflare API calls.
 // Return null for invalid or absent headers; never clip a valid server delay.
 export function parseRetryAfterMs(value,now=Date.now()){
   if(typeof value!=="string")return null;
   const text=value.trim();
   if(/^\d+$/.test(text)){
     const seconds=Number(text);
     return Number.isSafeInteger(seconds)&&Number.isSafeInteger(seconds*1000)?
       seconds*1000:null;
   }
   if(!/^[A-Z][a-z]{2}, \d{2} [A-Z][a-z]{2} \d{4} \d{2}:\d{2}:\d{2} GMT$/.test(text))
     return null;
   const at=Date.parse(text);
   return Number.isFinite(at)?Math.max(0,at-now):null;
 }

const retryableReadbackCodes = new Set([
  "ECONNRESET","ETIMEDOUT","EPIPE","UND_ERR_SOCKET",
  "UND_ERR_CONNECT_TIMEOUT","UND_ERR_HEADERS_TIMEOUT","EAI_AGAIN"
]);

export function isRetryableReadbackTransportError(error) {
  const code=error?.cause?.code;
  return ["TimeoutError","AbortError"].includes(error?.name)||
    (error?.name==="TypeError"&&retryableReadbackCodes.has(code));
}

// The live browser suite starts only after three consecutive signed GETs prove
// the exact deployed source. Transport failures and edge propagation may be
// retried; authorization failures and unexpected responses remain fail-closed.
export async function waitForExactSourceSha(endpoint,expectedSha,{
  headers={},
  request=fetch,
  pause=ms=>new Promise(resolve=>setTimeout(resolve,ms)),
  log=entry=>console.log(JSON.stringify(entry)),
  maxAttempts=40,
  stableReads=3,
  intervalMs=1500,
  timeoutMs=10000,
  deadlineMs=90000,
  now=Date.now
}={}) {
  if(!/^[a-f0-9]{40}$/.test(expectedSha))
    throw Error("CRM_E2E_VERSION_EXPECTED_SHA_INVALID");
  if(![maxAttempts,stableReads,intervalMs,timeoutMs,deadlineMs].every(Number.isInteger)||
    maxAttempts<1||stableReads<1||stableReads>maxAttempts||
    intervalMs<0||timeoutMs<1||deadlineMs<1)
    throw Error("CRM_E2E_VERSION_READBACK_OPTIONS_INVALID");

  const started=now(),deadline=started+deadlineMs;
  let stable=0,attempts=0,lastStatus=null,lastSource=null,lastTransportError=null;
  while(attempts<maxAttempts&&stable<stableReads&&now()<deadline) {
    attempts++;
    let response;
    try {
      response=await request(endpoint,{method:"GET",headers,redirect:"manual",
        cache:"no-store",signal:AbortSignal.timeout(
          Math.max(1,Math.min(timeoutMs,deadline-now())))});
    } catch(error) {
      if(!isRetryableReadbackTransportError(error))throw error;
      stable=0;
      lastTransportError=error;
      if(attempts<maxAttempts&&now()<deadline) {
        const delayMs=Math.min(intervalMs,Math.max(0,deadline-now()));
        log({event:"crm.e2e-version-readback",result:"RETRY",attempt:attempts,
          reason:error?.cause?.code??error.name,delay_ms:delayMs});
        if(delayMs>0)await pause(delayMs);
      }
      continue;
    }

    lastTransportError=null;
    lastStatus=response.status;
    lastSource=null;
    if(response.status===401||response.status===403)
      throw Error("CRM_E2E_VERSION_ACCESS_HTTP_"+response.status);
    if(response.status===200) {
      const body=await response.json();
      lastSource=typeof body?.source==="string"?body.source:null;
    } else if(response.status!==404&&response.status!==429&&
      !(response.status>=500&&response.status<=599)) {
      throw Error("CRM_E2E_VERSION_HTTP_"+response.status);
    }

    stable=response.status===200&&lastSource===expectedSha?stable+1:0;
    if(stable===stableReads)break;
    if(attempts<maxAttempts&&now()<deadline) {
      const delayMs=Math.min(intervalMs,Math.max(0,deadline-now()));
      if(attempts===1||attempts%5===0||response.status!==200)
        log({event:"crm.e2e-version-readback",result:"RETRY",attempt:attempts,
          http_status:response.status,observed_sha:lastSource,
          cf_ray:response.headers?.get?.("cf-ray")??null,
          stable_reads:stable,delay_ms:delayMs});
      if(delayMs>0)await pause(delayMs);
    }
  }

  if(stable!==stableReads) {
    const detail=lastTransportError
      ?" transport="+(lastTransportError?.cause?.code??lastTransportError.name)
      :" http_status="+(lastStatus??"none")+" observed_sha="+(lastSource??"none");
    throw Error("CRM_E2E_VERSION_READBACK_UNSTABLE expected="+expectedSha+
      " stable="+stable+"/"+stableReads+" attempts="+attempts+detail);
  }
  log({event:"crm.e2e-version-readback",result:"PASS",source_sha:expectedSha,
    attempts,elapsed_ms:now()-started});
}
export async function verifyAnonymousDenial(endpoint,{
  request=fetch,
  pause=ms=>new Promise(resolve=>setTimeout(resolve,ms)),
  log=entry=>console.log(JSON.stringify(entry))
}={}){
  for(let attempt=1;attempt<=3;attempt++){
    let response;
    try{
      response=await request(endpoint,{method:"GET",redirect:"manual",cache:"no-store",
        headers:{"cache-control":"no-cache",connection:"close"},
        signal:AbortSignal.timeout(20000)});
    }catch(error){
      const code=error?.cause?.code;
      if(!isRetryableReadbackTransportError(error)||attempt===3)throw error;
      const delayMs=attempt*1000;
      log({event:"crm.anonymous-denial-readback",result:"RETRY",attempt,
        reason:code??error.name,delay_ms:delayMs});
      await pause(delayMs);
      continue;
    }
    const ray=response.headers.get("cf-ray");
    const status=response.status;
    // An edge-routing 404 or outage is not a positive Access denial.
    // A different edge can still be propagating after enabling the subdomain.
    // Retry only the GET; a persistent 404/outage is not proof of protection.
    if(attempt<3&&[404,429,502,503,504,524].includes(status)){
      const delayMs=attempt*1000;
      log({event:"crm.anonymous-denial-readback",result:"RETRY",attempt,
        http_status:status,cf_ray:ray,delay_ms:delayMs});
      await pause(delayMs);
      continue;
    }
    // A redirect to Access or explicit 401/403 is the expected result.
    if(![301,302,303,307,308,401,403].includes(status))
      throw Error("Cloudflare Access anonymous denial failed: HTTP "+status+
        " ray="+(ray||"missing"));
    log({event:"crm.anonymous-denial-readback",result:"PASS",attempt,
      http_status:status,cf_ray:ray});
    return;
  }
}

// One-time migration precondition, not a second production normalization path.
// Check only; SQL copies already-canonical strings byte-for-byte. Never log names.
export async function verifyNfcMigrationNames(query){
  const pageSize=200;
  for(const [table,field] of [["crm_party_records","display_name"],["crm_account_records","name"]]){
    let after=null;
    for(;;){
      const sql="SELECT tenant,id,json_extract(state,'$."+field+"') AS name FROM "+table+
        (after?" WHERE (tenant,id)>(?,?)":"")+" ORDER BY tenant,id LIMIT "+pageSize;
      const rows=await query(sql,after??[]);
      if(!Array.isArray(rows)||rows.length>pageSize)
        throw Error("CRM_NFC_MIGRATION_READBACK_INVALID "+table);
      for(const row of rows){
        if(typeof row.tenant!=="string"||typeof row.id!=="string"||
           typeof row.name!=="string"||!row.name||row.name!==row.name.normalize("NFC"))
          throw Error("CRM_NFC_MIGRATION_NONCANONICAL_EXISTING_NAME "+table);
      }
      if(rows.length<pageSize)break;
      const last=rows.at(-1),next=[last.tenant,last.id];
      if(after&&next[0]===after[0]&&next[1]===after[1])
        throw Error("CRM_NFC_MIGRATION_CURSOR_STALLED "+table);
      after=next;
    }
  }
}


// A signed Access session can be routed to an older Workers.dev edge during
// version propagation. Retry only a well-formed, authenticated admin session
// whose explicit source SHA is stale. Invalid auth/role or missing provenance
// must fail immediately; an old version is never accepted as release proof.
export function classifySignedSessionReadback({status,source,session,expectedSha}) {
  if(status===404&&source===null)
    return {kind:"retry",reason:"edge_not_ready"};
  if(status!==200)
    throw Error("CRM_SIGNED_E2E_ACCESS_HTTP_"+status);
  if(session?.authenticated!==true||session?.role!=="admin")
    throw Error("CRM_SIGNED_E2E_ACCESS_SESSION_INVALID");
  if(source===expectedSha)
    return {kind:"pass"};
  if(typeof source==="string"&&/^[a-f0-9]{40}$/i.test(source))
    return {kind:"retry",reason:"stale_worker_version"};
  throw Error("CRM_SIGNED_E2E_ACCESS_SOURCE_INVALID");
}

// Native Cloudflare observability only. Check both the committed Wrangler
// config and the deployed Worker settings; do not trust source config alone.
// Native invocation logs also capture arbitrary request header values under
// $workers.event.request.headers. A live synthetic-header probe demonstrated
// this can include credentials; disable invocation logs, keep app logs/spans.
// Refuse release when settings drift or header capture is re-enabled.
export function verifyTracePrivacySettings(observability) {
  if(observability?.enabled!==true||observability?.redact_query_string!==true||
    observability?.logs?.enabled!==true||observability?.logs?.invocation_logs!==false||
    observability?.traces?.enabled!==true||
    observability.traces.head_sampling_rate!==1)
    throw Error("CRM_TRACE_PRIVACY_SETTINGS_UNSAFE");
  return {traces_enabled:true,query_string_redacted:true,logs_enabled:true,
    invocation_logs_enabled:false,trace_sampling_rate:observability.traces.head_sampling_rate};
}
