// Browser MCP browses the public web, but CRM service credentials never leave
// their exact origin. Do not treat an arbitrary web page as a trusted CRM actor.
export const CRM_ORIGIN = "https://crm-dev.crm-5bd5e1e1.workers.dev";
export const MCP_HOST = "crm-browser-mcp.crm-5bd5e1e1.workers.dev";

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
  if (h === "localhost" || h.endsWith(".localhost") ||
      h.endsWith(".local") || h.endsWith(".internal") ||
      h === "::1" || h === "0.0.0.0" || h === "::") return true;
  const parts = h.split(".");
  if (parts.length === 4 && parts.every(x => /^\d+$/.test(x) && +x <= 255)) {
    const [a = -1,b = -1] = parts.map(Number);
    return a === 0 || a === 10 || a === 127 || a >= 224 ||
      (a === 169 && b === 254) || (a === 172 && b >= 16 && b <= 31) ||
      (a === 192 && b === 168) || (a === 100 && b >= 64 && b <= 127);
  }
  return /^(?:fc|fd|fe[89ab])[0-9a-f:]+$/i.test(h);
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

// A malicious third-party web page must not gain the CRM service identity by
// issuing cross-site fetch() or navigating a hidden iframe to the CRM.
// A user-directed main-frame navigation from the MCP is allowed.
export function crmCredentialsAllowed(
  destination: string, initiator: string, authorizedMainNavigation: boolean,
): boolean {
  return isCrmRequest(destination) &&
    (isCrmRequest(initiator) || authorizedMainNavigation);
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
