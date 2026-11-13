import test from "node:test";
import assert from "node:assert/strict";
import { crmUrl, CRM_ORIGIN, isCrmRequest, safePagePath, redact } from "../src/security.ts";

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
    "BLOCKED_EXTERNAL_ORIGIN");
});
test("sensitive header values are always masked in console output", () => {
  const input = "Authorization: Bearer ABC.123 Cookie: sess123 CF-Access-Client-Secret: testsecret";
  const output = redact(input);
  for (const secret of ["ABC.123", "sess123", "testsecret"]) {
    assert.equal(output.includes(secret), false);
  }
  assert.match(output, /REDACTED/);
});
