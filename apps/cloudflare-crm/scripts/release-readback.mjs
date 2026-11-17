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
