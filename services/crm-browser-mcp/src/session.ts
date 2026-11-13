import { DurableObject } from "cloudflare:workers";
import { acquire, connect, type Browser, type BrowserContext, type Page } from "@cloudflare/playwright";
import { CRM_ORIGIN, browserUrl, crmCredentialsAllowed, isCrmRequest, isPermittedBrowserRequest, redact, requireCrmAgentIdentity, safePagePath } from "./security";
import { navigatePage } from "./page-navigation";
import { boundedWaitTimeout, compareBrowserAssertion, safeAssertionValue, type BrowserAssertionKind, type TextMatch, type WaitState } from "./browser-checks";

export interface Env {
  BROWSER: BrowserRun;
  SESSIONS: DurableObjectNamespace<CrmBrowserSession>;
  CRM_BROWSER_CLIENT_ID: string;
  CRM_BROWSER_CLIENT_SECRET: string;
  CRM_MCP_OWNER_EMAIL: string;
  CRM_MCP_ACCESS_AUD: string;
}

export type Target = {
  kind: "css" | "role" | "text" | "label";
  value: string;
  name?: string;
};

export type Command = {
  op: string;
  url?: string;
  target?: Target;
  text?: string;
  key?: string;
  checked?: boolean;
  x?: number;
  y?: number;
  value?: string;
  acceptDialog?: boolean;
  kind?: BrowserAssertionKind;
  expected?: string | number | boolean;
  match?: TextMatch;
  state?: WaitState;
  timeoutMs?: number;
  steps?: Command[];
};

type Result = { ok: true; data: unknown; image?: string } | { ok: false; error: string };

// One DO = one authenticated operator's browser session. The MCP transport stays stateless.
// State is ephemeral by design: expired Browser Run sessions are reopened explicitly.
export class CrmBrowserSession extends DurableObject<Env> {
  private browser: Browser | null = null;
  private context: BrowserContext | null = null;
  private page: Page | null = null;
  private browserSessionId: string | null = null;
  private nextOperation: Promise<void> = Promise.resolve();
  private pendingOperations = 0;
  private acceptingDialog = false;
  private ownerDirectedCrmNavigation = false;
  private consoleLog: Array<{ level: string; text: string }> = [];
  private requestLog: Array<{ method: string; path: string; status: number }> = [];

  private target(target: Target) {
    if (!this.page) throw new Error("CRM_BROWSER_NOT_OPEN");
    if (target.kind === "role") {
      return this.page.getByRole(target.value as Parameters<Page["getByRole"]>[0], {
        name: target.name, exact: true,
      });
    }
    if (target.kind === "label") return this.page.getByLabel(target.value, { exact: true });
    if (target.kind === "text") return this.page.getByText(target.value, { exact: true });
    return this.page.locator(target.value);
  }

  // Both browser_open(url) and browser_action(navigate) must really navigate
  // an existing session. This is the single owner of privileged CRM navigation.
  private async navigateTo(destination: string) {
    const page = this.page;
    if (!page || page.isClosed()) throw new Error("CRM_BROWSER_NOT_OPEN");
    requireCrmAgentIdentity(
      destination, this.env.CRM_BROWSER_CLIENT_ID, this.env.CRM_BROWSER_CLIENT_SECRET,
    );
    return navigatePage(
      page, destination, enabled => { this.ownerDirectedCrmNavigation = enabled; },
    );
  }

