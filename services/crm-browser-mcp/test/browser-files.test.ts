import test from "node:test";
import assert from "node:assert/strict";
import { validatedUpload, transferDownload, MAX_TRANSFER_BYTES } from "../src/browser-files.ts";

test("upload accepts only explicit bounded base64 bytes", () => {
  const input = Buffer.from("hello-world").toString("base64");
  const got = validatedUpload("test.txt", "text/plain", input);
  assert.equal(got.name, "test.txt");
  assert.equal(got.mimeType, "text/plain");
  assert.equal(got.buffer.toString(), "hello-world");
  for (const bad of ["../passwd", "..", "a/b", "a\\b", "", "a\n"]) {
    assert.throws(() => validatedUpload(bad, "text/plain", input), /INVALID_NAME/);
  }
  for (const bad of ["", "invalid", "text/plain;charset=UTF-8"]) {
    assert.throws(() => validatedUpload("a.txt", bad, input), /INVALID_MEDIA_TYPE/);
  }
  for (const bad of ["", "!!", "ab?=", "A==", Buffer.alloc(MAX_TRANSFER_BYTES+1).toString("base64")]) {
    assert.throws(() => validatedUpload("a.txt", "text/plain", bad), /INVALID_BASE64/);
  }
});

test("download data stays bounded and checks size before encoding", () => {
  const value = transferDownload("report.csv", [Buffer.from("a,"), Buffer.from("b\n")]);
  assert.deepEqual(value, { fileName: "report.csv", bytes: 4, base64: Buffer.from("a,b\n").toString("base64") });
  assert.equal(transferDownload("../private/report.csv", [Buffer.from("x")]).fileName, "report.csv");
  assert.equal(transferDownload("C:\\\\logs\\\\report.txt", [Buffer.from("y")]).fileName, "report.txt");
  assert.equal(transferDownload("report" + String.fromCharCode(0) + ".txt", [Buffer.from("z")]).fileName, "report.txt");
  assert.throws(() => transferDownload("huge.csv", [Buffer.alloc(MAX_TRANSFER_BYTES + 1)]), /TOO_LARGE/);
  // Real Browser Run repeated four false 0-byte successes on 14 Nov 2026.
  assert.throws(() => transferDownload("unknown.txt", []), /CONTENT_UNVERIFIED/);
  assert.throws(() => transferDownload("unknown.txt", [Buffer.alloc(0)]), /CONTENT_UNVERIFIED/);
});
