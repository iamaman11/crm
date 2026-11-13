import { DurableObject } from "cloudflare:workers";
import { launch, type Browser, type BrowserContext, type Page } from "@cloudflare/playwright";
import { CRM_ORIGIN, crmUrl, isCrmRequest, redact, safePagePath } from "./security";

export interface Env {
  BROWSER: Fetcher;
  SESSIONS: DurableObjectNamespace<CrmBrowserSession>;
  CRM_BROWSER_CLIENT_ID: string;
  CRM_BROWSER_CLIENT_SECRET: string;
  CRM_MCP_OWNER_EMAIL: string;
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
    if (!this.env.CRM_BROWSER_CLIENT_ID || !this.env.CRM_BROWSER_CLIENT_SECRET) {
      throw new Error("CRM_BROWSER_AGENT_NOT_ENROLLED");
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
      // Origin-scoped access credentials: they are never registered as
      // context-wide extraHTTPHeaders, which would leak on cross-origin navigations.
      await this.context.route("**/*", async route => {
        const request = route.request();
        if (!isCrmRequest(request.url())) {
          await route.abort("blockedbyclient");
          return;
        }
        await route.continue({
          headers: {
            ...request.headers(),
            "CF-Access-Client-Id": this.env.CRM_BROWSER_CLIENT_ID,
            "CF-Access-Client-Secret": this.env.CRM_BROWSER_CLIENT_SECRET,
          },
        });
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
        if (isCrmRequest(url)) {
          this.requestLog.push({
            method: response.request().method(),
            path: safePagePath(url),
            status: response.status(),
          });
          if (this.requestLog.length > 80) this.requestLog.shift();
        }
      });
      this.page.on("dialog", async dialog => {
        const accept = this.acceptingDialog;
        this.acceptingDialog = false;
        if (accept) await dialog.accept();
        else await dialog.dismiss();
      });
      const response = await this.page.goto(crmUrl(path), {
        waitUntil: "domcontentloaded", timeout: 25000,
      });
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
      case "navigate":
        await page.goto(crmUrl(command.url), { waitUntil: "domcontentloaded", timeout: 25000 });
        return { data: { path: safePagePath(page.url()), title: await page.title() } };
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