  // The only browser owner. The DO stores only an opaque Browser Run session
  // ID; cookies, tabs and browser processes remain in the official runtime.
  private async restore() {
    if (this.browser?.isConnected()) return;
    this.browser = null;
    this.context = null;
    this.page = null;
    const sessionId = await this.ctx.storage.get<string>("browser_session_id");
    if (!sessionId) return;
    const session = await this.env.BROWSER.getSession(sessionId);
    if (!session) {
      // Expired sessions are not silently replaced; only explicit open starts
      // another session. No attempt is made to recreate a login/cookie jar.
      await this.ctx.storage.delete("browser_session_id");
      this.browserSessionId = null;
      return;
    }
    let browser: Browser;
    try {
      browser = await connect(this.env.BROWSER, sessionId);
    } catch {
      // Session could still be live. Keep its ID, fail closed and let a later
      // request retry: do not launch an untracked second browser.
      throw new Error("BROWSER_SESSION_RECONNECT_FAILED");
    }
    this.browser = browser;
    this.browserSessionId = sessionId;
    const contexts = browser.contexts();
    this.context = contexts.find(c => c.pages().some(p => !p.isClosed())) ??
      contexts[0] ?? null;
    if (this.context) {
      await this.installSafetyRouting(this.context);
      this.page = this.context.pages().find(p => !p.isClosed()) ?? null;
      if (this.page) this.attachPage(this.page);
    }
  }

