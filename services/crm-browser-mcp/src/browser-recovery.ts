/** A single testable recovery decision used by the real Durable Object.
 * Only the official Cloudflare Browser Run session may be reattached.
 * Never acquire a replacement browser as a side-effect of status or restore. */
export type ExistingSession = { status?: string } | null;

export type RecoveryOutcome<Browser> =
  | { kind: "missing" }
  | { kind: "expired" }
  | { kind: "connected"; sessionId: string; browser: Browser };

export async function recoverExistingBrowserSession<Browser>(
  sessionId: string | undefined,
  getSession: (sessionId: string) => Promise<ExistingSession>,
  reconnect: (sessionId: string) => Promise<Browser>,
): Promise<RecoveryOutcome<Browser>> {
  if (!sessionId) return { kind: "missing" };
  // If lookup fails, propagate the error and preserve the stored session ID.
  // An API outage is not proof of closure and must not create a second browser.
  const session = await getSession(sessionId);
  const status = session?.status?.toLowerCase();
  if (!session || status === "closed" || status === "closing" || status === "expired") {
    return { kind: "expired" };
  }
  try {
    return { kind: "connected", sessionId, browser: await reconnect(sessionId) };
  } catch {
    // Session might still be alive. Do not erase the stored ID or acquire a
    // replacement, and never return an ambiguous success or raw credentials.
    throw new Error("BROWSER_SESSION_RECONNECT_FAILED");
  }
}
