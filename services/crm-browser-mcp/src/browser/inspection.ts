// Browser capabilities operate on the existing DO-owned page and state.
import type { Page } from "@cloudflare/playwright";
import type { Command, BrowserCommandResult, FailureScreenshot } from "../contracts";
import { redact, safePagePath } from "./security";
import { readVitals, type WebVitals } from "../browser-performance";
import { AXE_SOURCE } from "../axe-source.generated";
import { MAX_TRANSFER_BYTES } from "../browser-files";

export interface InspectionState {
  consoleLog: Array<{ level: string; text: string }>;
  requestStartedAt: WeakMap<object, number>;
  requestLog: Array<{
    method: string; path: string; status: number;
    resourceType?: string; durationMs?: number | null; failure?: string;
  }>;
}

export function attachInspection(page: Page, state: InspectionState): void {
  page.on("console", message => {
    state.consoleLog.push({ level: message.type(), text: redact(message.text()) });
    if (state.consoleLog.length > 80) state.consoleLog.shift();
  });
  page.on("pageerror", error => {
    state.consoleLog.push({ level: "pageerror", text: redact(error.message) });
    if (state.consoleLog.length > 80) state.consoleLog.shift();
  });
  page.on("request", request => {
    state.requestStartedAt.set(request, Date.now());
  });
  page.on("response", response => {
    const url = response.url();
    state.requestLog.push({
      method: response.request().method(),
      path: safePagePath(url),
      status: response.status(),
      resourceType: response.request().resourceType(),
      durationMs: (() => {
        const started = state.requestStartedAt.get(response.request());
        return started === undefined ? null : Math.max(0, Date.now() - started);
      })(),
    });
    if (state.requestLog.length > 80) state.requestLog.shift();
  });
  page.on("requestfailed", request => {
    state.requestLog.push({
      method: request.method(), path: safePagePath(request.url()), status: 0,
      resourceType: request.resourceType(),
      failure: redact(request.failure()?.errorText ?? "request_failed"),
    });
    if (state.requestLog.length > 80) state.requestLog.shift();
  });
}

