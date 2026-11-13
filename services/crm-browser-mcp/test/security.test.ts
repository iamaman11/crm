import test from "node:test";
import assert from "node:assert/strict";
import { browserUrl, crmCredentialsAllowed, CRM_ORIGIN, isCrmRequest, isPermittedBrowserRequest, isAuthorizedMcpOwner, safePagePath, redact, requireCrmAgentIdentity } from "../src/security.ts";

test("canonical CRM identification is exact; ordinary browser navigation stays universal", () => {
  assert.equal(browserUrl("/clients"), CRM_ORIGIN + "/clients");
  assert.equal(browserUrl("https://crm-dev.crm-5bd5e1e1.workers.dev/api/session"),
    CRM_ORIGIN + "/api/session");
  assert.equal(isCrmRequest(CRM_ORIGIN + "/api/session"), true);
  for (const url of [
    "http://crm-dev.crm-5bd5e1e1.workers.dev",
    "https://evil.example/",
    "https://crm-dev.crm-5bd5e1e1.workers.dev.evil.example/",
    "https://user:pass@crm-dev.crm-5bd5e1e1.workers.dev/",
    "javascript:alert(1)",
    "data:text/html,unsafe",
  ]) assert.equal(isCrmRequest(url), false, url);
});
test("network diagnostics contain only paths, never query secrets or foreign URLs", () => {
  assert.equal(safePagePath(CRM_ORIGIN + "/api/version?token=supersecret"), "/api/version");
  assert.equal(safePagePath("https://evil.example/auth?secret=supersecret"),
    "https://evil.example/auth");
});
test("sensitive header values are always masked in console output", () => {
  const input = "Authorization: Bearer ABC.123 Cookie: sess123 CF-Access-Client-Secret: testsecret";
  const output = redact(input);
  for (const secret of ["ABC.123", "sess123", "testsecret"]) {
    assert.equal(output.includes(secret), false);
  }
  assert.match(output, /REDACTED/);
});

test("universal browser allows real public HTTP(S) sites and links", () => {
  assert.equal(browserUrl("https://example.org/help?search=ui"),
    "https://example.org/help?search=ui");
  assert.equal(browserUrl("/contact", "https://example.org/docs"),
    "https://example.org/contact");
  assert.equal(browserUrl("http://example.org"), "http://example.org/");
  assert.equal(browserUrl("//example.net/test", CRM_ORIGIN),
    "https://example.net/test");
  assert.equal(isCrmRequest("https://example.org"), false);
});

test("universal web navigation rejects unsafe protocols and private endpoints", () => {
  for (const input of [
    "javascript:alert(1)", "file:///etc/passwd", "data:text/html,evil",
    "http://localhost:8000", "http://127.0.0.1", "http://10.2.3.4",
    "http://169.254.169.254/latest/meta-data", "https://example.local",
    "https://username:password@example.org/",
  ]) {
    assert.throws(() => browserUrl(input), /BROWSER_UNSAFE_URL/, input);
  }
});

test("CRM Access service headers never authorize foreign pages or third-party initiated CRM calls", () => {
  const crm = CRM_ORIGIN + "/api/session";
  const foreign = "https://example.org/form";
  assert.equal(crmCredentialsAllowed(crm, "about:blank", true), true);
  assert.equal(crmCredentialsAllowed(crm, CRM_ORIGIN + "/", false), true);
  assert.equal(crmCredentialsAllowed(crm, foreign, false), false,
    "untrusted third-party fetch must never get admin service credentials");
  assert.equal(crmCredentialsAllowed(foreign, CRM_ORIGIN, true), false,
    "a redirect or navigation to a foreign origin never receives CRM credentials");
  assert.equal(crmCredentialsAllowed(CRM_ORIGIN + ".evil.net", CRM_ORIGIN, true), false);
});

test("all browser requests, including page-initiated redirects and links, reject local/private destinations", () => {
  const unsafe = [
    "http://127.0.0.1/private",
    "http://[::1]/admin",
    "http://[::ffff:127.0.0.1]/token",
    "http://[::ffff:10.0.0.1]/token",
    "http://169.254.169.254/latest/meta-data",
    "http://192.168.0.1/",
    "http://172.16.254.1/",
    "http://10.0.0.10/",
    "http://localhost:9222/json",
    "https://internal.local/api",
    "file:///etc/passwd",
  ];
  for (const url of unsafe) assert.equal(isPermittedBrowserRequest(url), false, url);
  for (const url of [
    "https://example.org/",
    "https://example.org/oauth/callback?code=one-time",
    "https://developers.cloudflare.com/",
    CRM_ORIGIN + "/api/session",
  ]) assert.equal(isPermittedBrowserRequest(url), true, url);
});

test("only the exact CRM Browser MCP Access application owner can call control tools", () => {
  const audience = "8d0515ac0f3dc33e9a39bebe5e4530f70056e6ddf3d4cb910df80e7ba5f6b67c";
  assert.equal(isAuthorizedMcpOwner(audience, audience, "User@Example.test", "user@example.test"), true);
  for (const invalid of [
    { aud: "different-app", actual: "user@example.test", expected: "user@example.test" },
    { aud: audience, actual: "other@example.test", expected: "user@example.test" },
    { aud: audience, actual: undefined, expected: "user@example.test" },
    { aud: undefined, actual: "user@example.test", expected: "user@example.test" },
    { aud: audience, actual: "user@example.test", expected: "" },
  ]) {
    assert.equal(
      isAuthorizedMcpOwner(invalid.aud, audience, invalid.actual, invalid.expected),
      false,
      JSON.stringify(invalid),
    );
  }
});

test("CRM identity is fail-closed before provisioning; unrelated public websites remain usable", () => {
  for (const url of [CRM_ORIGIN + "/", CRM_ORIGIN + "/api/session"]) {
    for (const [id, secret] of [
      ["", ""], ["only-client", ""], ["", "only-secret"],
    ]) assert.throws(
      () => requireCrmAgentIdentity(url, id, secret),
      /CRM_BROWSER_AGENT_IDENTITY_NOT_PROVISIONED/,
    );
    assert.doesNotThrow(() => requireCrmAgentIdentity(url, "signed-client", "signed-secret"));
  }
  for (const url of ["https://example.org/", "https://www.iana.org/domains/reserved"]) {
    assert.doesNotThrow(() => requireCrmAgentIdentity(url, "", ""));
  }
});
