// A read-only anonymous GET can be replayed after transport or edge propagation failure.
// Never replay a write, accept a network error as denial, or accept an
// unauthorized success. Each attempt has its own bounded connection.
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
      const retryable=["TimeoutError","AbortError"].includes(error?.name)||
        (error?.name==="TypeError"&&["ECONNRESET","ETIMEDOUT","EPIPE",
          "UND_ERR_SOCKET","UND_ERR_CONNECT_TIMEOUT","UND_ERR_HEADERS_TIMEOUT",
          "EAI_AGAIN"].includes(code));
      if(!retryable||attempt===3)throw error;
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
