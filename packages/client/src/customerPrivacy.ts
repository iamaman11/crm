import { type Client } from "@connectrpc/connect";
import {
  create,
  fromBinary,
  toBinary,
  type DescMessage,
  type MessageShape,
} from "@bufbuild/protobuf";
import { PartyRefSchema } from "../gen/crm/customer/v1/reference_pb";
import {
  GetPrivacyCaseRequestSchema,
  GetPrivacyCaseResponseSchema,
  ListPrivacyCasesRequestSchema,
  ListPrivacyCasesResponseSchema,
  type GetPrivacyCaseResponse,
  type ListPrivacyCasesResponse,
} from "../gen/crm/customer_privacy/v1/cases_pb";
import { PrivacyCaseRefSchema } from "../gen/crm/customer_privacy/v1/reference_pb";
import { ApplicationGatewayService, TypedPayloadSchema } from "../gen/crm/gateway/v1/gateway_pb";
import { CONTRACT_HASHES } from "./contract_hashes";
import {
  ProductClientError,
  mapGatewayError,
  type GovernedGatewayClientOptions,
} from "./gateway";
import { requireAuthenticatedSession, type SessionProvider } from "./session";
import { createApplicationGatewayClient } from "./transport";

const OWNER_MODULE_ID = "crm.customer-privacy";
const CONTRACT_VERSION = "1.0.0";
const DATA_CLASS = "confidential";
const ENCODING = "protobuf";
const MAX_PROTOBUF_BYTES = 1048576n;
const RETENTION_POLICY_ID = "standard";
const DEFAULT_PAGE_SIZE = 25;
const MAX_PAGE_SIZE = 100;

export interface ListCustomerPrivacyCasesOptions {
  canonicalPartyId: string;
  pageSize?: number;
  cursor?: string;
}

interface QueryContract<I extends DescMessage, O extends DescMessage> {
  capabilityId: string;
  inputSchemaId: string;
  inputSchema: I;
  outputSchemaId: string;
  outputSchema: O;
}

const LIST_CASES_CONTRACT = {
  capabilityId: "customer_privacy.case.list",
  inputSchemaId: "crm.customer_privacy.v1.ListPrivacyCasesRequest",
  inputSchema: ListPrivacyCasesRequestSchema,
  outputSchemaId: "crm.customer_privacy.v1.ListPrivacyCasesResponse",
  outputSchema: ListPrivacyCasesResponseSchema,
} satisfies QueryContract<
  typeof ListPrivacyCasesRequestSchema,
  typeof ListPrivacyCasesResponseSchema
>;

const GET_CASE_CONTRACT = {
  capabilityId: "customer_privacy.case.get",
  inputSchemaId: "crm.customer_privacy.v1.GetPrivacyCaseRequest",
  inputSchema: GetPrivacyCaseRequestSchema,
  outputSchemaId: "crm.customer_privacy.v1.GetPrivacyCaseResponse",
  outputSchema: GetPrivacyCaseResponseSchema,
} satisfies QueryContract<
  typeof GetPrivacyCaseRequestSchema,
  typeof GetPrivacyCaseResponseSchema
>;

export class GovernedCustomerPrivacyClient {
  private readonly gatewayClient: Client<typeof ApplicationGatewayService>;
  private readonly sessionProvider: SessionProvider;

  public constructor(options: GovernedGatewayClientOptions) {
    this.sessionProvider = options.sessionProvider;
    this.gatewayClient = createApplicationGatewayClient(options);
  }

  public async listCases(
    options: ListCustomerPrivacyCasesOptions,
  ): Promise<ListPrivacyCasesResponse> {
    const canonicalPartyId = requireOpaqueId(
      options.canonicalPartyId,
      "canonical Party reference",
    );
    const pageSize = options.pageSize ?? DEFAULT_PAGE_SIZE;
    if (!Number.isInteger(pageSize) || pageSize < 1 || pageSize > MAX_PAGE_SIZE) {
      throw invalidArgument(
        `Page size must be an integer from 1 through ${MAX_PAGE_SIZE}.`,
        "CUSTOMER_PRIVACY_PAGE_SIZE_INVALID",
      );
    }
    return await this.query(
      LIST_CASES_CONTRACT,
      create(ListPrivacyCasesRequestSchema, {
        canonicalPartyRef: create(PartyRefSchema, { partyId: canonicalPartyId }),
        pageSize,
        cursor: options.cursor ?? "",
      }),
    );
  }

