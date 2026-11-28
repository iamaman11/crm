// Shared compile-time contracts. This module has no runtime behavior.
import type { CrmBrowserSession } from "./session.ts";
import type { BrowserDevice } from "./browser-device.ts";
import type {
  BrowserAssertionKind, TextMatch, WaitState, ScenarioOperation,
} from "./browser-checks.ts";
import type { CdpCommand } from "./browser-cdp.ts";

export interface Env {
  BROWSER: BrowserRun;
  SESSIONS: DurableObjectNamespace<CrmBrowserSession>;
  SITE_ACCESS_ORIGIN?: string; // Optional exact HTTPS origin for automated Cloudflare Access.
  SITE_ACCESS_CLIENT_ID?: string; // Optional Worker secret, never a browser parameter.
  SITE_ACCESS_CLIENT_SECRET?: string; // Must be paired with CLIENT_ID.
  SITE_ACCESS_PREVIEW_1_ORIGIN?: string; // Exact Access origin for isolated Writer #490.
  SITE_ACCESS_PREVIEW_1_CLIENT_ID?: string; // Separate protected Worker secret.
  SITE_ACCESS_PREVIEW_1_CLIENT_SECRET?: string; // Separate protected Worker secret.
  SITE_ACCESS_PREVIEW_2_ORIGIN?: string; // Exact Access origin for isolated Writer #491.
  SITE_ACCESS_PREVIEW_2_CLIENT_ID?: string; // Separate protected Worker secret.
  SITE_ACCESS_PREVIEW_2_CLIENT_SECRET?: string; // Separate protected Worker secret.
  CRM_MCP_ALLOWED_EMAILS: string; // Comma/newline-separated allowlist of human MCP controllers.
  CRM_MCP_ACCESS_AUD: string;
  CF_BROWSER_ACCOUNT_ID: string;
  CF_BROWSER_RECORDING_READ_TOKEN?: string; // Worker secret: Browser Run REST recording read only.
}

export type Target = {
  kind: "css" | "role" | "text" | "label";
  value: string;
  name?: string;
  frameIndex?: number;
  frameSelector?: string;
};

export type Command = {
  op: ScenarioOperation | "open" | "close" | "status" | "session" | "tabs" | "frames" | "file" | "inspect" | "visual" | "viewport" | "snapshot" | "screenshot" | "console" | "live_view" | "handoff" | "run" | "execute" | "recording" | "runtime";
  expectedRuntimeEpoch?: string; // Worker-supplied internal compatibility fence.
  workspaceId?: string; // MCP routing only; stripped before sending to this DO.
  url?: string;
  target?: Target;
  destination?: Target;
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
  cdpCommands?: CdpCommand[];
  tabAction?: "list" | "new" | "select" | "close";
  tabId?: string;
  frameIndex?: number;
  frameSelector?: string;
  followPopup?: boolean;
  fileAction?: "upload" | "download" | "upload_begin" | "upload_chunk" | "upload_commit" | "download_link" | "download_chunk" | "discard";
  fileName?: string;
  mimeType?: string;
  base64?: string;
  transferId?: string;
  expectedSha256?: string;
  offset?: number;
  chunkBytes?: number;
  recordingAction?: "targets" | "har" | "chunk" | "discard";
  recordingTarget?: string;
  inspection?: "network" | "performance" | "accessibility" | "axe" | "pdf";
  visualAction?: "capture" | "compare" | "discard";
  visualThreshold?: number;
  width?: number;
  height?: number;
  recording?: boolean;
  device?: BrowserDevice;
  handoffAction?: "start" | "status";
  handoffInstructions?: string;
  handoffTimeoutMs?: number;
  screenshotOnFailure?: boolean;
  screenshotSource?: "current" | "last_failure";
};

export type BrowserCommandResult = { data: unknown; image?: string };
export type FailureScreenshot = { image: string; capturedPath: string };
