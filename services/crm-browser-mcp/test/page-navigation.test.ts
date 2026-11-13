import test from "node:test";
import assert from "node:assert/strict";
import { navigatePage, type NavigablePage } from "../src/page-navigation.ts";
import { CRM_ORIGIN } from "../src/security.ts";

test("controller navigation to CRM uses the real Page, then immediately releases CRM trust", async () => {
  let actualUrl = "https://example.org/previous";
  const trust: boolean[] = [];
  let gotoCalls = 0;
  const page: NavigablePage = {
    url: () => actualUrl,
    title: async () => "Clients",
    goto: async (destination, options) => {
      gotoCalls++;
      assert.equal(destination, CRM_ORIGIN + "/clients");
      assert.deepEqual(options, { waitUntil: "domcontentloaded", timeout: 25_000 });
      assert.deepEqual(trust, [true], "CRM trust exists only during the requested navigation");
      actualUrl = destination;
      return { status: () => 200 };
    },
  };
  const result = await navigatePage(page, CRM_ORIGIN + "/clients", enabled => trust.push(enabled));
  assert.equal(gotoCalls, 1);
  assert.deepEqual(result, { http_status: 200, path: "/clients", title: "Clients" });
  assert.deepEqual(trust, [true, false]);
});

test("universal public-site navigation never elevates a foreign origin", async () => {
  let actualUrl = CRM_ORIGIN + "/";
  const trust: boolean[] = [];
  const destination = "https://www.iana.org/domains/reserved?debug=hidden";
  const page: NavigablePage = {
    url: () => actualUrl,
    title: async () => "IANA-managed Reserved Domains",
    goto: async (url) => {
      assert.deepEqual(trust, [false]);
      actualUrl = url;
      return { status: () => 200 };
    },
  };
  const result = await navigatePage(page, destination, enabled => trust.push(enabled));
  assert.deepEqual(result, {
    http_status: 200,
    path: "https://www.iana.org/domains/reserved",
    title: "IANA-managed Reserved Domains",
  });
  assert.deepEqual(trust, [false, false]);
});

test("navigation failure and redirect never leave persistent CRM trust", async () => {
  const trust: boolean[] = [];
  const blocked: NavigablePage = {
    url: () => "about:blank",
    title: async () => "Not reached",
    goto: async () => { throw new Error("navigation failed"); },
  };
  await assert.rejects(
    navigatePage(blocked, CRM_ORIGIN + "/clients", enabled => trust.push(enabled)),
    /navigation failed/,
  );
  assert.deepEqual(trust, [true, false]);

  let actualUrl = CRM_ORIGIN + "/";
  const redirected: NavigablePage = {
    url: () => actualUrl,
    title: async () => "External login",
    goto: async () => {
      actualUrl = "https://login.example.org/signin?token=sensitive";
      return { status: () => 302 };
    },
  };
  const redirectTrust: boolean[] = [];
  const result = await navigatePage(
    redirected, CRM_ORIGIN + "/auth", enabled => redirectTrust.push(enabled),
  );
  assert.deepEqual(redirectTrust, [true, false]);
  assert.equal(result.path, "https://login.example.org/signin");
});