export async function inspectionCommand(page: Page, command: Command, state: InspectionState,
    lastFailureScreenshot: FailureScreenshot | null): Promise<BrowserCommandResult> {
  switch (command.op) {
    case "inspect": {
      if (command.inspection === "network") {
        return {
          data: {
            path: safePagePath(page.url()), events: state.requestLog.slice(-80),
            timing_note: "Elapsed request-to-response-event ms (approximate); not a browser HAR waterfall.",
          },
        };
      }
      if (command.inspection === "performance") {
        const metrics = await page.evaluate(() => {
          const n = performance.getEntriesByType("navigation")[0] as PerformanceNavigationTiming | undefined;
          const paints = performance.getEntriesByType("paint")
            .map(item => ({ name: item.name, startTimeMs: Math.round(item.startTime) }));
          return {
            navigationType: n?.type ?? null,
            timeToFirstByteMs: n ? Math.round(n.responseStart - n.requestStart) : null,
            domContentLoadedMs: n ? Math.round(n.domContentLoadedEventEnd - n.startTime) : null,
            loadEventMs: n && n.loadEventEnd ? Math.round(n.loadEventEnd - n.startTime) : null,
            transferSizeBytes: n?.transferSize ?? null,
            resourceEntries: performance.getEntriesByType("resource").length,
            paintEntries: paints,
          };
        });
        return {
          data: {
            path: safePagePath(page.url()),metrics,
            webVitals: readVitals(await page.evaluate(() =>
              (window as Window & {__mcpVitals?:WebVitals}).__mcpVitals)),
            note:"Per-page provisional PerformanceObserver values. LCP/CLS and INP candidate are not finalized CrUX field metrics; unsupported/unobserved values are null.",
          },
        };
      }
      if (command.inspection === "accessibility") {
        const audit = await page.evaluate(() => {
          const problems: Array<{ rule: string; selector: string }> = [];
          const describe = (element: Element) =>
            element.tagName.toLowerCase() + (element.id ? "#" + element.id : "");
          for (const image of document.querySelectorAll("img:not([alt])")) {
            problems.push({ rule: "img_missing_alt", selector: describe(image) });
            if (problems.length >= 100) break;
          }
          for (const field of document.querySelectorAll("input:not([type=hidden]),select,textarea")) {
            if (problems.length >= 100) break;
            const id = field.getAttribute("id");
            const hasLabel = field.hasAttribute("aria-label") ||
              field.hasAttribute("aria-labelledby") ||
              Boolean(id && document.querySelector("label[for='" + CSS.escape(id) + "']")) ||
              Boolean(field.closest("label"));
            if (!hasLabel) problems.push({ rule: "form_field_missing_label", selector: describe(field) });
          }
          for (const button of document.querySelectorAll("button")) {
            if (problems.length >= 100) break;
            if (!button.textContent?.trim() &&
                !button.getAttribute("aria-label")?.trim() &&
                !button.getAttribute("aria-labelledby")?.trim()) {
              problems.push({ rule: "button_missing_name", selector: describe(button) });
            }
          }
          if (!document.title.trim()) problems.push({ rule: "document_missing_title", selector: "html" });
          const images = document.querySelectorAll("img").length;
          const fields = document.querySelectorAll("input,select,textarea").length;
          return { issues: problems, scanned: { images, fields }, complete: problems.length < 100 };
        });
        return {
          data: {
            path: safePagePath(page.url()), ...audit,
            note: "Limited DOM heuristics, not a WCAG conformance audit.",
          },
        };
      }
      if (command.inspection === "axe") {
        // This is the original axe.min.js bytes from the exact npm lockfile.
        // Build-time text generation prevents Wrangler/esbuild from rewriting
        // axeFunction.toString() with an unavailable __name() closure.
        // Playwright evaluation bypasses page inline-script CSP without a CDN.
        await page.evaluate(AXE_SOURCE);
        const audit=await page.evaluate(async () => {
          type AxeViolation={id:string;impact?:string|null;description?:string;
            nodes:Array<{target:string[]}>};
          type AxeReport={violations:AxeViolation[];
            passes:unknown[];incomplete:unknown[];inapplicable:unknown[]};
          const runner=(window as Window & {axe?: {
            run:(document:Document,options:unknown)=>Promise<AxeReport>;
          }}).axe;
          if(!runner) throw new Error("BROWSER_AXE_INJECTION_FAILED");
          const report=await runner.run(document,{
            runOnly:{type:"tag",values:["wcag2a","wcag2aa","wcag21a","wcag21aa","wcag22aa","best-practice"]},
          });
          return {engine:"axe-core",violationsTotal:report.violations.length,
            violations:report.violations.slice(0,50).map(item=>({
              id:item.id,impact:item.impact??null,
              description:(item.description??"").slice(0,200),
              affectedNodes:item.nodes.length,
              targets:item.nodes.slice(0,4).map(node=>node.target.slice(0,3)
                .map(selector=>String(selector).slice(0,180))),
            })),
            passes:report.passes.length,incomplete:report.incomplete.length,
            inapplicable:report.inapplicable.length,
            truncated:report.violations.length>50};
        });
        return {data:{path:safePagePath(page.url()),...audit,
          note:"Actual axe-core automated rules, not full WCAG certification. Some cross-origin frames may require separate audit."}};
      }
      if (command.inspection === "pdf") {
        const result = Buffer.from(await page.pdf({
          format: "A4", printBackground: true,
        }));
        if (result.byteLength > MAX_TRANSFER_BYTES) {
          throw new Error("BROWSER_PDF_TOO_LARGE");
        }
        return {
          data: {
            path: safePagePath(page.url()), mimeType: "application/pdf",
            bytes: result.byteLength, base64: result.toString("base64"),
          },
        };
      }
      throw new Error("BROWSER_INSPECTION_UNKNOWN_KIND");
    }
    case "frames":
      return {
        data: {
          path: safePagePath(page.url()),
          frames: page.frames().slice(0, 50).map((frame, index) => ({
            frameIndex: index,
            name: frame.name(),
            path: safePagePath(frame.url()),
            isMain: frame === page.mainFrame(),
          })),
        },
      };
    case "snapshot": {
      if (command.frameIndex !== undefined && command.frameSelector !== undefined) {
        throw new Error("BROWSER_FRAME_TARGET_CONFLICT");
      }
      if (command.frameSelector !== undefined) {
        if (!command.frameSelector || command.frameSelector.length > 512) {
          throw new Error("BROWSER_FRAME_SELECTOR_INVALID");
        }
        return {
          data: {
            path: safePagePath(page.url()),
            frameSelector: command.frameSelector,
            title: await page.title(),
            accessibility: (await page.frameLocator(command.frameSelector)
              .locator("body").ariaSnapshot()).slice(0, 30_000),
          },
        };
      }
      const frame = command.frameIndex === undefined ? page.mainFrame() :
        page.frames()[command.frameIndex];
      if (!frame) throw new Error("BROWSER_FRAME_NOT_FOUND");
      return {
        data: {
          path: safePagePath(frame.url()),
          frameIndex: page.frames().indexOf(frame),
          title: await page.title(),
          accessibility: (await frame.locator("body").ariaSnapshot()).slice(0, 30000),
        },
      };
    }
    case "screenshot": {
      if (command.screenshotSource === "last_failure") {
        if (!lastFailureScreenshot) throw new Error("BROWSER_FAILURE_SCREENSHOT_NOT_AVAILABLE");
        return {
          data: { path: lastFailureScreenshot.capturedPath,
            mimeType: "image/png", source: "last_failure" },
          image: lastFailureScreenshot.image,
        };
      }
      const png = await page.screenshot({ fullPage: true });
      return {
        data: { path: safePagePath(page.url()), mimeType: "image/png", source: "current" },
        image: Buffer.from(png).toString("base64"),
      };
    }
    case "console":
      return { data: { messages: state.consoleLog.slice(-80), responses: state.requestLog.slice(-80) } };
    case "session": {
      // Optional site convention: read /api/session as the CURRENT browser
      // origin, with ordinary cookies or configured service identity.
      // Non-JSON/404 responses are honest results, not a fabricated session.
      const result = await page.evaluate(async () => {
        const response = await fetch("/api/session", { credentials: "same-origin" });
        const contentType = response.headers.get("content-type") ?? "";
        if (!contentType.includes("json")) {
          return { status: response.status, body: null, contentType };
        }
        return { status: response.status, body: await response.json() };
      });
      return { data: result };
    }
    default: throw new Error("CRM_BROWSER_UNKNOWN_OPERATION");
  }
}
