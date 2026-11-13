import test from "node:test";
import assert from "node:assert/strict";
import { browserUrl, crmCredentialsAllowed, crmUrl, CRM_ORIGIN, isCrmRequest, safePagePath, redact } from "../src/security.ts";

test("path navigation stays on the canonical, real CRM hostname", () => {
  assert.equal(crmUrl("/clients"), CRM_ORIGIN + "/clients");
  assert.equal(crmUrl("https://crm-dev.crm-5bd5e1e1.workers.dev/api/session"),
    CRM_ORIGIN + "/api/session");
  assert.equal(isCrmRequest(CRM_ORIGIN + "/api/session"), true);
});
test("no browser navigation or credential forwarding to a foreign origin", () => {
  const denied = [
    "http://crm-dev.crm-5bd5e1e1.workers.dev",
    "https://evil.example/",
    "https://crm-dev.crm-5bd5e1e1.workers.dev.evil.example/",
    "https://user:pass@crm-dev.crm-5bd5e1e1.workers.dev/",
    "javascript:alert(1)",
    "data:text/html,unsafe",
    "//evil.example/",
  ];
  for (const url of denied) {
    assert.throws(() => crmUrl(url), /CRM_BROWSER_ORIGIN_DENIED/, url);
    assert.equal(isCrmRequest(url), false, url);
  }
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
