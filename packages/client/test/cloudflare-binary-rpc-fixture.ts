// Route-wide binary Connect verification against the canonical Buf descriptor set.
import { Buffer } from "node:buffer";
import { readFileSync } from "node:fs";
import { createFileRegistry, fromBinary } from "@bufbuild/protobuf";
import { FileDescriptorSetSchema } from "@bufbuild/protobuf/wkt";
import { DomainErrorDetailSchema } from "../gen/crm/core/v2/operation_pb.js";

const descriptorPath = process.env.CRM_PROTO_DESCRIPTOR_FILE ?? "/tmp/crm-proto-descriptors.binpb";
const registry = createFileRegistry(fromBinary(FileDescriptorSetSchema, readFileSync(descriptorPath)));

// The route-wide runtime inventory deliberately remains descriptor-driven.
export function decodeRpcOutput(path: string, bytes: Uint8Array) {
  const [serviceName, methodName, ...extra] = path.slice(1).split("/");
  if (!serviceName || !methodName || extra.length !== 0) {
    throw new Error("Invalid compiled Buf RPC path: " + path);
  }
  const method = registry.getService(serviceName)?.methods.find(item => item.name === methodName);
  if (!method || method.methodKind !== "unary") {
    throw new Error("Compiled Buf RPC missing or non-unary: " + path);
  }
  return fromBinary(method.output, bytes);
}

export function decodeDomainErrorDetail(detail: Readonly<{ type: string; value: string }>) {
  if (detail.type !== DomainErrorDetailSchema.typeName ||
      !registry.getMessage(DomainErrorDetailSchema.typeName)) {
    throw new Error("Missing generated DomainErrorDetail");
  }
  return fromBinary(DomainErrorDetailSchema, Buffer.from(detail.value, "base64"));
}
