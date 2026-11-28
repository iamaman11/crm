// Browser capabilities operate on the existing DO-owned page and state.
import type { Page } from "@cloudflare/playwright";
import type { Command, Target, BrowserCommandResult, FailureScreenshot } from "../contracts";
import { DEFAULT_BROWSER_URL, browserUrl, redact, safePagePath } from "./security";
import { boundedWaitTimeout, boundedRunTimeout, compareBrowserAssertion, operationTimeout, safeAssertionValue, validateScenarioSteps } from "../browser-checks";
import { executeCdpCommands, validateCdpCommands } from "../browser-cdp";

export interface InteractionState {
  acceptingDialog: boolean;
  lastFailureScreenshot: FailureScreenshot | null;
}

export interface InteractionDependencies {
  state: InteractionState;
  execute(command: Command, deadline: number): Promise<BrowserCommandResult>;
  selectPopup(page: Page): Promise<void>;
  tabId(page: Page): string | undefined;
  latestRequests(): unknown[];
}

export function resolveTarget(page: Page | null, target: Target) {
  if (!page) throw new Error("CRM_BROWSER_NOT_OPEN");
  if (target.frameSelector !== undefined && target.frameIndex !== undefined) {
    throw new Error("BROWSER_FRAME_TARGET_CONFLICT");
  }
  if (target.frameSelector !== undefined &&
      (!target.frameSelector || target.frameSelector.length > 512)) {
    throw new Error("BROWSER_FRAME_SELECTOR_INVALID");
  }
  const scope = target.frameSelector !== undefined ?
    page.frameLocator(target.frameSelector) :
    target.frameIndex === undefined ? page : page.frames()[target.frameIndex];
  if (!scope) throw new Error("BROWSER_FRAME_NOT_FOUND");
  if (target.kind === "role") {
    return scope.getByRole(target.value as Parameters<Page["getByRole"]>[0], {
      name: target.name, exact: true,
    });
  }
  if (target.kind === "label") return scope.getByLabel(target.value, { exact: true });
  if (target.kind === "text") return scope.getByText(target.value, { exact: true });
  return scope.locator(target.value);
}

export function attachDialogHandling(page: Page, state: InteractionState): void {
  page.on("dialog", async dialog => {
    const accept = state.acceptingDialog;
    state.acceptingDialog = false;
    if (accept) await dialog.accept();
    else await dialog.dismiss();
  });
}

