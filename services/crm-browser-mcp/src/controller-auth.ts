// Human identity allowed to control this MCP endpoint; never target-site credentials.
// The controller must be a human identity authenticated by this exact Access app.
// A comma/newline-separated Worker secret allows multiple independent ChatGPT accounts.
const CONTROLLER_EMAIL = /^[^\s@,;]+@[^\s@,;]+\.[^\s@,;]+$/;

export function parseAllowedMcpControllerEmails(value: string | undefined): string[] {
  if (!value?.trim()) return [];
  const entries = value.split(/[\r\n,]/).map(email => email.trim().toLowerCase()).filter(Boolean);
  // A malformed entry invalidates the entire allowlist rather than silently
  // creating an unexpected partial authorization configuration.
  if (!entries.length || entries.some(email => !CONTROLLER_EMAIL.test(email))) return [];
  return [...new Set(entries)];
}

export function isAuthorizedMcpController(
  accessAudience: string | undefined,
  expectedAudience: string | undefined,
  accessEmail: string | undefined,
  allowedEmails: string | undefined,
): boolean {
  const email = accessEmail?.trim().toLowerCase();
  const allowlist = parseAllowedMcpControllerEmails(allowedEmails);
  return Boolean(
    accessAudience && expectedAudience && accessAudience === expectedAudience &&
    email && allowlist.includes(email),
  );
}

// ctx.access is server-created by Cloudflare Access. Caller-supplied headers,
// email strings and MCP tool parameters are never accepted as controller proof.
// The same check works for both Managed OAuth and normal Access sessions.
export interface AccessControllerEvidence {
  aud: string;
  getIdentity(): Promise<{ email?: string } | null | undefined>;
}

export async function verifiedMcpControllerEmail(
  access: AccessControllerEvidence | undefined,
  expectedAudience: string | undefined,
  allowedEmails: string | undefined,
): Promise<string | null> {
  if (!access || !expectedAudience || !allowedEmails || access.aud !== expectedAudience) return null;
  try {
    const email = (await access.getIdentity())?.email?.trim().toLowerCase();
    return isAuthorizedMcpController(access.aud, expectedAudience, email, allowedEmails)
      ? email ?? null
      : null;
  } catch {
    // Expired, malformed, rejected or unavailable Access evidence is denied;
    // never turn an identity-verification error into an open MCP session.
    return null;
  }
}

