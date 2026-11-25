import assert from "node:assert/strict";
import { isRecord, records } from "./json-boundary.mts";

type ObservedRequest = {
  request_id: string;
  path: string;
  status: number;
  error_code?: string | null;
};

type TelemetryReadback = {
  requests: ReadonlyMap<string, ObservedRequest>;
  mutationIds: ReadonlySet<string>;
  probes: ReadonlyArray<{value: string}>;
  liveVersion: string;
  readSnapshot: (deadline: number) => Promise<unknown>;
  pause?: (ms: number) => Promise<unknown>;
  log?: (entry: Record<string, string | number>) => void;
  now?: () => number;
  deadlineMs?: number;
};

export async function waitForNativeTelemetry({
  requests,mutationIds,probes,liveVersion,readSnapshot,
  pause=ms=>new Promise(resolve=>setTimeout(resolve,ms)),
  log=entry=>console.log(JSON.stringify(entry)),now=Date.now,deadlineMs=120000
}: TelemetryReadback) {
  const pending=new Map(requests),requestTraces=new Map<string, string>(),dbTraces=new Set<string>();
  const started=now(),deadline=started+deadlineMs;
  let round=0,missing=[...mutationIds];
  log({event:"crm.telemetry-readback-scope",requests:pending.size,
    page_limit:2000,deadline_ms:deadlineMs});
  while((pending.size||missing.length)&&now()<deadline){
    const events=records(await readSnapshot(deadline));
    const serialized=JSON.stringify(events);
    for(const probe of probes) {
      assert.ok(!serialized.includes(probe.value),"Synthetic value leaked to native Cloudflare telemetry");
    }
    for(const event of events) {
      const metadata = isRecord(event.$metadata) ? event.$metadata : undefined;
      assert.ok(metadata?.type !== "cf-worker-event",
        "Unsafe automatic invocation log in CRM telemetry");
      const source = isRecord(event.source) ? event.source : undefined;
      const spanTrace = source?.traceId ?? metadata?.traceId;
      if (typeof spanTrace === "string" && metadata?.type === "span" && source?.db) {
        dbTraces.add(spanTrace);
      }
      if (typeof source?.request_id !== "string" || source.event !== "crm.native.request") continue;
      const entry = pending.get(source.request_id);
      if (!entry || source.operation !== entry.path || source.status !== entry.status ||
          (source.error_code ?? null) !== (entry.error_code ?? null)) continue;
      const workers = isRecord(event.$workers) ? event.$workers : undefined;
      const version = isRecord(workers?.scriptVersion) ? workers.scriptVersion : undefined;
      if (version?.id !== liveVersion) continue;

      const traceId = metadata?.traceId;
      assert.ok(typeof traceId === "string" && /^[a-f0-9]{32}$/i.test(traceId),
        "Missing or invalid Cloudflare trace ID for a matching CRM request");
      requestTraces.set(entry.request_id, traceId);
      pending.delete(entry.request_id);
    }
    missing = missing.filter(id => {
      const traceId = requestTraces.get(id);
      return traceId === undefined || !dbTraces.has(traceId);
    });
    log({event:"crm.telemetry-readback-progress",round:++round,
      accepted:requestTraces.size,still_missing:pending.size,
      d1_mutation_spans:mutationIds.size-missing.length,still_missing_d1:missing.length,
      elapsed_ms:now()-started});
    if((pending.size||missing.length)&&now()<deadline)
      await pause(Math.min(2000,deadline-now()));
  }
  assert.equal(pending.size,0,"Missing native Cloudflare traces: "+
    [...pending.values()].map(item=>item.path).join(","));
  assert.equal(missing.length,0,"Published mutations missing exact Cloudflare D1 spans");
  return {verifiedD1:mutationIds.size};
}
