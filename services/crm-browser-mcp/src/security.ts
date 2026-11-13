// Browser MCP browses the public web, but CRM service credentials never leave
// their exact origin. Do not treat an arbitrary web page as a trusted CRM actor.
export const CRM_ORIGIN = "https://crm-dev.crm-5bd5e1e1.workers.dev";
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

export function isCrmRequest(input: string): boolean {
  try {
    const url = new URL(input);
    return url.origin === CRM_ORIGIN && url.username === "" && url.password === "";
  } catch {
    return false;
  }
}

function isInternalHost(host: string): boolean {
  const h = host.replace(/^\[|\]$/g, "").toLowerCase();
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
      (a === 192 && (b === 168 || (b === 0 && c === 0) ||
        (b === 0 && c === 2) || (b === 88 && c === 99))) ||
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
  return /^(?:fc|fd|fe[89ab]|ff)[0-9a-f:]+$/i.test(h);
}

// Normal public HTTP(S) sites are supported; only local/private network and
// non-web protocols are out of scope for remote Cloudflare Browser Run.
export function browserUrl(input: string, base: string = CRM_ORIGIN + "/"): string {
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

// A malicious third-party web page must not gain the CRM service identity by
// issuing cross-site fetch() or navigating a hidden iframe to the CRM.
// A user-directed main-frame navigation from the MCP is allowed.
export function crmCredentialsAllowed(
  destination: string, initiator: string, authorizedMainNavigation: boolean,
): boolean {
  return isCrmRequest(destination) &&
    (isCrmRequest(initiator) || authorizedMainNavigation);
}

// A public website can open without CRM credentials. CRM navigation cannot
// silently create a new Access service principal in a different test tenant.
export function requireCrmAgentIdentity(
  destination: string, clientId: string | undefined, clientSecret: string | undefined,
): void {
  if (isCrmRequest(destination) && (!clientId || !clientSecret)) {
    throw new Error("CRM_BROWSER_AGENT_IDENTITY_NOT_PROVISIONED");
  }
}

export function safePagePath(input: string): string {
  try {
    const url = new URL(input);
    if (url.protocol !== "https:" && url.protocol !== "http:") return "UNSAFE_URL";
    // Query strings, fragments and embedded credentials are never logged.
    if (url.username || url.password) return "URL_WITH_CREDENTIALS";
    return (isCrmRequest(input) ? "" : url.origin) + url.pathname.slice(0, 512);
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
