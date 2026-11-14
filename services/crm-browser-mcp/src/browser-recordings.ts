/** Read finalized Browser Run recordings via the official Cloudflare REST API.
 * The Browser binding cannot read recordings. This module owns no browser,
 * session, account identity or HTTP listener: the existing DO remains owner.
 */
export const MAX_RECORDING_RESPONSE_BYTES = 8_000_000;

type Fetcher = typeof fetch;
type ObjectMap = Record<string, unknown>;
const RECORDING_ID = /^[0-9a-fA-F-]{16,128}$/;
const ACCOUNT_ID = /^[a-f0-9]{32}$/;

function obj(value: unknown): ObjectMap | null {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as ObjectMap : null;
}

async function readBoundedJson(response: Response): Promise<unknown> {
  if (response.status === 404) throw new Error("BROWSER_RECORDING_NOT_READY");
  if (!response.ok) throw new Error("BROWSER_RECORDING_PROVIDER_UNAVAILABLE");
  if (Number(response.headers.get("content-length") ?? 0) > MAX_RECORDING_RESPONSE_BYTES) {
    throw new Error("BROWSER_RECORDING_RESPONSE_TOO_LARGE");
  }
  if (!response.body) throw new Error("BROWSER_RECORDING_EMPTY_RESPONSE");
  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let total = 0;
  try {
    for (;;) {
      const { value, done } = await reader.read();
      if (done) break;
      total += value.byteLength;
      if (total > MAX_RECORDING_RESPONSE_BYTES) {
        throw new Error("BROWSER_RECORDING_RESPONSE_TOO_LARGE");
      }
      chunks.push(value);
    }
  } finally { await reader.cancel().catch(() => {}); }
  if (!total) throw new Error("BROWSER_RECORDING_EMPTY_RESPONSE");
  const data = new Uint8Array(total);
  let at = 0;
  for (const part of chunks) { data.set(part, at); at += part.byteLength; }
  try { return JSON.parse(new TextDecoder().decode(data)); }
  catch { throw new Error("BROWSER_RECORDING_INVALID_JSON"); }
}

function guard(accountId: string, token: string, sessionId: string): void {
  if (!ACCOUNT_ID.test(accountId) || !RECORDING_ID.test(sessionId)) {
    throw new Error("BROWSER_RECORDING_INVALID_ID");
  }
  if (!token) throw new Error("BROWSER_RECORDING_READ_TOKEN_MISSING");
}

function url(accountId: string, sessionId: string, targetId?: string): string {
  const root = "https://api.cloudflare.com/client/v4/accounts/" +
    accountId + "/browser-run/recording/" + encodeURIComponent(sessionId);
  if (targetId === undefined) return root;
  const params = new URLSearchParams({ target: targetId, format: "har" });
  return root + "/network?" + params.toString();
}

async function recordingFetch(
  accountId: string, token: string, sessionId: string,
  targetId?: string, fetcher: Fetcher = fetch,
): Promise<unknown> {
  guard(accountId, token, sessionId);
  const response = await fetcher(url(accountId, sessionId, targetId), {
    method: "GET",
    headers: { "Authorization": "Bearer " + token, "Accept": "application/json" },
    signal: AbortSignal.timeout(25_000),
  });
  return readBoundedJson(response);
}

export async function recordingTargets(
  accountId: string, token: string, sessionId: string, fetcher: Fetcher = fetch,
): Promise<string[]> {
  const response = await recordingFetch(accountId, token, sessionId, undefined, fetcher);
  const body = obj(response);
  if (!body || body.success !== true) throw new Error("BROWSER_RECORDING_PROVIDER_INVALID_RESPONSE");
  const events = obj(obj(body.result)?.events);
  if (!events) throw new Error("BROWSER_RECORDING_TARGETS_MISSING");
  // A target ID is an opaque identifier; never derive it from a page URL.
  return Object.keys(events).slice(0, 100);
}

