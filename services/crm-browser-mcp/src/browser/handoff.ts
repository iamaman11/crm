// Browser capabilities operate on the existing DO-owned page and state.
import type { BrowserContext } from "@cloudflare/playwright";
import type { Command, BrowserCommandResult } from "../contracts";
import type { BrowserLifecycleState } from "./lifecycle";
import { completedHandoff, handoffIdentifier, handoffLiveViewUrl, isProviderHandoffActive,
  validateHandoffRequest, type HumanHandoffRecord } from "../browser-handoff";

export interface HandoffState {
  cdp: Awaited<ReturnType<BrowserContext["newCDPSession"]>> | null;
}

export async function closeHandoff(state: HandoffState): Promise<void> {
  try { await state.cdp?.detach(); } catch { /* Browser already lost. */ }
  state.cdp = null;
}

async function persistHandoffCompletion(ctx: DurableObjectState, record: HumanHandoffRecord, event: unknown) {
  // The human may click Done after Close; do not resurrect a stale handoff.
  const currentId = await ctx.storage.get<string>("browser_session_id");
  const active = await ctx.storage.get<HumanHandoffRecord>("browser_handoff");
  if (currentId !== record.sessionId ||
      !active || active.handoffId !== record.handoffId ||
      active.status !== "waiting_human") return;
  await ctx.storage.put("browser_handoff", completedHandoff(active, event));
}

function listenForHandoffCompletion(
  ctx: DurableObjectState, state: HandoffState,
  cdp: Awaited<ReturnType<BrowserContext["newCDPSession"]>>,
  record: HumanHandoffRecord,
) {
  state.cdp = cdp;
  // Cloudflare.* events are supported at runtime, but Playwright's
  // arrow-method typings do not accept additional CDP event overloads.
  const listen = cdp.once as unknown as
    (event: string, callback: (value: unknown) => void) => void;
  listen.call(cdp, "Cloudflare.handoffComplete", event => {
    ctx.waitUntil(persistHandoffCompletion(ctx, record, event).finally(async () => {
      try { await cdp.detach(); } catch { /* Browser may have ended. */ }
      if (state.cdp === cdp) state.cdp = null;
    }));
  });
}