  public async getCase(privacyCaseId: string): Promise<GetPrivacyCaseResponse> {
    const validatedCaseId = requireOpaqueId(privacyCaseId, "privacy case reference");
    return await this.query(
      GET_CASE_CONTRACT,
      create(GetPrivacyCaseRequestSchema, {
        privacyCaseRef: create(PrivacyCaseRefSchema, {
          privacyCaseId: validatedCaseId,
        }),
      }),
    );
  }

  private async query<I extends DescMessage, O extends DescMessage>(
    contract: QueryContract<I, O>,
    input: MessageShape<I>,
  ): Promise<MessageShape<O>> {
    try {
      requireAuthenticatedSession(this.sessionProvider.getSnapshot());
      const response = await this.gatewayClient.query({
        ownerModuleId: OWNER_MODULE_ID,
        capabilityId: contract.capabilityId,
        capabilityVersion: CONTRACT_VERSION,
        input: createPayload(contract.inputSchemaId, contract.inputSchema, input),
      });
      if (!response.output) {
        throw contractFailure("Gateway response did not contain an output payload.");
      }
      return decodePayload(contract.outputSchemaId, contract.outputSchema, response.output);
    } catch (error) {
      throw mapGatewayError(error);
    }
  }
}

function createPayload<I extends DescMessage>(
  schemaId: string,
  schema: I,
  message: MessageShape<I>,
) {
  const payload = toBinary(schema, message);
  if (BigInt(payload.length) > MAX_PROTOBUF_BYTES) {
    throw invalidArgument(
      "The encoded Customer Privacy payload exceeds the permitted size.",
      "CUSTOMER_PRIVACY_PROTOBUF_PAYLOAD_TOO_LARGE",
    );
  }
  return create(TypedPayloadSchema, {
    ownerModuleId: OWNER_MODULE_ID,
    schemaId,
    schemaVersion: CONTRACT_VERSION,
    descriptorHash: requireDescriptorHash(schemaId),
    dataClass: DATA_CLASS,
    encoding: ENCODING,
    maximumSizeBytes: MAX_PROTOBUF_BYTES,
    retentionPolicyId: RETENTION_POLICY_ID,
    payload,
  });
}

function decodePayload<O extends DescMessage>(
  expectedSchemaId: string,
  schema: O,
  output: {
    ownerModuleId: string;
    schemaId: string;
    schemaVersion: string;
    descriptorHash: Uint8Array;
    dataClass: string;
    encoding: string;
    maximumSizeBytes: bigint;
    retentionPolicyId: string;
    payload: Uint8Array;
  },
): MessageShape<O> {
  if (
    output.ownerModuleId !== OWNER_MODULE_ID ||
    output.schemaId !== expectedSchemaId ||
    output.schemaVersion !== CONTRACT_VERSION ||
    output.dataClass !== DATA_CLASS ||
    output.encoding !== ENCODING ||
    output.maximumSizeBytes !== MAX_PROTOBUF_BYTES ||
    output.retentionPolicyId !== RETENTION_POLICY_ID ||
    !equalUint8Arrays(output.descriptorHash, requireDescriptorHash(expectedSchemaId)) ||
    BigInt(output.payload.length) > output.maximumSizeBytes
  ) {
    throw contractFailure(`Contract verification failed for ${expectedSchemaId}.`);
  }
  try {
    return fromBinary(schema, output.payload);
  } catch (error) {
    throw new ProductClientError({
      kind: "internal",
      message: `Contract verification failed: malformed ${expectedSchemaId} payload.`,
      retryable: false,
      cause: error,
    });
  }
}

function requireDescriptorHash(schemaId: string): Uint8Array {
  const descriptorHash = CONTRACT_HASHES[schemaId];
  if (!descriptorHash) {
    throw contractFailure(`Missing local contract descriptor hash for ${schemaId}.`);
  }
  return descriptorHash;
}

function requireOpaqueId(value: string, label: string): string {
  const normalized = value.trim();
  if (normalized.length === 0 || normalized.length > 128) {
    throw invalidArgument(
      `A non-empty ${label} of at most 128 characters is required.`,
      "CUSTOMER_PRIVACY_REFERENCE_INVALID",
    );
  }
  return normalized;
}

function invalidArgument(message: string, safeCode: string): ProductClientError {
  return new ProductClientError({
    kind: "invalid_argument",
    message,
    retryable: false,
    safeCode,
  });
}

function contractFailure(message: string): ProductClientError {
  return new ProductClientError({
    kind: "internal",
    message,
    retryable: false,
    safeCode: "CUSTOMER_PRIVACY_CONTRACT_VERIFICATION_FAILED",
  });
}

function equalUint8Arrays(left: Uint8Array, right: Uint8Array): boolean {
  if (left.length !== right.length) return false;
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) return false;
  }
  return true;
}
