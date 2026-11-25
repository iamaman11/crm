import assert from "node:assert/strict";

export function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
export function record(value: unknown): Record<string, unknown> {
  assert(isRecord(value), "Expected JSON object");
  return value;
}
export function records(value: unknown): Record<string, unknown>[] {
  assert(Array.isArray(value), "Expected JSON object array");
  return value.map(record);
}
export function string(value: unknown): string {
  assert(typeof value === "string", "Expected JSON string");
  return value;
}
export function strings(value: unknown): string[] {
  assert(Array.isArray(value), "Expected JSON string array");
  return value.map(string);
}
export function number(value: unknown): number {
  assert(typeof value === "number" && Number.isFinite(value), "Expected finite JSON number");
  return value;
}
