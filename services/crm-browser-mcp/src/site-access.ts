// Credentials for target websites. This identity is separate from the MCP controller.
import { isInternalHost } from "./browser/security.ts";

export function isConfiguredSiteRequest(input: string, origin: string | null): boolean {
  if (!origin) return false;
  try {
    const url = new URL(input);
    return url.origin === origin && url.username === "" && url.password === "" &&
      (url.protocol === "https:" || url.protocol === "http:");
  } catch {
    return false;
  }
}

export interface SiteAccessTarget {
  origin: string;
  clientId: string | null;
  clientSecret: string | null;
  serviceCredentials: boolean;
  requiredCredentials: boolean;
}

export interface SiteAccessInput {
  origin?: string;
  clientId?: string;
  clientSecret?: string;
  requiredCredentials?: boolean;
}

// One Browser MCP may serve several separately provisioned Access applications.
// Each credential pair is bound to one exact origin and never selected by a tool argument.
export function siteAccessTargets(entries: readonly SiteAccessInput[]): SiteAccessTarget[] {
  const targets: SiteAccessTarget[] = [];
  const origins = new Set<string>();
  for (const entry of entries) {
    const requiredCredentials = entry.requiredCredentials === true;
    const partialRequiredPair = requiredCredentials &&
      Boolean(entry.clientId) !== Boolean(entry.clientSecret);
    // A partially provisioned preview pair disables only that preview target.
    // The existing crm-dev pair retains its stricter global partial-pair error.
    const config = siteAccessConfiguration(
      entry.origin,
      partialRequiredPair ? undefined : entry.clientId,
      partialRequiredPair ? undefined : entry.clientSecret,
    );
    if (!config.origin) continue;
    if (origins.has(config.origin)) throw new Error("BROWSER_SITE_ACCESS_ORIGIN_DUPLICATE");
    origins.add(config.origin);
    targets.push({
      origin: config.origin,
      clientId: partialRequiredPair ? null : entry.clientId ?? null,
      clientSecret: partialRequiredPair ? null : entry.clientSecret ?? null,
      serviceCredentials: config.serviceCredentials,
      requiredCredentials,
    });
  }
  return targets;
}

export function siteAccessForRequest(
  input: string, targets: readonly SiteAccessTarget[],
): SiteAccessTarget | null {
  return targets.find(target => isConfiguredSiteRequest(input, target.origin)) ?? null;
}

export function isTrustedSiteNavigationRequest(
  destination: string, ownerDirectedOrigin: string | null,
  isNavigationRequest: boolean, isMainFrame: boolean,
): boolean {
  return Boolean(ownerDirectedOrigin && isNavigationRequest && isMainFrame &&
    isConfiguredSiteRequest(destination, ownerDirectedOrigin));
}

export function siteAccessCredentialsReady(
  input: string, targets: readonly SiteAccessTarget[],
): boolean {
  const target = siteAccessForRequest(input, targets);
  return !target || !target.requiredCredentials || target.serviceCredentials;
}

export function siteRequestDispositionForTargets(
  destination: string, initiator: string, authorizedMainNavigation: boolean,
  targets: readonly SiteAccessTarget[],
): "public" | "site" | "block" {
  const target = siteAccessForRequest(destination, targets);
  if (!target) return "public";
  if (!target.serviceCredentials) return target.requiredCredentials ? "block" : "public";
  return siteCredentialsAllowed(destination, initiator, authorizedMainNavigation, target.origin)
    ? "site" : "block";
}

// An origin without credentials uses ordinary browser Access login / cookies.
// A partial pair is never silently accepted. No site needs a service token to open.
export function siteAccessConfiguration(
  origin: string | undefined, clientId: string | undefined, clientSecret: string | undefined,
): { origin: string | null; serviceCredentials: boolean } {
  if (!origin) {
    if (clientId || clientSecret) throw new Error("BROWSER_SITE_ACCESS_ORIGIN_MISSING");
    return { origin: null, serviceCredentials: false };
  }
  let url: URL;
  try { url = new URL(origin); }
  catch { throw new Error("BROWSER_SITE_ACCESS_ORIGIN_INVALID"); }
  if (url.protocol !== "https:" || url.username || url.password ||
      url.href !== url.origin + "/" || isInternalHost(url.hostname)) {
    throw new Error("BROWSER_SITE_ACCESS_ORIGIN_INVALID");
  }
  if (Boolean(clientId) !== Boolean(clientSecret)) {
    throw new Error("BROWSER_SITE_ACCESS_CREDENTIALS_INCOMPLETE");
  }
  return { origin: url.origin, serviceCredentials: Boolean(clientId && clientSecret) };
}

// Credentials are injected ONLY into a configured exact origin for a trusted
// controller navigation or requests initiated within that same origin.
// Without configured service credentials the browser uses normal login flows,
// including identity-provider redirects and SameSite browser cookie policy.
export function siteCredentialsAllowed(
  destination: string, initiator: string, authorizedMainNavigation: boolean,
  siteOrigin: string,
): boolean {
  return isConfiguredSiteRequest(destination, siteOrigin) &&
    (isConfiguredSiteRequest(initiator, siteOrigin) || authorizedMainNavigation);
}

export function siteRequestDisposition(
  destination: string, initiator: string, authorizedMainNavigation: boolean,
  siteOrigin: string | null, serviceCredentials: boolean,
): "public" | "site" | "block" {
  if (!siteOrigin || !serviceCredentials || !isConfiguredSiteRequest(destination, siteOrigin)) {
    return "public";
  }
  return siteCredentialsAllowed(destination, initiator, authorizedMainNavigation, siteOrigin)
    ? "site" : "block";
}
