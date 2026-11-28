// The MCP is site-neutral. An optional Cloudflare Access service identity is
// configured per exact HTTPS origin; normal human sign-in never needs it.
export const DEFAULT_BROWSER_URL = "https://example.org/";
export const MCP_HOST = "crm-browser-mcp.crm-5bd5e1e1.workers.dev";

// A human-readable owner email alone is insufficient. Verify the Access app
// audience as well: a token issued for a different Access app is not accepted.
export function isAuthorizedMcpOwner(
  accessAudience: string | undefined,
  expectedAudience: string | undefined,
  accessEmail: string | undefined,
  ownerEmail: string | undefined,
): boolean {
  return Boolean(
    accessAudience && expectedAudience && accessAudience === expectedAudience &&
    accessEmail?.trim() && ownerEmail?.trim() &&
    accessEmail.trim().toLowerCase() === ownerEmail.trim().toLowerCase(),
  );
}

// ctx.access is server-created by Cloudflare Access. Caller-supplied headers,
// email strings and MCP tool parameters are never accepted as controller proof.
// The same check works for both Managed OAuth and normal Access sessions.
export interface AccessControllerEvidence {
  aud: string;
  getIdentity(): Promise<{ email?: string } | null | undefined>;
}

export async function verifiedMcpOwnerEmail(
  access: AccessControllerEvidence | undefined,
  expectedAudience: string | undefined,
  ownerEmail: string | undefined,
): Promise<string | null> {
  if (!access || !expectedAudience || !ownerEmail ||
      access.aud !== expectedAudience) return null;
  try {
    const identity = await access.getIdentity();
    const email = identity?.email?.trim().toLowerCase();
    return isAuthorizedMcpOwner(access.aud, expectedAudience, email, ownerEmail)
      ? email!
      : null;
  } catch {
    // Expired, malformed, rejected or unavailable Access evidence is denied;
    // never turn an identity-verification error into an open MCP session.
    return null;
  }
}

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

// route.continue may carry overridden headers into a redirect chain. Every
// request outside a configured Access origin must explicitly lose both service
// credentials before Chromium sends it to the network.
export function stripSiteAccessHeaders(headers: Record<string, string>): Record<string, string> {
  return Object.fromEntries(Object.entries(headers).filter(([name]) => {
    const normalized = name.toLowerCase();
    return normalized !== "cf-access-client-id" &&
      normalized !== "cf-access-client-secret";
  }));
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

function isInternalHost(host: string): boolean {
  // A fully qualified DNS hostname may end in one or more dots. Compare its
  // canonical label spelling, not the URL's raw hostname: "localhost." and
  // "metadata." resolve to exactly the same destinations as bare names.
  const h = host.replace(/^\[|\]$/g, "").replace(/\.+$/g, "").toLowerCase();
  if (!h) return true;
  // A one-label name can resolve to a private machine through search
  // domains even though the URL itself contains no obvious private IP.
  if (!h.includes(".") && !h.includes(":") && !/^\d+$/.test(h)) return true;
  if (h === "localhost" || h.endsWith(".localhost") ||
      h.endsWith(".local") || h.endsWith(".internal") ||
      h.endsWith(".home.arpa") || h.endsWith(".lan") ||
      h.endsWith(".test") || h.endsWith(".invalid") ||
      h === "::1" || h === "0.0.0.0" || h === "::") return true;
  const parts = h.split(".");
  if (parts.length === 4 && parts.every(x => /^\d+$/.test(x) && +x <= 255)) {
    const [a = -1,b = -1,c = -1] = parts.map(Number);
    return a === 0 || a === 10 || a === 127 || a >= 224 ||
      (a === 169 && b === 254) || (a === 172 && b >= 16 && b <= 31) ||
      (a === 192 && (b === 168 || b === 0 ||
        (b === 88 && c === 99))) ||
      (a === 198 && (b === 18 || b === 19 || (b === 51 && c === 100))) ||
      (a === 203 && b === 0 && c === 113) ||
      (a === 100 && b >= 64 && b <= 127);
  }
  // IPv4-mapped IPv6, e.g. [::ffff:127.0.0.1] -> [::ffff:7f00:1].
  if (h.startsWith("::ffff:")) {
    const mapped = h.slice("::ffff:".length);
    if (/^\d+\.\d+\.\d+\.\d+$/.test(mapped)) return isInternalHost(mapped);
    const words = mapped.split(":");
    if (words.length === 2 && words.every(x => /^[0-9a-f]{1,4}$/i.test(x))) {
      const left = Number.parseInt(words[0]!, 16);
      const right = Number.parseInt(words[1]!, 16);
      return isInternalHost([left >> 8, left & 255, right >> 8, right & 255].join("."));
    }
    return true;
  }
  if (h.includes(":")) {
    // Native IPv6 literals: only global-unicast 2000::/3 is routable on
    // the ordinary public Internet. This excludes IPv4-compatible ::/96,
    // NAT64 64:ff9b::/96, loopback, unspecified, link-local, ULA, site-local,
    // multicast, and special-use ranges without enumerating their aliases.
    // Public IPv4-mapped IPv6 was handled by the branch above.
    if (!/^[23][0-9a-f]{3}:/i.test(h)) return true;
    // Tunneling/translation and documentation prefixes are not appropriate
    // destinations for this owner-operated public-web browser.
    return h.startsWith("2002:") ||
      h.startsWith("2001::") || h.startsWith("2001:0:") ||
      h.startsWith("2001:db8:");
  }
  return false;
}

// Normal public HTTP(S) sites are supported; only local/private network and
// non-web protocols are out of scope for remote Cloudflare Browser Run.
export function browserUrl(input: string, base: string = DEFAULT_BROWSER_URL): string {
  let parsed: URL;
  try { parsed = new URL(input, base); }
  catch { throw new Error("BROWSER_INVALID_URL"); }
  if ((parsed.protocol !== "https:" && parsed.protocol !== "http:") ||
      parsed.username || parsed.password || !parsed.hostname || isInternalHost(parsed.hostname)) {
    throw new Error("BROWSER_UNSAFE_URL");
  }
  return parsed.toString();
}

// Apply the same destination policy to every intercepted browser request,
// including links, redirects and subresources initiated inside untrusted pages.
export function isPermittedBrowserRequest(input: string): boolean {
  try {
    browserUrl(input, input);
    return true;
  } catch {
    return false;
  }
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

export function safePagePath(input: string): string {
  try {
    const url = new URL(input);
    if (url.protocol !== "https:" && url.protocol !== "http:") return "UNSAFE_URL";
    // Query strings, fragments and embedded credentials are never logged.
    if (url.username || url.password) return "URL_WITH_CREDENTIALS";
    return url.origin + url.pathname.slice(0, 512);
  } catch {
    return "INVALID_URL";
  }
}

export function redact(input: unknown): string {
  const raw = String(input).slice(0, 4000);
  // Mask complete credential header values (which can contain spaces).
  // A failed previous regex must never leave a Bearer token behind.
  return raw
    .replace(/\bBearer\s+\S+/gi, "Bearer [REDACTED]")
    .replace(/\b(Authorization|Cookie|Set-Cookie|CF-Access-Client-Id|CF-Access-Client-Secret)\s*[:=]\s*[\s\S]*?(?=\s+\b(?:Authorization|Cookie|Set-Cookie|CF-Access-Client-Id|CF-Access-Client-Secret)\s*[:=]|$)/gi, "$1=[REDACTED]");
}
