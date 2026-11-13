// Browser MCP security boundary: never trust model-supplied origins or headers.
export const CRM_ORIGIN = "https://crm-dev.crm-5bd5e1e1.workers.dev";
export const MCP_HOST = "crm-browser-mcp.crm-5bd5e1e1.workers.dev";

export function crmUrl(input: string = "/"): string {
  const value = new URL(input, CRM_ORIGIN + "/");
  if (value.protocol !== "https:" || value.origin !== CRM_ORIGIN ||
      value.username !== "" || value.password !== "") {
    throw new Error("CRM_BROWSER_ORIGIN_DENIED");
  }
  return value.toString();
}

export function isCrmRequest(url: string): boolean {
  try {
    return crmUrl(url) === new URL(url).toString();
  } catch {
    return false;
  }
}

export function safePagePath(url: string): string {
  try {
    if (!isCrmRequest(url)) return "BLOCKED_EXTERNAL_ORIGIN";
    // Query strings can contain secrets; never expose them in MCP or logs.
    const pathname = new URL(url).pathname;
    return pathname.slice(0, 512);
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
