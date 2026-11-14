import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { WorkspaceFileTransfers, validatedBinary, safeDownloadName,
  MAX_WORKSPACE_FILE_BYTES, MAX_WORKSPACE_CHUNK_BYTES } from "../src/browser-transfers.ts";

const sha = (value: Buffer) => createHash("sha256").update(value).digest("hex");

test("a >1 MB upload is accepted only after every chunk and exact SHA-256 verification", () => {
  const store = new WorkspaceFileTransfers();
  const payload = Buffer.alloc(1_350_123);
  for (let i = 0; i < payload.length; i++) payload[i] = i % 251;
  const upload = store.beginUpload("report.bin", "application/octet-stream");
  assert.equal(upload.maxBytes, MAX_WORKSPACE_FILE_BYTES);
  assert.match(upload.transferId, /^[0-9a-f-]{36}$/);
  for (let offset = 0; offset < payload.byteLength; offset += 196_608) {
    const chunk = payload.subarray(offset, offset + 196_608);
    const ack = store.appendUpload(upload.transferId, chunk.toString("base64"));
    assert.equal(ack.bytes, Math.min(offset + 196_608, payload.length));
  }
  assert.throws(() => store.preparedUpload(upload.transferId, "0".repeat(64)), /SHA256_MISMATCH/);
  const checked = store.preparedUpload(upload.transferId, sha(payload));
  assert.equal(checked.bytes, payload.length);
  assert.equal(checked.buffer.compare(payload), 0);
  assert.equal(checked.sha256, sha(payload));
  store.finishUpload(upload.transferId);
  assert.throws(() => store.preparedUpload(upload.transferId, sha(payload)), /NOT_FOUND/);
});

test("download receipts contain only verified real bytes and reconstruct from bounded pieces", () => {
  const store = new WorkspaceFileTransfers();
  const payload = Buffer.alloc(450_123);
  for (let i = 0; i < payload.length; i++) payload[i] = i % 233;
  const receipt = store.storeDownload("../../report.pdf", "application/pdf", payload.toString("base64"));
  assert.equal(receipt.fileName, "report.pdf");
  assert.equal(receipt.mimeType, "application/pdf");
  assert.equal(receipt.bytes, payload.length);
  assert.equal(receipt.sha256, sha(payload));
  const pieces: Buffer[] = [];
  for (let offset = 0; offset < payload.length; offset += 64_000) {
    const part = store.readDownload(receipt.transferId, offset, 64_000);
    assert.equal(part.offset, offset);
    assert.equal(part.sha256, receipt.sha256);
    assert.equal(part.done, offset + part.bytes === payload.length);
    pieces.push(Buffer.from(part.base64, "base64"));
  }
  assert.equal(Buffer.concat(pieces).compare(payload), 0);
  store.discard(receipt.transferId);
  assert.throws(() => store.readDownload(receipt.transferId, 0), /NOT_FOUND/);
});

test("transfer does not silently accept empty or malformed data", () => {
  const store = new WorkspaceFileTransfers();
  for (const bad of ["", "%%%%", "A==", "AB", Buffer.alloc(MAX_WORKSPACE_CHUNK_BYTES + 1).toString("base64")]) {
    assert.throws(() => validatedBinary(bad), /INVALID_BASE64/);
  }
  assert.throws(() => store.storeDownload("empty.txt", "text/plain", ""), /INVALID_BASE64/);
  assert.equal(safeDownloadName("C:\\tmp\\secret.txt"), "secret.txt");
  assert.equal(safeDownloadName("../a\0b.txt"), "ab.txt");
  const upload = store.beginUpload("data.txt", "text/plain");
  assert.throws(() => store.beginUpload("other.txt", "text/plain"), /ALREADY_ACTIVE/);
  assert.throws(() => store.appendUpload(crypto.randomUUID(), "YQ=="), /NOT_FOUND/);
  assert.throws(() => store.preparedUpload(upload.transferId, sha(Buffer.from("abc"))), /UPLOAD_EMPTY/);
  store.appendUpload(upload.transferId, Buffer.from("abc").toString("base64"));
  assert.throws(() => store.preparedUpload(upload.transferId, "xxx"), /SHA256_REQUIRED/);
  assert.throws(() => store.readDownload(crypto.randomUUID(), 0), /NOT_FOUND/);
  assert.deepEqual(store.discard(upload.transferId), { discarded: true });
});

test("one transfer in each direction and explicit close cleanup", () => {
  const store = new WorkspaceFileTransfers();
  const upload = store.beginUpload("data.txt", "text/plain");
  const download = store.storeDownload("data.bin", "application/octet-stream", "YQ==");
  store.appendUpload(upload.transferId, "Yg==");
  assert.equal(store.readDownload(download.transferId, 0).bytes, 1);
  store.clear();
  assert.throws(() => store.appendUpload(upload.transferId, "Yw=="), /NOT_FOUND/);
  assert.throws(() => store.readDownload(download.transferId, 0), /NOT_FOUND/);
});
