import { DurableObject } from "cloudflare:workers";
import { launch, type Browser, type BrowserContext, type Page } from "@cloudflare/playwright";
import { CRM_ORIGIN, browserUrl, crmCredentialsAllowed, isCrmRequest, isPermittedBrowserRequest, redact, safePagePath } from "./security";

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
};

type Result = { ok: true; data: unknown; image?: string } | { ok: false; error: string };

// One DO = one authenticated operator's browser session. The MCP transport stays stateless.
// State is ephemeral by design: expired Browser Run sessions are reopened explicitly.
export class CrmBrowserSession extends DurableObject<Env> {
  private browser: Browser | null = null;
  private context: BrowserContext | null = null;
  private page: Page | null = null;
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

  private async open(path: string = "/") {
    const destination = browserUrl(path);
    if (isCrmRequest(destination) &&
        (!this.env.CRM_BROWSER_CLIENT_ID || !this.env.CRM_BROWSER_CLIENT_SECRET)) {
      throw new Error("CRM_BROWSER_AGENT_IDENTITY_NOT_PROVISIONED");
    }
    if (this.page && !this.page.isClosed()) {
      return { state: "already_open", path: safePagePath(this.page.url()) };
    }
    // Never use the owner's real Access cookie or the CI E2E service principals.
    this.browser = await launch(this.env.BROWSER);
    try {
      this.context = await this.browser.newContext({
        acceptDownloads: true,
        serviceWorkers: "block",
      });
      // Universal public-web browsing. Never pass CRM's Access credentials to
      // the whole browser context: that leaks across origins and redirects.
      await this.context.route("**/*", async route => {
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
      this.page = await this.context.newPage();
      this.page.on("console", message => {
        this.consoleLog.push({ level: message.type(), text: redact(message.text()) });
        if (this.consoleLog.length > 80) this.consoleLog.shift();
      });
      this.page.on("pageerror", error => {
        this.consoleLog.push({ level: "pageerror", text: redact(error.message) });
        if (this.consoleLog.length > 80) this.consoleLog.shift();
      });
      this.page.on("response", response => {
        const url = response.url();
        this.requestLog.push({
          method: response.request().method(),
          path: safePagePath(url),
          status: response.status(),
        });
        if (this.requestLog.length > 80) this.requestLog.shift();
      });
      this.page.on("dialog", async dialog => {
        const accept = this.acceptingDialog;
        this.acceptingDialog = false;
        if (accept) await dialog.accept();
        else await dialog.dismiss();
      });
      this.ownerDirectedCrmNavigation = isCrmRequest(destination);
      let response;
      try {
        response = await this.page.goto(destination, {
          waitUntil: "domcontentloaded", timeout: 25000,
        });
      } finally {
        this.ownerDirectedCrmNavigation = false;
      }
      return {
        state: "open",
        http_status: response?.status() ?? null,
        path: safePagePath(this.page.url()),
        title: await this.page.title(),
      };
    } catch (error) {
      await this.close();
      throw error;
    }
  }

  private async close() {
    try {
      await this.browser?.close();
    } finally {
      this.browser = null;
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
      case "navigate": {
        const destination = browserUrl(command.url ?? "/", page.url() || CRM_ORIGIN);
        if (isCrmRequest(destination) &&
            (!this.env.CRM_BROWSER_CLIENT_ID || !this.env.CRM_BROWSER_CLIENT_SECRET)) {
          throw new Error("CRM_BROWSER_AGENT_IDENTITY_NOT_PROVISIONED");
        }
        this.ownerDirectedCrmNavigation = isCrmRequest(destination);
        try {
          await page.goto(destination, { waitUntil: "domcontentloaded", timeout: 25000 });
        } finally {
          this.ownerDirectedCrmNavigation = false;
        }
        return { data: { path: safePagePath(page.url()), title: await page.title() } };
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
    let result: Result;
    try {
      const command = await request.json() as Command;
      const { data, image } = await this.execute(command);
      result = { ok: true, data, ...(image ? { image } : {}) };
    } catch (error) {
      result = { ok: false, error: redact(error instanceof Error ? error.message : error) };
    }
    return Response.json(result);
  }
}
