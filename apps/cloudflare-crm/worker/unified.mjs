// One public Worker entrypoint: existing CRM + the original Rust Party owner.
// worker-build produces an EntryPoint class (not a fetch object). It is
// instantiated inside this same public Worker, never deployed separately.
// All user-supplied internal tenant headers are stripped before invocation.
import { crmFetch, resolveActor } from "./index.mjs";
import rustParty from "../rust-party/build/index.js";

const partyRoute = pathname => /^\/api\/parties(?:\/[^/]+)?$/.test(pathname);

export async function unifiedFetch(request, env, ctx, native = rustParty) {
  const url = new URL(request.url);
  if (!partyRoute(url.pathname)) return crmFetch(request, env, ctx);

  const headers = new Headers(request.headers);
  headers.delete("x-crm-ci-tenant");
  headers.delete("x-crm-native-tenant");
  headers.delete("x-crm-internal-request-id");
  let tenant;
  const requestId = crypto.randomUUID();
  // Only this trusted entrypoint may establish the Rust-to-D1 audit id.
  headers.set("x-crm-internal-request-id", requestId);

  // Explicit loopback-only test build. This flag is NEVER present in crm-dev.
  if (env.CRM_CI_LOCAL_ONLY === "yes" &&
      ["localhost", "127.0.0.1"].includes(url.hostname)) {
    const candidate = request.headers.get("x-crm-ci-tenant");
    if (["tenant-a", "tenant-b"].includes(candidate)) {
      tenant = candidate;
      headers.set("x-crm-ci-tenant", candidate);
    }
  } else {
    const actor = await resolveActor(request, env, ctx);
    tenant = actor?.tenant;
    if (tenant) headers.set("x-crm-native-tenant", tenant);
  }
  if (!tenant) return Response.json({code:"AUTH_REQUIRED",error:"Authentication required",request_id:requestId},
    {status:401,headers:{"cache-control":"no-store","x-crm-error-code":"AUTH_REQUIRED","x-request-id":requestId}});
  const started = performance.now();
  try {
    // workers-rs exports a WorkerEntrypoint subclass, which is instantiated with
    // the current ExecutionContext and Environment inside the SAME Worker.
    const rustHandler = new native(ctx, env);
    const response = await rustHandler.fetch(new Request(request, {headers}));
    const outbound = new Headers(response.headers);
    outbound.set("x-request-id", requestId);
    outbound.set("cache-control","no-store");
    if (response.status >= 400) {
      // The Rust implementation emits stable business error codes in JSON.
      const payload = await response.clone().json().catch(()=>null);
      if (payload?.code) outbound.set("x-crm-error-code", payload.code);
    }
    console.log(JSON.stringify({event:"crm.rust_party",request_id:requestId,
      source:env.GIT_SHA||"local",status:response.status,
      duration_ms:Math.round((performance.now()-started)*100)/100}));
    return new Response(response.body, {status:response.status,headers:outbound});
  } catch (err) {
    console.error(JSON.stringify({event:"crm.rust_party.error",request_id:requestId,
      source:env.GIT_SHA||"local",code:"INTERNAL_ERROR",message:String(err?.message||err)}));
    // In the isolated 127.0.0.1 CI build only, retain the actual cross-runtime
    // exception. No internal details are ever returned by the public CRM Worker.
    if(env.CRM_CI_LOCAL_ONLY==="yes" &&
      ["localhost","127.0.0.1"].includes(url.hostname)) {
      return Response.json({code:"INTERNAL_ERROR",request_id:requestId,
        local_diagnostic:String(err?.stack||err)},{status:500});
    }
    return Response.json({code:"INTERNAL_ERROR",error:"Internal CRM error",request_id:requestId},
      {status:500,headers:{"x-request-id":requestId,"cache-control":"no-store"}});
  }
}
export default {fetch:unifiedFetch};