export async function interactionCommand(page: Page, command: Command,
    deps: InteractionDependencies, runDeadline?: number): Promise<BrowserCommandResult> {
  const { state } = deps;
  switch (command.op) {
    case "execute": {
      // Universal CDP on the SAME active Playwright Page. No second Browser,
      // MCP Worker, CDP connection manager or worker-side script evaluator.
      // The helper validates the whole batch before the first mutation.
      const commands = command.cdpCommands ?? [];
      // Validate the entire batch before obtaining a CDP handle; in
      // particular Cloudflare.handoff must use the typed DO handoff owner.
      validateCdpCommands(commands);
      const cdp = await page.context().newCDPSession(page);
      // Playwright's typed Protocol.CommandParameters enumerates known CDP
      // methods at compile time. This ONE boundary supports new runtime CDP
      // methods; browser-cdp.ts validates the dynamic method and JSON params.
      // No `any`, private SDK APIs or alternative browser engine.
      const dynamicSend = cdp.send.bind(cdp) as unknown as
        (method: string, params?: Record<string, unknown>) => Promise<unknown>;
      try {
        return {
          data: await executeCdpCommands(
            commands, dynamicSend, command.timeoutMs ?? 90_000, redact,
          ),
        };
      } finally {
        try { await cdp.detach(); } catch { /* Already disconnected. */ }
      }
    }
    case "viewport": {
      if (!Number.isInteger(command.width) || !Number.isInteger(command.height) ||
          (command.width ?? 0) < 320 || (command.width ?? 0) > 2560 ||
          (command.height ?? 0) < 320 || (command.height ?? 0) > 2560) {
        throw new Error("BROWSER_VIEWPORT_OUT_OF_RANGE");
      }
      await page.setViewportSize({ width: command.width!, height: command.height! });
      return {
        data: { viewport: page.viewportSize(), path: safePagePath(page.url()) },
      };
    }
    case "wait": {
      const timeout = operationTimeout(boundedWaitTimeout(command.timeoutMs), runDeadline);
      if (command.target) {
        await resolveTarget(page, command.target).waitFor({
          state: command.state ?? "visible",
          timeout,
        });
      } else if (command.url) {
        // Playwright owns the wait; no arbitrary sleeps or polling loops.
        await page.waitForURL(command.url, { timeout });
      } else {
        await page.waitForLoadState("domcontentloaded", { timeout });
      }
      return {
        data: {
          completed: true,
          condition: command.target ? "element" : command.url ? "url" : "domcontentloaded",
          state: command.state ?? (command.target ? "visible" : "ready"),
          path: safePagePath(page.url()),
        },
      };
    }
    case "assert": {
      if (!command.kind || command.expected === undefined) {
        throw new Error("BROWSER_ASSERT_MISSING_CONDITION");
      }
      let actual: string | number | boolean;
      if (command.kind === "url") actual = safePagePath(page.url());
      else if (command.kind === "title") actual = await page.title();
      else {
        if (!command.target) throw new Error("BROWSER_ASSERT_TARGET_REQUIRED");
        const locator = resolveTarget(page, command.target);
        if (command.kind === "visible") actual = await locator.isVisible();
        else if (command.kind === "hidden") actual = !(await locator.isVisible());
        else if (command.kind === "count") actual = await locator.count();
        else if (command.kind === "text") actual = (await locator.textContent()) ?? "";
        else throw new Error("BROWSER_ASSERT_UNKNOWN_KIND");
      }
      const result = compareBrowserAssertion({
        kind: command.kind, actual, expected: command.expected, match: command.match,
      });
      return {
        data: {
          ...result,
          expected: safeAssertionValue(result.expected),
          actual: safeAssertionValue(result.actual),
          path: safePagePath(page.url()),
        },
      };
    }
    case "run": {
      const steps = command.steps;
      validateScenarioSteps(steps, url => {
        browserUrl(url, page.url() || DEFAULT_BROWSER_URL);
      });
      state.lastFailureScreenshot = null;
      const startedAt = Date.now();
      const deadline = startedAt + boundedRunTimeout(command.timeoutMs);
      const results: Array<{ index: number; op: string; passed: boolean; elapsedMs: number; data?: unknown; error?: string; outcome?: string }> = [];
      let passed = true;
      for (const [index, step] of steps!.entries()) {
        const stepStartedAt = Date.now();
        try {
          operationTimeout(1, deadline);
          const response = await deps.execute(step, deadline);
          // A late provider ACK must never count as PASS.
          operationTimeout(1, deadline);
          const data = response.data as { passed?: boolean };
          const ok = data.passed !== false;
          // Request diagnostics live in browser_diagnostics. Never repeat
          // up to eight HTTP receipts for every scenario step or error.
          const receipt = data && typeof data === "object" ?
            Object.fromEntries(Object.entries(data).filter(([key]) => key !== "latest_requests")) : data;
          results.push({ index, op: step.op, passed: ok, elapsedMs: Date.now() - stepStartedAt, data: receipt });
          if (!ok) { passed = false; break; }
        } catch (error) {
          passed = false;
          results.push({
            index, op: step.op, passed: false, elapsedMs: Date.now() - stepStartedAt,
            error: redact(error instanceof Error ? error.message : error),
            outcome: step.op === "assert" || step.op === "wait" ? "not_completed" : "uncertain",
          });
          break;
        }
      }
      const report = { passed, steps: results, total: steps!.length, executed: results.length, elapsedMs: Date.now() - startedAt };
      if (!passed && command.screenshotOnFailure) {
        try {
          const png = await page.screenshot({ fullPage: true, timeout: 5_000 });
          if (png.byteLength <= 2_000_000) {
            state.lastFailureScreenshot = {
              image: Buffer.from(png).toString("base64"),
              capturedPath: safePagePath(page.url()),
            };
          }
        } catch { /* The failure report remains authoritative. */ }
        return { data: { ...report, failureScreenshot: state.lastFailureScreenshot ?
          "available_via_browser_screenshot" : "unavailable" } };
      }
      return { data: report };
    }
    case "scroll":
      await page.mouse.wheel(command.x ?? 0, command.y ?? 650);
      return { data: { action: "scroll", completed: true } };
    case "press":
      // A target-scoped press focuses the actual element with native
      // Playwright. Without a target, retain the ordinary page keyboard.
      if (command.target) {
        await resolveTarget(page, command.target).press(command.key ?? "", {
          timeout: operationTimeout(12_000, runDeadline),
        });
      } else {
        await page.keyboard.press(command.key ?? "");
      }
      return { data: { action: "press", completed: true, targeted: !!command.target } };
    case "click":
    case "double_click":
    case "fill":
    case "select":
    case "check":
    case "uncheck":
    case "hover":
    case "drag_to":
    case "type": {
      if (!command.target) throw new Error("CRM_BROWSER_TARGET_REQUIRED");
      if (command.followPopup &&
          command.op !== "click" && command.op !== "double_click") {
        throw new Error("BROWSER_POPUP_REQUIRES_CLICK");
      }
      const locator = resolveTarget(page, command.target);
      let followedPopup: Page | null = null;
      state.acceptingDialog = command.acceptDialog === true;
      try {
        if (command.followPopup) {
          // Listen before clicking so a fast window.open is never missed.
          // The native Playwright event identifies exactly the new page.
          const [popup] = await Promise.all([
            page.waitForEvent("popup", { timeout: operationTimeout(12_000, runDeadline) }),
            command.op === "click"
              ? locator.click({ timeout: operationTimeout(12_000, runDeadline) })
              : locator.dblclick({ timeout: operationTimeout(12_000, runDeadline) }),
          ]);
          await deps.selectPopup(popup);
          followedPopup = popup;
        } else if (command.op === "click") await locator.click({ timeout: operationTimeout(12_000, runDeadline) });
        else if (command.op === "double_click") await locator.dblclick({ timeout: operationTimeout(12_000, runDeadline) });
        else if (command.op === "fill") await locator.fill(command.text ?? "", { timeout: operationTimeout(12_000, runDeadline) });
        else if (command.op === "select") await locator.selectOption(command.value ?? "", { timeout: operationTimeout(12_000, runDeadline) });
        else if (command.op === "check") await locator.check({ timeout: operationTimeout(12_000, runDeadline) });
        else if (command.op === "uncheck") await locator.uncheck({ timeout: operationTimeout(12_000, runDeadline) });
        else if (command.op === "hover") await locator.hover({ timeout: operationTimeout(12_000, runDeadline) });
        else if (command.op === "drag_to") {
          if (!command.destination) throw new Error("BROWSER_DRAG_DESTINATION_REQUIRED");
          // Official Playwright performs a real HTML5 drag gesture. The
          // page owns drag/drop events; do not synthesize DOM events or CDP.
          await locator.dragTo(resolveTarget(page, command.destination), {
            timeout: operationTimeout(12_000, runDeadline),
          });
        }
        else if (command.op === "type") await locator.pressSequentially(command.text ?? "", { timeout: operationTimeout(12_000, runDeadline) });
      } finally {
        state.acceptingDialog = false;
      }
      return {
        data: {
          action: command.op, completed: true,
          path: safePagePath(followedPopup?.url() ?? page.url()),
          ...(followedPopup ? { followedPopup: true, tabId: deps.tabId(followedPopup) } : {}),
          // Request IDs and full data values can be inspected separately through Cloudflare.
          latest_requests: deps.latestRequests(),
        },
      };
    }
    default: throw new Error("CRM_BROWSER_UNKNOWN_OPERATION");
  }
}
