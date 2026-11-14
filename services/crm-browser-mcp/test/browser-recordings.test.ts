import test from "node:test";
import assert from "node:assert/strict";
import { recordingHar, recordingTargets, sanitizeHar } from "../src/browser-recordings.ts";

const account = "5bd5e1e1e813f061afc259e91334262d";
const session = "11111111-2222-4333-8444-555555555555";
const target = "A104BEEF3499";

const recorded = {
  success: true,
  result: { events: { [target]: [{ type: 1 }], OTHER_TARGET: [] } },
};
const rawHar = { log: { version: "1.2", creator: { name: "Browser Run" }, entries: [{
  startedDateTime: "2026-11-15T00:00:00Z",
  time: 42,
  request: {
    method: "POST", url: "https://example.org/login?token=secret-token",
    headers: [
      { name: "Authorization", value: "Bearer secret-bearer" },
      { name: "Cookie", value: "session=private" },
      { name: "Content-Type", value: "application/json" },
    ],
    queryString: [{name:"password",value:"private"}],
    postData: {text:"password=private"},
  },
  response: {
    status: 200, statusText: "OK",
    headers: [
      {name:"Set-Cookie",value:"session=secret"},
      {name:"Server",value:"nginx"},
    ],
    content: { size: 141, mimeType: "text/html", text: "secret-response-content" },
  },
  timings: { blocked: 1, dns: -1, connect: 2, ssl: 1, send: 3, wait: 14, receive: 21 },
}] } };

function fakeFetch(requests: string[], override?: Response): typeof fetch {
  return (async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
    const u = String(input);
    requests.push(u);
    assert.equal(init?.method, "GET");
    assert.equal(new Headers(init?.headers).get("Authorization"), "Bearer test-read-token");
    if (override) return override;
    if (u.includes("/network?")) return new Response(JSON.stringify(rawHar), {status:200});
    return new Response(JSON.stringify(recorded), {status:200});
  }) as typeof fetch;
}

test("native Cloudflare recording target IDs are taken only from official events", async () => {
  const urls: string[] = [];
  const targets = await recordingTargets(account, "test-read-token", session, fakeFetch(urls));
  assert.deepEqual(targets, [target, "OTHER_TARGET"]);
  assert.equal(urls.length, 1);
  assert.match(urls[0]!, /browser-run\/recording\/11111111-/);
});

test("real HAR response becomes valid sanitized HAR 1.2 without credentials or payloads", async () => {
  const urls: string[] = [];
  const har = await recordingHar(account, "test-read-token", session, target, fakeFetch(urls));
  assert.equal(urls.length, 2);
  assert.match(urls[1]!, /\/network\?target=A104BEEF3499&format=har/);
  assert.equal(har.log.version, "1.2");
  assert.equal(har.log.entries.length, 1);
  assert.equal(har.log.entries[0]?.request.url, "https://example.org/login");
  assert.equal(har.log.entries[0]?.request.method, "POST");
  assert.equal(har.log.entries[0]?.response.status, 200);
  assert.equal(har.log.entries[0]?.timings.wait, 14);
  assert.deepEqual(har.log.entries[0]?.request.headers, [{name:"Content-Type",value:"application/json"}]);
  const serialized = JSON.stringify(har);
  for (const forbidden of ["secret-token","secret-bearer","session=private","password=private","secret-response-content","Set-Cookie", "postData"]) {
    assert.equal(serialized.includes(forbidden), false, "must remove "+forbidden);
  }
});

test("invalid requested CDP target is refused before the HAR endpoint is called", async () => {
  const urls: string[] = [];
  await assert.rejects(recordingHar(account, "test-read-token", session, "other",
    fakeFetch(urls)), /TARGET_NOT_FOUND/);
  assert.equal(urls.length, 1);
});

test("Cloudflare recording finalization 404 is reported as not ready (no false PASS)", async () => {
  await assert.rejects(recordingTargets(account, "test-read-token", session,
    fakeFetch([],new Response("",{status:404}))), /NOT_READY/);
});

test("unexpected HAR payload, oversized entry arrays and invalid IDs fail explicitly", async () => {
  assert.throws(()=>sanitizeHar({result:{events:{}}}), /INVALID_FORMAT/);
  assert.throws(()=>sanitizeHar({log:{version:"1.2",entries:Array(2001).fill({})}}), /TOO_MANY_ENTRIES/);
  assert.throws(()=>sanitizeHar({log:{version:"1.2",entries:[{}]}}), /INVALID_ENTRY/);
  await assert.rejects(recordingTargets("bad", "test-read-token", session), /INVALID_ID/);
  await assert.rejects(recordingTargets(account, "", session), /TOKEN_MISSING/);
});