function finite(value: unknown, fallback: number = -1): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function pathOnly(value: unknown): string {
  if (typeof value !== "string") return "about:blank";
  try {
    const parsed = new URL(value);
    if (parsed.protocol !== "https:" && parsed.protocol !== "http:") return "about:blank";
    // Never export token/query credentials from a HAR, even to an owner-visible
    // ChatGPT transcript. Full raw recording remains available in Cloudflare.
    return parsed.origin + parsed.pathname;
  } catch { return "about:blank"; }
}

function allowedHeaders(value: unknown): Array<{ name: string; value: string }> {
  if (!Array.isArray(value)) return [];
  const whitelist = new Set(["content-type", "content-length", "cache-control", "accept", "server"]);
  return value.map(v => obj(v)).filter((v): v is ObjectMap => v !== null).filter(v =>
    typeof v.name === "string" && whitelist.has(v.name.toLowerCase()) &&
    typeof v.value === "string",
  ).slice(0, 30).map(v => ({ name: String(v.name).slice(0, 64), value: String(v.value).slice(0, 512) }));
}

export type SafeHar = {
  log: {
    version: "1.2"; creator: { name: string; version: string };
    pages: unknown[];
    entries: Array<{
      startedDateTime: string; time: number;
      request: { method: string; url: string; httpVersion: string; cookies: []; headers: Array<{ name: string; value: string }>; queryString: []; headersSize: number; bodySize: number };
      response: { status: number; statusText: string; httpVersion: string; cookies: []; headers: Array<{ name: string; value: string }>; content: { size: number; mimeType: string }; redirectURL: ""; headersSize: number; bodySize: number };
      cache: {}; timings: Record<string, number>;
    }>;
  };
};

export function sanitizeHar(payload: unknown): SafeHar {
  const value = obj(payload);
  const log = obj(value?.log) ?? obj(obj(value?.result)?.log);
  if (!log || log.version !== "1.2" || !Array.isArray(log.entries)) {
    throw new Error("BROWSER_HAR_INVALID_FORMAT");
  }
  if (log.entries.length > 2000) throw new Error("BROWSER_HAR_TOO_MANY_ENTRIES");
  const entries = log.entries.map((entry: unknown) => {
    const base = obj(entry), req = obj(base?.request), resp = obj(base?.response);
    const content = obj(resp?.content), timings = obj(base?.timings);
    if (!base || !req || !resp) throw new Error("BROWSER_HAR_INVALID_ENTRY");
    const safeTimings: Record<string, number> = {};
    for (const key of ["blocked", "dns", "connect", "ssl", "send", "wait", "receive"]) {
      safeTimings[key] = finite(timings?.[key]);
    }
    return {
      startedDateTime: typeof base.startedDateTime === "string"
        ? base.startedDateTime.slice(0, 40) : "",
      time: finite(base.time, 0),
      request: {
        method: typeof req.method === "string" ? req.method.slice(0, 12) : "GET",
        url: pathOnly(req.url), httpVersion: "HTTP/1.1",
        cookies: [] as [], headers: allowedHeaders(req.headers), queryString: [] as [],
        headersSize: -1, bodySize: -1,
      },
      response: {
        status: finite(resp.status, 0), statusText: typeof resp.statusText === "string"
          ? resp.statusText.slice(0, 128) : "", httpVersion: "HTTP/1.1",
        cookies: [] as [], headers: allowedHeaders(resp.headers),
        content: { size: finite(content?.size, 0),
          mimeType: typeof content?.mimeType === "string"
            ? content.mimeType.slice(0, 100) : "" },
        redirectURL: "" as const, headersSize: -1, bodySize: finite(resp.bodySize),
      },
      cache: {} as {}, timings: safeTimings,
    };
  });
  return { log: {
    version: "1.2", creator: { name: "crm-browser-mcp (sanitized Cloudflare Browser Run)", version: "1" },
    pages: [], entries,
  } };
}

export async function recordingHar(
  accountId: string, token: string, sessionId: string, targetId: string,
  fetcher: Fetcher = fetch,
): Promise<SafeHar> {
  const targets = await recordingTargets(accountId, token, sessionId, fetcher);
  if (!targets.includes(targetId)) throw new Error("BROWSER_HAR_TARGET_NOT_FOUND");
  return sanitizeHar(await recordingFetch(accountId, token, sessionId, targetId, fetcher));
}
