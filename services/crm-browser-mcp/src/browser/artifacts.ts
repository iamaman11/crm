// Browser capabilities operate on the existing DO-owned page and state.
import type { BrowserContext, Page } from "@cloudflare/playwright";
import type { Command, Env, BrowserCommandResult } from "../contracts";
import { safePagePath } from "./security";
import { MAX_VISUAL_PNG_BYTES, comparePngPixels, pngReceipt, visualThreshold } from "../browser-visual";
import { validatedUpload, transferDownload } from "../browser-files";
import { type WorkspaceFileTransfers, MAX_WORKSPACE_FILE_BYTES, MAX_WORKSPACE_CHUNK_BYTES } from "../browser-transfers";
import { recordingTargets, recordingHar } from "../browser-recordings";
import { resolveTarget } from "./interactions";

export async function recordingCommand(command: Command, env: Env, storage: DurableObjectStorage,
    transfers: WorkspaceFileTransfers, browserSessionId: string | null): Promise<BrowserCommandResult> {
  // Recordings are finalized ONLY after Close. Reading them must never
  // acquire a new browser or create a new page/context.
  if (command.recordingAction === "chunk") {
    return { data: transfers.readDownload(
      command.transferId ?? "", command.offset ?? -1, command.chunkBytes,
    ) };
  }
  if (command.recordingAction === "discard") {
    return { data: transfers.discard(command.transferId ?? "") };
  }
  if (browserSessionId) throw new Error("BROWSER_RECORDING_REQUIRES_CLOSED_SESSION");
  const sessionId = await storage.get<string>("last_recording_session_id");
  if (!sessionId) throw new Error("BROWSER_RECORDING_NOT_FOUND");
  const token = env.CF_BROWSER_RECORDING_READ_TOKEN;
  if (!token) throw new Error("BROWSER_RECORDING_READ_TOKEN_MISSING");
  if (command.recordingAction === "targets") {
    return { data: {
      sessionId, targets: await recordingTargets(env.CF_BROWSER_ACCOUNT_ID, token, sessionId),
    } };
  }
  if (command.recordingAction === "har") {
    if (!command.recordingTarget) throw new Error("BROWSER_HAR_TARGET_REQUIRED");
    const har = await recordingHar(
      env.CF_BROWSER_ACCOUNT_ID, token, sessionId, command.recordingTarget,
    );
    const buffer = Buffer.from(JSON.stringify(har));
    const receipt = transfers.storeDownload(
      "browser-" + sessionId.slice(0, 8) + ".har", "application/json",
      buffer.toString("base64"),
    );
    return { data: {
      action: "har", sanitized: true, format: "HAR 1.2",
      entryCount: har.log.entries.length, ...receipt,
      ...(receipt.bytes <= MAX_WORKSPACE_CHUNK_BYTES ?
        { base64: buffer.toString("base64") } : {}),
    } };
  }
  throw new Error("BROWSER_RECORDING_UNKNOWN_ACTION");
}

