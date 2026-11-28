import test from "node:test";
import assert from "node:assert/strict";
import { browserUrl, siteCredentialsAllowed, siteRequestDisposition, siteAccessConfiguration, siteAccessTargets, siteAccessForRequest, siteAccessCredentialsReady, siteRequestDispositionForTargets, isTrustedSiteNavigationRequest, isConfiguredSiteRequest, isPermittedBrowserRequest, isAuthorizedMcpOwner, safePagePath, redact, verifiedMcpOwnerEmail, stripSiteAccessHeaders } from "../src/security.ts";
const CRM_ORIGIN = "https://crm-dev.crm-5bd5e1e1.workers.dev"; // Test-only configured site.

test("public redirects never receive either Cloudflare Access service header", () => {
  const headers = stripSiteAccessHeaders({
    "cf-access-client-id": "preview-client-id",
    "CF-Access-Client-Secret": "preview-client-secret",
    accept: "text/html",
    authorization: "Bearer ordinary-site-session",
  });
  assert.deepEqual(headers, {
    accept: "text/html",
    authorization: "Bearer ordinary-site-session",
  });
});

test("site identity is configured per exact origin; ordinary browser navigation is universal", () => {
  assert.equal(browserUrl("/clients", CRM_ORIGIN + "/"), CRM_ORIGIN + "/clients");
  assert.equal(browserUrl("https://crm-dev.crm-5bd5e1e1.workers.dev/api/session"),
    CRM_ORIGIN + "/api/session");
  assert.equal(isConfiguredSiteRequest(CRM_ORIGIN + "/api/session", CRM_ORIGIN), true);
  for (const url of [
    "http://crm-dev.crm-5bd5e1e1.workers.dev",
    "https://evil.example/",
    "https://crm-dev.crm-5bd5e1e1.workers.dev.evil.example/",
    "https://user:pass@crm-dev.crm-5bd5e1e1.workers.dev/",
    "javascript:alert(1)",
    "data:text/html,unsafe",
  ]) assert.equal(isConfiguredSiteRequest(url, CRM_ORIGIN), false, url);
});
test("network diagnostics contain only paths, never query secrets or foreign URLs", () => {
  assert.equal(safePagePath(CRM_ORIGIN + "/api/version?token=supersecret"), CRM_ORIGIN + "/api/version");
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
  assert.equal(isConfiguredSiteRequest("https://example.org", CRM_ORIGIN), false);
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

test("reserved and private-address aliases are rejected without blocking public HTTPS", () => {
  const blocked = [
    "http://router/",
    "http://metadata/",
    "http://secret.home.arpa/",
    "http://intranet.lan/",
    "http://a.test/",
    "http://a.invalid/",
    "http://192.0.2.1/",
    "http://198.18.0.1/",
    "http://198.19.1.2/",
    "http://198.51.100.5/",
    "http://203.0.113.9/",
    "http://192.0.0.3/",
    "http://192.88.99.1/",
  ];
  for (const url of blocked) {
    assert.equal(isPermittedBrowserRequest(url), false, url);
    assert.throws(() => browserUrl(url), /BROWSER_UNSAFE_URL/, url);
  }
  for (const url of [
    "https://example.org/",
    "https://www.iana.org/help/example-domains",
    "https://developers.cloudflare.com/browser-run/",
    "http://1.1.1.1/",
  ]) assert.equal(isPermittedBrowserRequest(url), true, url);
});

test("CRM Access service headers never authorize foreign pages or third-party initiated CRM calls", () => {
  const crm = CRM_ORIGIN + "/api/session";
  const foreign = "https://example.org/form";
  assert.equal(siteCredentialsAllowed(crm, "about:blank", true, CRM_ORIGIN), true);
  assert.equal(siteCredentialsAllowed(crm, CRM_ORIGIN + "/", false, CRM_ORIGIN), true);
  assert.equal(siteCredentialsAllowed(crm, foreign, false, CRM_ORIGIN), false,
    "untrusted third-party fetch must never get admin service credentials");
  assert.equal(siteCredentialsAllowed(foreign, CRM_ORIGIN, true, CRM_ORIGIN), false,
    "a redirect or navigation to a foreign origin never receives CRM credentials");
  assert.equal(siteCredentialsAllowed(CRM_ORIGIN + ".evil.net", CRM_ORIGIN, true, CRM_ORIGIN), false);
});

test("configured service routing blocks foreign-origin cookies while preserving UI navigation", () => {
  const crm = CRM_ORIGIN + "/crm.parties.v3.PartyService/ListParties";
  const foreign = "https://example.org/untrusted";
  assert.equal(siteRequestDisposition(crm, foreign, false, CRM_ORIGIN, true), "block",
    "foreign fetch cannot use an existing CRM Access cookie");
  assert.equal(siteRequestDisposition(CRM_ORIGIN + "/api/session", foreign, false, CRM_ORIGIN, true), "block",
    "foreign iframe or redirect must be denied even after previous CRM login");
  assert.equal(siteRequestDisposition(crm, "about:blank", false, CRM_ORIGIN, true), "block");
  assert.equal(siteRequestDisposition(crm, "about:blank", true, CRM_ORIGIN, true), "site",
    "explicit controller top-level navigation is permitted");
  assert.equal(siteRequestDisposition(crm, CRM_ORIGIN + "/contacts", false, CRM_ORIGIN, true), "site",
    "real React-generated Connect traffic from the CRM remains permitted");
  assert.equal(siteRequestDisposition(foreign, CRM_ORIGIN + "/contacts", true, CRM_ORIGIN, true), "public",
    "CRM credentials never accompany a cross-origin redirect");
  assert.equal(siteRequestDisposition(foreign, foreign, false, CRM_ORIGIN, true), "public",
    "ordinary public pages remain fully usable");
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

test("separate preview Access credentials are selected only for their exact origin", () => {
  const previewOne = "https://crm-writer-preview-1.crm-5bd5e1e1.workers.dev";
  const previewTwo = "https://crm-writer-preview-2.crm-5bd5e1e1.workers.dev";
  const targets = siteAccessTargets([
    {origin: CRM_ORIGIN, clientId: "crm-id", clientSecret: "crm-secret"},
    {origin: previewOne, clientId: "preview-one-id", clientSecret: "preview-one-secret", requiredCredentials: true},
    {origin: previewTwo, clientId: "preview-two-id", clientSecret: "preview-two-secret", requiredCredentials: true},
  ]);

  assert.equal(siteAccessForRequest(previewOne + "/api/session", targets)?.clientId, "preview-one-id");
  assert.equal(siteAccessForRequest(previewTwo + "/api/session", targets)?.clientId, "preview-two-id");
  assert.equal(siteAccessForRequest(CRM_ORIGIN + "/api/session", targets)?.clientId, "crm-id");
  assert.equal(siteAccessForRequest(previewOne + ".evil.example/api/session", targets), null);
  assert.equal(isTrustedSiteNavigationRequest(previewOne + "/clients", previewOne, true, true), true);
  assert.equal(isTrustedSiteNavigationRequest(previewTwo + "/clients", previewOne, true, true), false,
    "a redirect from preview one cannot inherit its explicit-navigation trust for preview two");
  assert.equal(isTrustedSiteNavigationRequest(previewOne + "/clients", previewOne, true, false), false);
  assert.equal(siteAccessCredentialsReady(previewOne + "/api/session", targets), true);
  assert.equal(siteRequestDispositionForTargets(previewOne + "/api/session", "about:blank", true, targets), "site");
  assert.equal(siteRequestDispositionForTargets(previewTwo + "/api/session", previewOne + "/", false, targets), "block",
    "a page from preview one cannot use preview two credentials");
  assert.equal(siteRequestDispositionForTargets("https://example.org/", previewOne + "/", false, targets), "public",
    "redirects to unrelated public sites never receive preview credentials");

  const missingPreviewCredentials = siteAccessTargets([
    {origin: previewOne, requiredCredentials: true},
  ]);
  const partialPreviewCredentials = siteAccessTargets([
    {origin: previewOne, clientId: "only-id", requiredCredentials: true},
  ]);
  assert.equal(siteAccessCredentialsReady(previewOne + "/api/session", partialPreviewCredentials), false,
    "a partial preview pair disables that preview without breaking unrelated sites");
  assert.equal(siteRequestDispositionForTargets("https://example.org/", previewOne + "/", false, partialPreviewCredentials), "public");
  assert.equal(siteAccessCredentialsReady(previewOne + "/api/session", missingPreviewCredentials), false);
  assert.equal(siteRequestDispositionForTargets(previewOne + "/api/session", "about:blank", true, missingPreviewCredentials), "block");
  assert.equal(siteRequestDispositionForTargets("https://example.org/", previewOne + "/", false, missingPreviewCredentials), "public");
  assert.throws(() => siteAccessTargets([
    {origin: previewOne, clientId: "one", clientSecret: "secret"},
    {origin: previewOne, clientId: "two", clientSecret: "secret"},
  ]), /BROWSER_SITE_ACCESS_ORIGIN_DUPLICATE/);
});

test("site Access credentials are optional, exact-origin and fail closed when partial", () => {
  assert.deepEqual(siteAccessConfiguration(undefined, undefined, undefined),
    { origin: null, serviceCredentials: false });
  assert.deepEqual(siteAccessConfiguration(CRM_ORIGIN, undefined, undefined),
    { origin: CRM_ORIGIN, serviceCredentials: false });
  assert.deepEqual(siteAccessConfiguration(CRM_ORIGIN, "id", "secret"),
    { origin: CRM_ORIGIN, serviceCredentials: true });
  assert.equal(siteRequestDisposition(CRM_ORIGIN + "/api/session", "https://example.org", false, CRM_ORIGIN, false),
    "public", "interactive human login uses standard browser navigation and cookies");
  assert.throws(() => siteAccessConfiguration(undefined, "id", "secret"),
    /BROWSER_SITE_ACCESS_ORIGIN_MISSING/);
  assert.throws(() => siteAccessConfiguration(CRM_ORIGIN, "id", undefined),
    /BROWSER_SITE_ACCESS_CREDENTIALS_INCOMPLETE/);
  assert.throws(() => siteAccessConfiguration(CRM_ORIGIN, undefined, "secret"),
    /BROWSER_SITE_ACCESS_CREDENTIALS_INCOMPLETE/);
  for (const invalid of ["https://crm-dev.crm-5bd5e1e1.workers.dev/clients",
    "https://crm-dev.crm-5bd5e1e1.workers.dev/?token=secret",
    "http://crm-dev.crm-5bd5e1e1.workers.dev", "https://127.0.0.1",
    "https://user:password@example.org", "https://evil.example.com/hidden"]) {
    assert.throws(() => siteAccessConfiguration(invalid, "id", "secret"),
      /BROWSER_SITE_ACCESS_ORIGIN_INVALID/, invalid);
  }
});

test("Managed OAuth/Access controller evidence denies missing, wrong, and broken identities", async () => {
  const aud = "exact-app-audience";
  const owner = "owner@example.test";
  const valid = { aud, getIdentity: async () => ({ email: "OWNER@example.test" }) };
  assert.equal(await verifiedMcpOwnerEmail(valid, aud, owner), owner);
  assert.equal(await verifiedMcpOwnerEmail(undefined, aud, owner), null);
  assert.equal(await verifiedMcpOwnerEmail(valid, "other-audience", owner), null);
  assert.equal(await verifiedMcpOwnerEmail(valid, aud, undefined), null);
  assert.equal(await verifiedMcpOwnerEmail(
    { aud, getIdentity: async () => ({ email: "other@example.test" }) }, aud, owner,
  ), null);
  assert.equal(await verifiedMcpOwnerEmail(
    { aud, getIdentity: async () => null }, aud, owner,
  ), null);
  assert.equal(await verifiedMcpOwnerEmail(
    { aud, getIdentity: async () => { throw new Error("expired JWT"); } }, aud, owner,
  ), null);
});


test("DNS root-dot aliases cannot bypass local, metadata, and reserved-host filters", () => {
  const unsafe = [
    "http://localhost./", "http://LOCALHOST..:8000/",
    "http://metadata./latest/meta-data", "http://router../admin",
    "https://console.local./", "https://example.internal./",
    "http://printer.lan./", "http://service.home.arpa./",
    "http://one.test./", "http://two.invalid./",
    "http://a.localhost./", "http://service.internal..",
  ];
  for (const input of unsafe) {
    assert.equal(isPermittedBrowserRequest(input), false, input);
    assert.throws(() => browserUrl(input), /BROWSER_UNSAFE_URL/, input);
  }
  // FQDN root dots are not forbidden by themselves; the browser stays
  // universal for public sites and their ordinary external redirects.
  assert.equal(browserUrl("https://example.org./help"), "https://example.org./help");
  assert.equal(isPermittedBrowserRequest("https://example.org./"), true);
});

test("public-web policy rejects special-use IPv6 and IPv4-embedded literals at every navigation boundary", () => {
  const unsafe = [
    "http://[2::1]/", // not global unicast: 0x0002 != 0x2000
    "http://[3::1]/", // not global unicast: 0x0003 != 0x3000
    "http://[200::1]/", // not global unicast: 0x0200
    "http://[::7f00:1]/admin", // IPv4-compatible 127.0.0.1
    "http://[::a9fe:a9fe]/latest/meta-data", // IPv4-compatible link-local
    "http://[::c0a8:101]/", // IPv4-compatible 192.168.1.1
    "http://[64:ff9b::7f00:1]/", // NAT64 can target loopback
    "http://[64:ff9b::808:808]/", // public NAT64 literal is unsupported
    "http://[2002:0a00:1::]/", // 6to4 tunnel with embedded private IPv4
    "http://[2001:0:1234::]/", // Teredo tunnel prefix
    "http://[fec0::1]/", // deprecated IPv6 site-local
    "http://[feff::1]/",
    "http://[fc00::1]/",
    "http://[ff02::1]/",
    "http://[::]/",
    "http://[2001:db8::1]/", // documentation, not a public endpoint
    "http://192.0.0.9/", // entire IPv4 special-use 192.0.0.0/24
  ];
  for (const address of unsafe) {
    assert.throws(() => browserUrl(address), /BROWSER_UNSAFE_URL/, address);
    assert.equal(isPermittedBrowserRequest(address), false, address);
  }
  for (const address of [
    "https://[2606:4700:4700::1111]/",
    "http://[2001:4860:4860::8888]/",
    "http://[::ffff:8.8.8.8]/", // canonical IPv4-mapped public address
  ]) {
    assert.doesNotThrow(() => browserUrl(address), address);
    assert.equal(isPermittedBrowserRequest(address), true, address);
  }
});