  private async installSafetyRouting(context: BrowserContext) {
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
          trustedMainNavigation = this.ownerDirectedCrmNavigation &&
            request.isNavigationRequest() && frame === this.page?.mainFrame();
        } catch { /* Service-worker requests have no frame; never authorize. */ }
        const hasAgentCredentials = Boolean(
          this.env.CRM_BROWSER_CLIENT_ID && this.env.CRM_BROWSER_CLIENT_SECRET,
        );
        if (!hasAgentCredentials ||
            !crmCredentialsAllowed(request.url(), initiator, trustedMainNavigation)) {
          // Includes foreign sites, their assets, third-party CDNs and SSO.
          await route.continue();
          return;
        }
        // Playwright route.continue({headers}) propagates overridden headers to
        // every redirect, including foreign origins. This is NOT safe.
        // route.fetch(maxRedirects:0) keeps the signed credentials on exactly
        // the first CRM request and fulfills the response to the browser.
        // A 3xx is followed in the browser WITHOUT privileged request headers.
        const response = await route.fetch({
          headers: {
            ...request.headers(),
            "CF-Access-Client-Id": this.env.CRM_BROWSER_CLIENT_ID,
            "CF-Access-Client-Secret": this.env.CRM_BROWSER_CLIENT_SECRET,
          },
          maxRedirects: 0,
          timeout: 25000,
        });
        await route.fulfill({ response });
      });
  }

  private attachPage(page: Page) {
      page.on("console", message => {
        this.consoleLog.push({ level: message.type(), text: redact(message.text()) });
        if (this.consoleLog.length > 80) this.consoleLog.shift();
      });
      page.on("pageerror", error => {
        this.consoleLog.push({ level: "pageerror", text: redact(error.message) });
        if (this.consoleLog.length > 80) this.consoleLog.shift();
      });
      page.on("response", response => {
        const url = response.url();
        this.requestLog.push({
          method: response.request().method(),
          path: safePagePath(url),
          status: response.status(),
        });
        if (this.requestLog.length > 80) this.requestLog.shift();
      });
      page.on("dialog", async dialog => {
        const accept = this.acceptingDialog;
        this.acceptingDialog = false;
        if (accept) await dialog.accept();
        else await dialog.dismiss();
      });
  }

  private async open(path: string = "/") {
    const destination = browserUrl(path);
    requireCrmAgentIdentity(
      destination, this.env.CRM_BROWSER_CLIENT_ID, this.env.CRM_BROWSER_CLIENT_SECRET,
    );
    await this.restore();
    if (this.page && !this.page.isClosed()) {
      if (this.page.url() === destination) {
        return { state: "already_open", path: safePagePath(this.page.url()) };
      }
      return { state: "navigated", ...(await this.navigateTo(destination)) };
    }
    let created = false;
    if (!this.browser) {
      // Official acquisition + explicit connection allows DO eviction recovery.
      // Keep-alive is idle time, not an automatic browser polling loop.
      const { sessionId } = await acquire(this.env.BROWSER, { keep_alive: 600_000 });
      try {
        this.browser = await connect(this.env.BROWSER, sessionId);
        this.browserSessionId = sessionId;
        await this.ctx.storage.put("browser_session_id", sessionId);
        created = true;
      } catch (error) {
        await this.env.BROWSER.closeSession(sessionId);
        throw error;
      }
    }
    try {
      if (!this.context) {
        this.context = await this.browser.newContext({
          acceptDownloads: true,
          serviceWorkers: "block",
        });
        await this.installSafetyRouting(this.context);
      }
      this.page = await this.context.newPage();
      this.attachPage(this.page);
      return { state: "open", ...(await this.navigateTo(destination)) };
    } catch (error) {
      if (created) await this.close();
      throw error;
    }
  }

  private async close() {
    const sessionId = this.browserSessionId ??
      await this.ctx.storage.get<string>("browser_session_id");
    try {
      // connect().close() disconnects Playwright; closeSession() terminates the
      // Browser Run session, not just this DO's CDP connection.
      await this.browser?.close();
      if (sessionId) await this.env.BROWSER.closeSession(sessionId);
      await this.ctx.storage.delete("browser_session_id");
    } finally {
      this.browser = null;
      this.browserSessionId = null;
      this.context = null;
      this.page = null;
      this.acceptingDialog = false;
      this.ownerDirectedCrmNavigation = false;
      this.requestLog = [];
      this.consoleLog = [];
    }
  }

  private async execute(command: Command): Promise<{ data: unknown; image?: string }> {
    if (command.op === "open") return { data: await this.open(command.url) };
    if (command.op === "close") {
      await this.close();
      return { data: { state: "closed" } };
    }
    if (command.op !== "close") await this.restore();
    if (command.op === "status") return {
      data: {
        state: this.page && !this.page.isClosed() ? "open" : "closed",
        path: this.page && !this.page.isClosed() ? safePagePath(this.page.url()) : null,
      },
    };
    const page = this.page;
    if (!page || page.isClosed()) throw new Error("CRM_BROWSER_NOT_OPEN");

    switch (command.op) {
      case "snapshot":
        return {
          data: {
            path: safePagePath(page.url()),
            title: await page.title(),
            accessibility: (await page.locator("body").ariaSnapshot()).slice(0, 30000),
          },
        };
      case "screenshot": {
        const png = await page.screenshot({ fullPage: true });
        return {
          data: { path: safePagePath(page.url()), mimeType: "image/png" },
          image: Buffer.from(png).toString("base64"),
        };
      }
      case "console":
        return { data: { messages: this.consoleLog.slice(-80), responses: this.requestLog.slice(-80) } };
      case "live_view": {
        if (!this.browser) throw new Error("BROWSER_SESSION_NOT_AVAILABLE");
        // The URL contains a signed bearer credential. Return it only through
        // the owner-authenticated MCP call; never log or store it.
        const view = await this.env.BROWSER.getLiveView(this.browser.sessionId(), {
          mode: "tab",
          expiresInMs: 60_000,
          guardrails: { mode: "readonly" },
        });
        return { data: { url: view.devtoolsFrontendUrl, readonly: true, expires_in_seconds: 60 } };
      }
      case "wait": {
        const timeout = boundedWaitTimeout(command.timeoutMs);
        if (command.target) {
          await this.target(command.target).waitFor({
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
          const locator = this.target(command.target);
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
        if (!steps || steps.length === 0 || steps.length > 12) {
          throw new Error("BROWSER_RUN_STEPS_OUT_OF_RANGE");
        }
        const results: Array<{ index: number; op: string; passed: boolean; data?: unknown; error?: string }> = [];
        let passed = true;
        for (const [index, step] of steps.entries()) {
          if (step.op === "run" || step.op === "open" || step.op === "close" || step.op === "session" || step.op === "live_view") {
            throw new Error("BROWSER_RUN_UNSAFE_STEP");
          }
          try {
            const response = await this.execute(step);
            const data = response.data as { passed?: boolean };
            const ok = data.passed !== false;
            results.push({ index, op: step.op, passed: ok, data });
            if (!ok) { passed = false; break; }
          } catch (error) {
            passed = false;
            results.push({
              index, op: step.op, passed: false,
              error: redact(error instanceof Error ? error.message : error),
            });
            break;
          }
        }
        return { data: { passed, steps: results, total: steps.length, executed: results.length } };
      }
      case "navigate": {
        const destination = browserUrl(command.url ?? "/", page.url() || CRM_ORIGIN);
        return { data: await this.navigateTo(destination) };
      }
      case "reload":
        await page.reload({ waitUntil: "domcontentloaded", timeout: 25000 });
        return { data: { path: safePagePath(page.url()) } };
      case "back":
        await page.goBack({ waitUntil: "domcontentloaded", timeout: 25000 });
        return { data: { path: safePagePath(page.url()) } };
      case "forward":
        await page.goForward({ waitUntil: "domcontentloaded", timeout: 25000 });
        return { data: { path: safePagePath(page.url()) } };
      case "scroll":
        await page.mouse.wheel(command.x ?? 0, command.y ?? 650);
        return { data: { action: "scroll", completed: true } };
      case "press":
        await page.keyboard.press(command.key ?? "");
        return { data: { action: "press", completed: true } };
      case "session": {
        if (!isCrmRequest(page.url())) throw new Error("CRM_BROWSER_NOT_ON_CRM");
        // Uses the CRM's own current Access identity, not an override header.
        const result = await page.evaluate(async () => {
          const response = await fetch("/api/session", { credentials: "same-origin" });
          return { status: response.status, body: await response.json() };
        });
        return { data: result };
      }
      case "click":
      case "double_click":
      case "fill":
      case "select":
      case "check":
      case "uncheck":
      case "hover":
      case "type": {
        if (!command.target) throw new Error("CRM_BROWSER_TARGET_REQUIRED");
        const locator = this.target(command.target);
        this.acceptingDialog = command.acceptDialog === true;
        try {
          if (command.op === "click") await locator.click({ timeout: 12000 });
          else if (command.op === "double_click") await locator.dblclick({ timeout: 12000 });
          else if (command.op === "fill") await locator.fill(command.text ?? "", { timeout: 12000 });
          else if (command.op === "select") await locator.selectOption(command.value ?? "", { timeout: 12000 });
          else if (command.op === "check") await locator.check({ timeout: 12000 });
          else if (command.op === "uncheck") await locator.uncheck({ timeout: 12000 });
          else if (command.op === "hover") await locator.hover({ timeout: 12000 });
          else if (command.op === "type") await locator.pressSequentially(command.text ?? "", { timeout: 12000 });
        } finally {
          this.acceptingDialog = false;
        }
        return {
          data: {
            action: command.op, completed: true, path: safePagePath(page.url()),
            // Request IDs and full data values can be inspected separately through Cloudflare.
            latest_requests: this.requestLog.slice(-8),
          },
        };
      }
      default:
        throw new Error("CRM_BROWSER_UNKNOWN_OPERATION");
    }
  }

  override async fetch(request: Request): Promise<Response> {
    if (request.method !== "POST" || new URL(request.url).pathname !== "/internal") {
      return new Response("Not found", { status: 404 });
    }
    // Commands for one owner share one browser and must not interleave across
    // await points. A bounded FIFO serializes tab changes and all UI actions.
    if (this.pendingOperations >= 8) {
      return Response.json({ ok: false, error: "BROWSER_BUSY" }, { status: 429 });
    }
    this.pendingOperations++;
    const previous = this.nextOperation;
    let release!: () => void;
    this.nextOperation = new Promise<void>(resolve => { release = resolve; });
    await previous;
    let result: Result;
    try {
      const command = await request.json() as Command;
      const { data, image } = await this.execute(command);
      result = { ok: true, data, ...(image ? { image } : {}) };
    } catch (error) {
      result = { ok: false, error: redact(error instanceof Error ? error.message : error) };
    } finally {
      this.pendingOperations--;
      release();
    }
    return Response.json(result);
  }
}
