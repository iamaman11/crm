import { createHash } from "node:crypto";
import { validatedUpload } from "./browser-files";

// A single bounded, transient binary transfer per direction per workspace.
// The Browser Run page and Cloudflare DO are still the only owners. No R2,
// filesystem paths, external fetch proxy or independent browser process.
export const MAX_WORKSPACE_FILE_BYTES = 8_000_000;
export const MAX_WORKSPACE_CHUNK_BYTES = 256_000;
export const DEFAULT_DOWNLOAD_CHUNK_BYTES = 64_000;

function hash(buffer: Buffer): string {
  return createHash("sha256").update(buffer).digest("hex");
}

export function safeDownloadName(value: string): string {
  const leaf = value.replaceAll("\\", "/").split("/").pop() ?? "";
  return leaf.replace(/[\x00-\x1f\x7f]/g, "").slice(0, 128) || "download";
}

export function validatedBinary(
  base64: string,
  maxBytes: number = MAX_WORKSPACE_CHUNK_BYTES,
): Buffer {
  if (!base64 || base64.length > Math.ceil(maxBytes / 3) * 4 ||
      !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(base64)) {
    throw new Error("BROWSER_FILE_INVALID_BASE64");
  }
  const bytes = Buffer.from(base64, "base64");
  if (bytes.byteLength > maxBytes || bytes.toString("base64") !== base64) {
    throw new Error("BROWSER_FILE_INVALID_BASE64");
  }
  return bytes;
}

export type BrowserFileTransferReceipt = {
  transferId: string; fileName: string; mimeType: string;
  bytes: number; sha256: string;
};

type Upload = {
  id: string; name: string; mimeType: string;
  chunks: Buffer[]; bytes: number;
};
type Download = { id: string; receipt: BrowserFileTransferReceipt; bytes: Buffer };

export class WorkspaceFileTransfers {
  private upload: Upload | null = null;
  private download: Download | null = null;

  beginUpload(name: string, mimeType: string): { transferId: string; accepted: boolean; maxBytes: number } {
    // Reuse the existing file name / MIME validator, without imposing the
    // legacy 1 MB maximum on the aggregate upload.
    validatedUpload(name, mimeType, Buffer.from("x").toString("base64"));
    if (this.upload) throw new Error("BROWSER_UPLOAD_ALREADY_ACTIVE");
    const id = crypto.randomUUID();
    this.upload = { id, name, mimeType, chunks: [], bytes: 0 };
    return { transferId: id, accepted: true, maxBytes: MAX_WORKSPACE_FILE_BYTES };
  }

  appendUpload(id: string, base64: string): { accepted: boolean; bytes: number } {
    if (!this.upload || this.upload.id !== id) throw new Error("BROWSER_UPLOAD_NOT_FOUND");
    const bytes = validatedBinary(base64);
    if (this.upload.bytes + bytes.byteLength > MAX_WORKSPACE_FILE_BYTES) {
      throw new Error("BROWSER_UPLOAD_TOO_LARGE");
    }
    this.upload.chunks.push(bytes);
    this.upload.bytes += bytes.byteLength;
    return { accepted: true, bytes: this.upload.bytes };
  }

  preparedUpload(id: string, expectedSha256: string): {
    name: string; mimeType: string; buffer: Buffer; sha256: string; bytes: number
  } {
    if (!this.upload || this.upload.id !== id) throw new Error("BROWSER_UPLOAD_NOT_FOUND");
    if (!this.upload.bytes) throw new Error("BROWSER_UPLOAD_EMPTY");
    if (!/^[0-9a-f]{64}$/.test(expectedSha256)) throw new Error("BROWSER_UPLOAD_SHA256_REQUIRED");
    const buffer = Buffer.concat(this.upload.chunks, this.upload.bytes);
    const sha256 = hash(buffer);
    if (sha256 !== expectedSha256) throw new Error("BROWSER_UPLOAD_SHA256_MISMATCH");
    return {
      name: this.upload.name, mimeType: this.upload.mimeType,
      buffer, sha256, bytes: buffer.byteLength,
    };
  }

  finishUpload(id: string): void {
    if (!this.upload || this.upload.id !== id) throw new Error("BROWSER_UPLOAD_NOT_FOUND");
    this.upload = null;
  }

  storeDownload(name: string, mimeType: string, base64: string): BrowserFileTransferReceipt {
    // Only the verified successful Browser page fetch is allowed to stage data.
    const bytes = validatedBinary(base64, MAX_WORKSPACE_FILE_BYTES);
    if (!bytes.byteLength) throw new Error("BROWSER_DOWNLOAD_CONTENT_UNVERIFIED");
    const id = crypto.randomUUID();
    const receipt = {
      transferId: id, fileName: safeDownloadName(name),
      mimeType: mimeType.slice(0, 100), bytes: bytes.byteLength,
      sha256: hash(bytes),
    };
    this.download = { id, receipt, bytes };
    return receipt;
  }

  readDownload(id: string, offset: number, length: number = DEFAULT_DOWNLOAD_CHUNK_BYTES): {
    transferId: string; offset: number; bytes: number; totalBytes: number;
    sha256: string; done: boolean; base64: string;
  } {
    if (!this.download || this.download.id !== id) throw new Error("BROWSER_DOWNLOAD_NOT_FOUND");
    if (!Number.isSafeInteger(offset) || offset < 0 ||
        offset >= this.download.bytes.byteLength ||
        !Number.isSafeInteger(length) || length <= 0 || length > MAX_WORKSPACE_CHUNK_BYTES) {
      throw new Error("BROWSER_DOWNLOAD_CHUNK_OUT_OF_RANGE");
    }
    const chunk = this.download.bytes.subarray(offset, offset + length);
    return {
      transferId: id, offset, bytes: chunk.byteLength,
      totalBytes: this.download.bytes.byteLength,
      sha256: this.download.receipt.sha256,
      done: offset + chunk.byteLength >= this.download.bytes.byteLength,
      base64: chunk.toString("base64"),
    };
  }

  discard(id: string): { discarded: boolean } {
    let discarded = false;
    if (this.download?.id === id) { this.download = null; discarded = true; }
    if (this.upload?.id === id) { this.upload = null; discarded = true; }
    if (!discarded) throw new Error("BROWSER_FILE_TRANSFER_NOT_FOUND");
    return { discarded };
  }

  clear(): void { this.download = null; this.upload = null; }
}
