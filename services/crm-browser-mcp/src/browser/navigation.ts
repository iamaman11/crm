// Browser capabilities operate on the existing DO-owned page and state.
import type { BrowserContext, Page } from "@cloudflare/playwright";
import type { Command, Env, BrowserCommandResult } from "../contracts";
import type { BrowserLifecycleState } from "./lifecycle";
import { DEFAULT_BROWSER_URL, browserUrl, isPermittedBrowserRequest, safePagePath } from "./security";
import { siteAccessTargets, siteAccessForRequest, siteAccessCredentialsReady, siteRequestDispositionForTargets, isTrustedSiteNavigationRequest } from "../site-access";
import { navigatePage } from "../page-navigation";
import { operationTimeout } from "../browser-checks";

export interface NavigationState { ownerDirectedSiteNavigationOrigin: string | null }

// Both Open and navigate use one Playwright page owner. Service access, if
// configured, is an optional origin-bound transport; interactive login is normal.
export function configuredSiteAccess(env: Env) {
  return siteAccessTargets([
    {
      origin: env.SITE_ACCESS_ORIGIN,
      clientId: env.SITE_ACCESS_CLIENT_ID,
      clientSecret: env.SITE_ACCESS_CLIENT_SECRET,
    },
    {
      origin: env.SITE_ACCESS_PREVIEW_1_ORIGIN,
      clientId: env.SITE_ACCESS_PREVIEW_1_CLIENT_ID,
      clientSecret: env.SITE_ACCESS_PREVIEW_1_CLIENT_SECRET,
      requiredCredentials: true,
    },
    {
      origin: env.SITE_ACCESS_PREVIEW_2_ORIGIN,
      clientId: env.SITE_ACCESS_PREVIEW_2_CLIENT_ID,
      clientSecret: env.SITE_ACCESS_PREVIEW_2_CLIENT_SECRET,
      requiredCredentials: true,
    },
  ]);
}

export async function navigateTo(page: Page | null, state: NavigationState, env: Env, destination: string, runDeadline?: number) {
  if (!page || page.isClosed()) throw new Error("CRM_BROWSER_NOT_OPEN");
  const access = configuredSiteAccess(env);
  if (!siteAccessCredentialsReady(destination, access)) {
    throw new Error("BROWSER_SITE_ACCESS_CREDENTIALS_REQUIRED");
  }
  const target = siteAccessForRequest(destination, access);
  return navigatePage(
    page, destination, origin => { state.ownerDirectedSiteNavigationOrigin = origin; },
    operationTimeout(25_000, runDeadline),
    target?.serviceCredentials ? target.origin : null,
  );
}

export async function installSafetyRouting(context: BrowserContext, env: Env, state: NavigationState, lifecycle: BrowserLifecycleState, attachPage: (page: Page) => void) {
  const siteAccess = configuredSiteAccess(env);
  context.on("page", page => { attachPage(page); });
    await context.route("**/*", async route => {
      const request = route.request();
      if (!isPermittedBrowserRequest(request.url())) {
        await route.abort("blockedbyclient");
        return;
      }
      let initiator = "";
      let trustedMainNavigation = false;
      try {
        const frame = request.frame();
        initiator = frame.url();
        trustedMainNavigation = isTrustedSiteNavigationRequest(
          request.url(), state.ownerDirectedSiteNavigationOrigin,
          request.isNavigationRequest(), frame === lifecycle.page?.mainFrame(),
        );
      } catch { /* Service-worker requests have no frame; never authorize. */ }
      const disposition = siteRequestDispositionForTargets(
        request.url(), initiator, trustedMainNavigation, siteAccess,
      );
      if (disposition === "block") {
        // Browser-managed CRM cookies can survive earlier authenticated
        // navigation; never allow a foreign page to trigger their use.
        await route.abort("blockedbyclient");
        return;
      }
      if (disposition === "public") {
        // Foreign sites, their assets, third-party CDNs and SSO are public.
        await route.continue();
        return;
      }
      const target = siteAccessForRequest(request.url(), siteAccess);
      if (!target?.serviceCredentials || !target.clientId || !target.clientSecret) {
        await route.abort("blockedbyclient");
        return;
      }
      // Accepted native Browser Run transport for this TEST CRM: route.fetch →
      // route.fulfill loses upstream response headers/MIME in this provider,
      // so Chromium must continue the original request. Playwright may forward
      // overridden headers across redirects; this residual risk is accepted
      // only for isolated test data and MUST NOT be reused for production or
      // real customer data. A production-safe redirect-scoped native transport
      // remains a separate prerequisite (see issue #357).
      await route.continue({
        headers: {
          ...request.headers(),
          "CF-Access-Client-Id": target.clientId,
          "CF-Access-Client-Secret": target.clientSecret,
        },
      });
    });
}

export async function navigationCommand(page: Page, command: Command,
  navigate: (destination: string, runDeadline?: number) => ReturnType<typeof navigateTo>,
  runDeadline?: number): Promise<BrowserCommandResult> {
  switch (command.op) {
    case "navigate": {
      const destination = browserUrl(command.url ?? "/", page.url() || DEFAULT_BROWSER_URL);
      return { data: await navigate(destination, runDeadline) };
    }
    case "reload":
      await page.reload({ waitUntil: "domcontentloaded", timeout: operationTimeout(25_000, runDeadline) });
      return { data: { path: safePagePath(page.url()) } };
    case "back":
      await page.goBack({ waitUntil: "domcontentloaded", timeout: operationTimeout(25_000, runDeadline) });
      return { data: { path: safePagePath(page.url()) } };
    case "forward":
      await page.goForward({ waitUntil: "domcontentloaded", timeout: operationTimeout(25_000, runDeadline) });
      return { data: { path: safePagePath(page.url()) } };
    default: throw new Error("CRM_BROWSER_UNKNOWN_OPERATION");
  }
}