export async function structuredHandoff(command: Command, ctx: DurableObjectState, lifecycle: BrowserLifecycleState, state: HandoffState) {
  const existing = await ctx.storage.get<HumanHandoffRecord>("browser_handoff");
  const isStatus = command.handoffAction === "status";
  if (!isStatus && command.handoffAction !== "start") {
    throw new Error("BROWSER_HANDOFF_ACTION_INVALID");
  }
  if (isStatus && !existing) return { state: "none" };
  if (isStatus && existing?.status !== "waiting_human") {
    return { state: existing?.status, handoffId: existing?.handoffId };
  }
  // Validate before acquiring any CDP handles or minting an interactive
  // Live View URL; malformed handoffs cannot affect the existing session.
  const checked = isStatus ? undefined : validateHandoffRequest(
    command.handoffInstructions, command.handoffTimeoutMs,
  );
  const page = lifecycle.page;
  if (!page || page.isClosed() || !lifecycle.browserSessionId) {
    return isStatus ? { state: "needs_reopen", handoff: "unconfirmed" } :
      (() => { throw new Error("BROWSER_HANDOFF_REQUIRES_OPEN_PAGE"); })();
  }
  if (!lifecycle.context) throw new Error("BROWSER_HANDOFF_CONTEXT_MISSING");
  if (!isStatus && existing &&
      (existing.status === "waiting_human" || existing.status === "unconfirmed")) {
    throw new Error("BROWSER_HANDOFF_ALREADY_PENDING");
  }
  const cdp = state.cdp ?? await lifecycle.context.newCDPSession(page);
  const newCdp = state.cdp !== cdp;
  const send = cdp.send.bind(cdp) as unknown as
    (method: string, params?: Record<string, unknown>) => Promise<unknown>;
  try {
    if (isStatus) {
      if (existing?.sessionId !== lifecycle.browserSessionId) {
        return { state: "needs_reopen", handoff: "unconfirmed" };
      }
      if (newCdp) listenForHandoffCompletion(ctx, state, cdp, existing!);
      const provider = isProviderHandoffActive(await send("Cloudflare.getHandoffState"));
      if (provider === null) throw new Error("BROWSER_HANDOFF_STATE_UNAVAILABLE");
      const updated = await ctx.storage.get<HumanHandoffRecord>("browser_handoff");
      if (updated && updated.status !== "waiting_human") {
        return { state: updated.status, handoffId: updated.handoffId };
      }
      if (provider === false) {
        // No event observed: provider may have completed while DO was
        // evicted. Do not invent Done/Failed or silently release the lock.
        const unconfirmed = { ...existing!, status: "unconfirmed" as const };
        await ctx.storage.put("browser_handoff", unconfirmed);
        return { state: "unconfirmed", handoffId: existing!.handoffId };
      }
      const view = handoffLiveViewUrl(await send("Cloudflare.getLiveView", {
        mode: "tab", expiresInMs: 300_000,
      }));
      return { state: "waiting_human", handoffId: existing!.handoffId,
        liveViewUrl: view, expiresInSeconds: 300 };
    }

    if (!checked) throw new Error("BROWSER_HANDOFF_ACTION_INVALID");
    if (isProviderHandoffActive(await send("Cloudflare.getHandoffState")) !== false) {
      throw new Error("BROWSER_HANDOFF_PROVIDER_ALREADY_ACTIVE_OR_UNAVAILABLE");
    }
    const url = handoffLiveViewUrl(await send("Cloudflare.getLiveView", {
      mode: "tab", expiresInMs: 300_000,
    }));
    // Persist the intent BEFORE requesting a potentially irreversible
    // handoff. A lost ACK must never silently allow other browser actions.
    const startedAt = Date.now();
    const pending: HumanHandoffRecord = {
      sessionId: lifecycle.browserSessionId, handoffId: "pending",
      status: "waiting_human", startedAt,
      deadline: startedAt + checked.timeoutMs,
    };
    await ctx.storage.put("browser_handoff", pending);
    let earlyOutcome: unknown = null;
    let record: HumanHandoffRecord | null = null;
    const listen = cdp.once as unknown as
      (event: string, callback: (value: unknown) => void) => void;
    listen.call(cdp, "Cloudflare.handoffComplete", event => {
      if (!record) { earlyOutcome = event; return; }
      ctx.waitUntil(persistHandoffCompletion(ctx, record, event).finally(async () => {
        try { await cdp.detach(); } catch { /* Browser may have ended. */ }
        if (state.cdp === cdp) state.cdp = null;
      }));
    });
    const handoff = await send("Cloudflare.handoff", {
      instructions: checked.instructions, timeout: checked.timeoutMs,
    });
    record = { ...pending, handoffId: handoffIdentifier(handoff) };
    await ctx.storage.put("browser_handoff", record);
    state.cdp = cdp;
    if (earlyOutcome !== null) {
      await persistHandoffCompletion(ctx, record, earlyOutcome);
    }
    const current = await ctx.storage.get<HumanHandoffRecord>("browser_handoff");
    return { state: current?.status ?? "unconfirmed", handoffId: record.handoffId,
      liveViewUrl: url, expiresInSeconds: 300 };
  } catch (error) {
    if (newCdp && !state.cdp) {
      try { await cdp.detach(); } catch { /* Detached already. */ }
    }
    throw error;
  }
}

export async function liveView(lifecycle: BrowserLifecycleState, binding: BrowserRun): Promise<BrowserCommandResult> {
    if (!lifecycle.browser) throw new Error("BROWSER_SESSION_NOT_AVAILABLE");
    // The URL contains a signed bearer credential. Return it only through
    // the owner-authenticated MCP call; never log or store it.
    const view = await binding.getLiveView(lifecycle.browser.sessionId(), {
      mode: "tab",
      expiresInMs: 60_000,
      guardrails: { mode: "readonly" },
    });
    return { data: { url: view.devtoolsFrontendUrl, readonly: true, expires_in_seconds: 60 } };
}
