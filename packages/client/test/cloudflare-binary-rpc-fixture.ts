// Test-only binary Connect codecs derived from the canonical generated Buf services.
// Static method descriptors type request/response payloads; the FileDescriptorSet
// remains the runtime source for route-wide response decoding and negative probes.
import { Buffer } from "node:buffer";
import { readFileSync } from "node:fs";
import {
  create,
  createFileRegistry,
  fromBinary,
  toBinary,
} from "@bufbuild/protobuf";
import type {
  DescMessage,
  DescMethodUnary,
  MessageInitShape,
  MessageShape,
} from "@bufbuild/protobuf";
import { FileDescriptorSetSchema } from "@bufbuild/protobuf/wkt";
import { PartyService as PartyV3 } from "../gen/crm/parties/v3/party_pb.js";
import { AccountService } from "../gen/crm/accounts/v2/account_pb.js";
import { ContactPointService } from "../gen/crm/contact_points/v2/contact_point_pb.js";
import { PartyRelationshipService } from "../gen/crm/party_relationships/v2/relationship_pb.js";
import { MemberService } from "../gen/crm/access/v2/member_pb.js";
import { TaskService } from "../gen/crm/activities/v2/task_pb.js";
import { InteractionService } from "../gen/crm/activities/v2/interaction_pb.js";
import { AutomationService } from "../gen/crm/activities/v2/automation_pb.js";
import { DealService as DealServiceV2 } from "../gen/crm/sales/v2/deal_pb.js";
import { PrivacyCaseService } from "../gen/crm/customer_privacy/v2/case_pb.js";
import { DomainErrorDetailSchema } from "../gen/crm/core/v2/operation_pb.js";

type UnaryMethodMap<M> = { [K in keyof M]: DescMethodUnary };
type RpcMethodName<M> = Capitalize<Extract<keyof M, string>>;
type RpcLocalMethodName<M, N extends string> = {
  [K in keyof M]: K extends string ? Capitalize<K> extends N ? K : never : never;
}[keyof M];
type RpcMethod<M, N extends string> = M[Extract<RpcLocalMethodName<M, N>, keyof M>];
type RpcInput<M, N extends string> =
  RpcMethod<M, N> extends DescMethodUnary<infer I, DescMessage> ? MessageInitShape<I> : never;
type RpcOutput<M, N extends string> =
  RpcMethod<M, N> extends DescMethodUnary<DescMessage, infer O> ? MessageShape<O> : never;

function typedServiceWire<const M extends UnaryMethodMap<M>>(
  service: { readonly typeName: string; readonly method: M },
) {
  const resolve = <N extends RpcMethodName<M>>(methodName: N): RpcMethod<M, N> => {
    const key = `${methodName.charAt(0).toLowerCase()}${methodName.slice(1)}` as
      Extract<RpcLocalMethodName<M, N>, keyof M>;
    const method = service.method[key];
    if (!method) {
      throw new Error("Compiled Buf RPC missing: " + service.typeName + "/" + methodName);
    }
    return method;
  };

  return {
    path: <N extends RpcMethodName<M>>(methodName: N): string => {
      const method = resolve(methodName);
      return `/${service.typeName}/${method.name}`;
    },
    binary: <N extends RpcMethodName<M>>(methodName: N, payload: RpcInput<M, N>) => {
      const method = resolve(methodName);
      return toBinary(method.input, create(method.input, payload));
    },
    decode: <N extends RpcMethodName<M>>(methodName: N, bytes: Uint8Array): RpcOutput<M, N> => {
      const method = resolve(methodName);
      return fromBinary(method.output, bytes) as RpcOutput<M, N>;
    },
  };
}

const partyV3 = typedServiceWire(PartyV3);
export const partyV3Path = partyV3.path;
export const binaryPartyV3 = partyV3.binary;
export const decodePartyV3 = partyV3.decode;

const accountV2 = typedServiceWire(AccountService);
export const accountV2Path = accountV2.path;
export const binaryAccountV2 = accountV2.binary;
export const decodeAccountV2 = accountV2.decode;

const contactV2 = typedServiceWire(ContactPointService);
export const contactV2Path = contactV2.path;
export const binaryContactV2 = contactV2.binary;
export const decodeContactV2 = contactV2.decode;

const relationshipV2 = typedServiceWire(PartyRelationshipService);
export const relationshipV2Path = relationshipV2.path;
export const binaryRelationshipV2 = relationshipV2.binary;
export const decodeRelationshipV2 = relationshipV2.decode;

const memberV2 = typedServiceWire(MemberService);
export const memberV2Path = memberV2.path;
export const binaryMemberV2 = memberV2.binary;
export const decodeMemberV2 = memberV2.decode;

const taskV2 = typedServiceWire(TaskService);
export const taskV2Path = taskV2.path;
export const binaryTaskV2 = taskV2.binary;
export const decodeTaskV2 = taskV2.decode;

const interactionV2 = typedServiceWire(InteractionService);
export const interactionV2Path = interactionV2.path;
export const binaryInteractionV2 = interactionV2.binary;
export const decodeInteractionV2 = interactionV2.decode;

const automationV2 = typedServiceWire(AutomationService);
export const automationV2Path = automationV2.path;
export const binaryAutomationV2 = automationV2.binary;
export const decodeAutomationV2 = automationV2.decode;

const salesV2 = typedServiceWire(DealServiceV2);
export const salesV2Path = salesV2.path;
export const binarySalesV2 = salesV2.binary;
export const decodeSalesV2 = salesV2.decode;

const privacyV2 = typedServiceWire(PrivacyCaseService);
export const privacyV2Path = privacyV2.path;
export const binaryPrivacyV2 = privacyV2.binary;
export const decodePrivacyV2 = privacyV2.decode;

const descriptorPath = process.env.CRM_PROTO_DESCRIPTOR_FILE ?? "/tmp/crm-proto-descriptors.binpb";
const registry = createFileRegistry(fromBinary(FileDescriptorSetSchema, readFileSync(descriptorPath)));

// The route-wide runtime inventory deliberately remains descriptor-driven.
// It handles arbitrary active methods and malformed/unknown route probes.
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