export async function artifactCommand(page: Page, command: Command, storage: DurableObjectStorage,
    transfers: WorkspaceFileTransfers, context: BrowserContext | null): Promise<BrowserCommandResult> {
  switch (command.op) {
    case "visual": {
      const action=command.visualAction;
      if (action==="discard") {
        await storage.delete("visual_baseline");
        return {data:{discarded:true}};
      }
      if (action!=="capture" && action!=="compare")
        throw new Error("BROWSER_VISUAL_ACTION_INVALID");
      const threshold=visualThreshold(command.visualThreshold);
      const png=Buffer.from(await page.screenshot({fullPage:false}));
      const receipt=pngReceipt(png);
      if(action==="capture") {
        await storage.put("visual_baseline",
          {base64:png.toString("base64"),sha256:receipt.sha256,bytes:receipt.bytes});
        return {data:{baseline:true,path:safePagePath(page.url()),...receipt}};
      }
      const baseline=await storage.get<{base64:string;sha256:string;bytes:number}>("visual_baseline");
      if(!baseline) throw new Error("BROWSER_VISUAL_BASELINE_MISSING");
      if(!baseline.bytes || baseline.bytes>MAX_VISUAL_PNG_BYTES ||
          Buffer.byteLength(baseline.base64,"base64")!==baseline.bytes)
        throw new Error("BROWSER_VISUAL_BASELINE_INVALID");
      const result=await comparePngPixels(context!,baseline.base64,png.toString("base64"),threshold);
      return {data:{...result,path:safePagePath(page.url()),
        baselineSha256:baseline.sha256,currentSha256:receipt.sha256,
        note:"Pixel comparison of actual browser viewport PNGs, max channel delta > 24. Difference is measured in an isolated blank tab; no second browser."}};
    }
    case "file": {
      // A single file transfer lives only in this DO's memory. If Cloudflare
      // evicts the DO mid-transfer, subsequent calls fail, never return
      // corrupted or invented bytes. Browser state stays owned by Playwright.
      if (command.fileAction === "upload_begin") {
        return { data: transfers.beginUpload(command.fileName ?? "", command.mimeType ?? "") };
      }
      if (command.fileAction === "upload_chunk") {
        return { data: transfers.appendUpload(command.transferId ?? "", command.base64 ?? "") };
      }
      if (command.fileAction === "discard") {
        return { data: transfers.discard(command.transferId ?? "") };
      }
      if (command.fileAction === "download_chunk") {
        return { data: transfers.readDownload(
          command.transferId ?? "", command.offset ?? -1, command.chunkBytes,
        ) };
      }
      if (command.fileAction === "upload_commit") {
        if (!command.target) throw new Error("BROWSER_FILE_TARGET_REQUIRED");
        const transferId = command.transferId ?? "";
        const file = transfers.preparedUpload(transferId, command.expectedSha256 ?? "");
        await resolveTarget(page, command.target).setInputFiles({
          name: file.name, mimeType: file.mimeType, buffer: file.buffer,
        }, { timeout: 15_000 });
        transfers.finishUpload(transferId);
        return { data: {
          action: "upload", completed: true, fileName: file.name,
          mimeType: file.mimeType, bytes: file.bytes, sha256: file.sha256,
          path: safePagePath(page.url()),
        } };
      }
      if (!command.target) throw new Error("BROWSER_FILE_TARGET_REQUIRED");
      const locator = resolveTarget(page, command.target);
      if (command.fileAction === "download_link" || command.fileAction === "download") {
        // Browser-side fetch is intentionally same-origin, using the
        // actual page's cookies/CORS and Playwright request routing.
        // Unlike a browser download event, response bytes are verified.
        // No Worker-side remote URL fetch and no speculative re-click.
        const response = await locator.evaluate(async (element, maxBytes) => {
          const link = element.closest("a[href]");
          if (!(link instanceof HTMLAnchorElement)) throw new Error("BROWSER_FILE_LINK_REQUIRED");
          const url = new URL(link.href, location.href);
          if (!["http:", "https:"].includes(url.protocol) ||
              url.origin !== location.origin) {
            throw new Error("BROWSER_FILE_LINK_CROSS_ORIGIN_UNSUPPORTED");
          }
          const downloaded = await fetch(url.href, {
            credentials: "same-origin", mode: "same-origin", redirect: "follow",
          });
          if (!downloaded.ok || new URL(downloaded.url).origin !== location.origin) {
            throw new Error("BROWSER_DOWNLOAD_HTTP_FAILED");
          }
          const length = downloaded.headers.get("content-length");
          if (length && Number(length) > maxBytes) throw new Error("BROWSER_DOWNLOAD_TOO_LARGE");
          if (!downloaded.body) throw new Error("BROWSER_DOWNLOAD_BODY_UNAVAILABLE");
          const reader = downloaded.body.getReader();
          const chunks: Uint8Array[] = [];
          let size = 0;
          try {
            for (;;) {
              const { done, value } = await reader.read();
              if (done) break;
              size += value.byteLength;
              if (size > maxBytes) throw new Error("BROWSER_DOWNLOAD_TOO_LARGE");
              chunks.push(value);
            }
          } finally { await reader.cancel().catch(() => {}); }
          if (!size) throw new Error("BROWSER_DOWNLOAD_CONTENT_UNVERIFIED");
          const bytes = new Uint8Array(size);
          let offset = 0;
          for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength; }
          let binary = "";
          for (let i = 0; i < size; i += 8192) {
            binary += String.fromCharCode(...bytes.subarray(i, i + 8192));
          }
          return {
            name: url.pathname.split("/").pop() || "download",
            mimeType: downloaded.headers.get("content-type") ?? "application/octet-stream",
            base64: btoa(binary),
          };
        }, MAX_WORKSPACE_FILE_BYTES, { timeout: 45_000 });
        if (command.fileAction === "download") {
          // Browser Run/CDP exposes a download event, but the stream is empty.
          // Use the same supported browser-side, same-origin link fetch as
          // download_link; never replay HTTP via a Worker or invent bytes.
          // Legacy inline downloads remain capped at 1 MB.
          const bytes = Buffer.from(response.base64, "base64");
          return { data: {
            action: "download", completed: true, mimeType: response.mimeType,
            ...transferDownload(response.name, [bytes]),
          } };
        }
        const receipt = transfers.storeDownload(response.name, response.mimeType, response.base64);
        return { data: {
          action: "download_link", completed: true,
          ...receipt,
          // Small files are one-call results. Larger verified files are
          // returned in bounded chunks using this same transferId.
          ...(receipt.bytes <= MAX_WORKSPACE_CHUNK_BYTES ?
            { base64: response.base64 } : {}),
        } };
      }
      if (command.fileAction === "upload") {
        const file = validatedUpload(
          command.fileName ?? "", command.mimeType ?? "", command.base64 ?? "",
        );
        await locator.setInputFiles(file, { timeout: 15_000 });
        return {
          data: {
            action: "upload", fileName: file.name, mimeType: file.mimeType,
            bytes: file.buffer.byteLength, completed: true,
            path: safePagePath(page.url()),
          },
        };
      }
      throw new Error("BROWSER_FILE_UNKNOWN_ACTION");
    }
    default: throw new Error("CRM_BROWSER_UNKNOWN_OPERATION");
  }
}
