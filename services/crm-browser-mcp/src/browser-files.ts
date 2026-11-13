// File payloads are explicitly supplied by the authenticated MCP controller.
// Never read Worker filesystem paths or construct URLs to fetch upload bytes.
export const MAX_TRANSFER_BYTES = 1_000_000;

export function validatedUpload(
  name: string,
  mediaType: string,
  base64: string,
): { name: string; mimeType: string; buffer: Buffer } {
  if (!name || name.length > 128 || name === "." || name === ".." ||
      name.includes("/") || name.includes("\\") || /[\x00-\x1f\x7f]/.test(name)) {
    throw new Error("BROWSER_FILE_INVALID_NAME");
  }
  if (!/^[\w.+-]+\/[\w.+-]+$/.test(mediaType) || mediaType.length > 100) {
    throw new Error("BROWSER_FILE_INVALID_MEDIA_TYPE");
  }
  if (!base64 || base64.length > 1_333_336 ||
      !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(base64)) {
    throw new Error("BROWSER_FILE_INVALID_BASE64");
  }
  const buffer = Buffer.from(base64, "base64");
  if (buffer.byteLength > MAX_TRANSFER_BYTES || buffer.toString("base64") !== base64) {
    throw new Error("BROWSER_FILE_INVALID_BASE64");
  }
  return { name, mimeType: mediaType, buffer };
}

export function transferDownload(
  filename: string,
  parts: Uint8Array[],
): { fileName: string; bytes: number; base64: string } {
  const bytes = parts.reduce((total, item) => total + item.byteLength, 0);
  if (bytes > MAX_TRANSFER_BYTES) throw new Error("BROWSER_DOWNLOAD_TOO_LARGE");
  const buffer = Buffer.concat(parts.map(item => Buffer.from(item)));
  return { fileName: filename.slice(0, 128), bytes, base64: buffer.toString("base64") };
}
