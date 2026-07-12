import { type Client } from "@connectrpc/connect";
import {
  create,
  fromBinary,
  toBinary,
  type DescMessage,
  type MessageShape,
} from "@bufbuild/protobuf";
import { ApplicationGatewayService, TypedPayloadSchema } from "../gen/crm/gateway/v1/gateway_pb";
import {
  ActivateMetadataRevisionRequestSchema,
  ActivateMetadataRevisionResponseSchema,
  GetMetadataActivationRequestSchema,
  GetMetadataActivationResponseSchema,
  GetMetadataImpactRequestSchema,
  GetMetadataImpactResponseSchema,
  GetMetadataRevisionRequestSchema,
  GetMetadataRevisionResponseSchema,
  PublishMetadataBundleRequestSchema,
  PublishMetadataBundleResponseSchema,
  RollbackMetadataRevisionRequestSchema,
  RollbackMetadataRevisionResponseSchema,
  type ActivateMetadataRevisionResponse,
  type GetMetadataActivationResponse,
  type GetMetadataImpactResponse,
  type GetMetadataRevisionResponse,
  type MetadataDefinitionInput,
  type PublishMetadataBundleResponse,
  type RollbackMetadataRevisionResponse,
} from "../gen/crm/metadata/v1/metadata_pb";
import { CONTRACT_HASHES } from "./contract_hashes";
import {
  ProductClientError,
  mapGatewayError,
  type GovernedGatewayClientOptions,
} from "./gateway";
import {
  requireAuthenticatedSession,
  type SessionProvider,
} from "./session";
import { createApplicationGatewayClient } from "./transport";

const METADATA_OWNER = "crm.metadata";
const CONTRACT_VERSION = "1.0.0";
const MAX_PROTOBUF_BYTES = 1048576n;
const RETENTION_POLICY_ID = "standard";
const IDEMPOTENCY_HEADER = "idempotency-key";

export interface PublishMetadataBundleOptions {
  definitions: MetadataDefinitionInput[];
  idempotencyKey: string;
}

export interface ActivateMetadataRevisionOptions {
  revisionId: string;
  expectedGeneration: bigint;
  confirmBreakingChanges: boolean;
  idempotencyKey: string;
}

export interface RollbackMetadataRevisionOptions {
  expectedGeneration: bigint;
  idempotencyKey: string;
}

interface MetadataMutationContract<I extends DescMessage, O extends DescMessage> {
  capabilityId: string;
  inputSchemaId: string;
  inputSchema: I;
  outputSchemaId: string;
  outputSchema: O;
}

interface MetadataQueryContract<I extends DescMessage, O extends DescMessage> {
  capabilityId: string;
  inputSchemaId: string;
  inputSchema: I;
  outputSchemaId: string;
  outputSchema: O;
}

const PUBLISH_CONTRACT = {
  capabilityId: "metadata.bundle.publish",
  inputSchemaId: "crm.metadata.v1.PublishMetadataBundleRequest",
  inputSchema: PublishMetadataBundleRequestSchema,
  outputSchemaId: "crm.metadata.v1.PublishMetadataBundleResponse",
  outputSchema: PublishMetadataBundleResponseSchema,
} satisfies MetadataMutationContract<
  typeof PublishMetadataBundleRequestSchema,
  typeof PublishMetadataBundleResponseSchema
>;

const ACTIVATE_CONTRACT = {
  capabilityId: "metadata.revision.activate",
  inputSchemaId: "crm.metadata.v1.ActivateMetadataRevisionRequest",
  inputSchema: ActivateMetadataRevisionRequestSchema,
  outputSchemaId: "crm.metadata.v1.ActivateMetadataRevisionResponse",
  outputSchema: ActivateMetadataRevisionResponseSchema,
} satisfies MetadataMutationContract<
  typeof ActivateMetadataRevisionRequestSchema,
  typeof ActivateMetadataRevisionResponseSchema
>;

const ROLLBACK_CONTRACT = {
  capabilityId: "metadata.revision.rollback",
  inputSchemaId: "crm.metadata.v1.RollbackMetadataRevisionRequest",
  inputSchema: RollbackMetadataRevisionRequestSchema,
  outputSchemaId: "crm.metadata.v1.RollbackMetadataRevisionResponse",
  outputSchema: RollbackMetadataRevisionResponseSchema,
} satisfies MetadataMutationContract<
  typeof RollbackMetadataRevisionRequestSchema,
  typeof RollbackMetadataRevisionResponseSchema
>;

const IMPACT_CONTRACT = {
  capabilityId: "metadata.bundle.impact",
  inputSchemaId: "crm.metadata.v1.GetMetadataImpactRequest",
  inputSchema: GetMetadataImpactRequestSchema,
  outputSchemaId: "crm.metadata.v1.GetMetadataImpactResponse",
  outputSchema: GetMetadataImpactResponseSchema,
} satisfies MetadataQueryContract<
  typeof GetMetadataImpactRequestSchema,
  typeof GetMetadataImpactResponseSchema
>;

const REVISION_CONTRACT = {
  capabilityId: "metadata.revision.get",
  inputSchemaId: "crm.metadata.v1.GetMetadataRevisionRequest",
  inputSchema: GetMetadataRevisionRequestSchema,
  outputSchemaId: "crm.metadata.v1.GetMetadataRevisionResponse",
  outputSchema: GetMetadataRevisionResponseSchema,
} satisfies MetadataQueryContract<
  typeof GetMetadataRevisionRequestSchema,
  typeof GetMetadataRevisionResponseSchema
>;

const ACTIVATION_CONTRACT = {
  capabilityId: "metadata.activation.get",
  inputSchemaId: "crm.metadata.v1.GetMetadataActivationRequest",
  inputSchema: GetMetadataActivationRequestSchema,
  outputSchemaId: "crm.metadata.v1.GetMetadataActivationResponse",
  outputSchema: GetMetadataActivationResponseSchema,
} satisfies MetadataQueryContract<
  typeof GetMetadataActivationRequestSchema,
  typeof GetMetadataActivationResponseSchema
>;

export class GovernedMetadataClient {
  private readonly gatewayClient: Client<typeof ApplicationGatewayService>;
  private readonly sessionProvider: SessionProvider;

  public constructor(options: GovernedGatewayClientOptions) {
    this.sessionProvider = options.sessionProvider;
    this.gatewayClient = createApplicationGatewayClient(options);
  }

  public async publishBundle(
    options: PublishMetadataBundleOptions,
  ): Promise<PublishMetadataBundleResponse> {
    return await this.mutate(
      PUBLISH_CONTRACT,
      create(PublishMetadataBundleRequestSchema, {
        definitions: options.definitions,
      }),
      options.idempotencyKey,
    );
  }

  public async getImpact(candidateRevisionId: string): Promise<GetMetadataImpactResponse> {
    return await this.query(
      IMPACT_CONTRACT,
      create(GetMetadataImpactRequestSchema, { candidateRevisionId }),
    );
  }

  public async activateRevision(
    options: ActivateMetadataRevisionOptions,
  ): Promise<ActivateMetadataRevisionResponse> {
    return await this.mutate(
      ACTIVATE_CONTRACT,
      create(ActivateMetadataRevisionRequestSchema, {
        revisionId: options.revisionId,
        expectedGeneration: options.expectedGeneration,
        confirmBreakingChanges: options.confirmBreakingChanges,
      }),
      options.idempotencyKey,
    );
  }

  public async getRevision(revisionId: string): Promise<GetMetadataRevisionResponse> {
    return await this.query(
      REVISION_CONTRACT,
      create(GetMetadataRevisionRequestSchema, { revisionId }),
    );
  }

  public async getActivation(): Promise<GetMetadataActivationResponse> {
    return await this.query(
      ACTIVATION_CONTRACT,
      create(GetMetadataActivationRequestSchema, {}),
    );
  }

  public async rollbackRevision(
    options: RollbackMetadataRevisionOptions,
  ): Promise<RollbackMetadataRevisionResponse> {
    return await this.mutate(
      ROLLBACK_CONTRACT,
      create(RollbackMetadataRevisionRequestSchema, {
        expectedGeneration: options.expectedGeneration,
      }),
      options.idempotencyKey,
    );
  }

  private async mutate<I extends DescMessage, O extends DescMessage>(
    contract: MetadataMutationContract<I, O>,
    input: MessageShape<I>,
    idempotencyKey: string,
  ): Promise<MessageShape<O>> {
    try {
      requireAuthenticatedSession(this.sessionProvider.getSnapshot());
      requireIdempotencyKey(idempotencyKey);
      const response = await this.gatewayClient.mutate(
        {
          ownerModuleId: METADATA_OWNER,
          capabilityId: contract.capabilityId,
          capabilityVersion: CONTRACT_VERSION,
          input: createPayload(contract.inputSchemaId, contract.inputSchema, input),
        },
        {
          headers: new Headers({ [IDEMPOTENCY_HEADER]: idempotencyKey }),
        },
      );
      if (!response.output) {
        throw contractFailure("Gateway response did not contain an output payload.");
      }
      return decodePayload(contract.outputSchemaId, contract.outputSchema, response.output);
    } catch (error) {
      throw mapGatewayError(error);
    }
  }

  private async query<I extends DescMessage, O extends DescMessage>(
    contract: MetadataQueryContract<I, O>,
    input: MessageShape<I>,
  ): Promise<MessageShape<O>> {
    try {
      requireAuthenticatedSession(this.sessionProvider.getSnapshot());
      const response = await this.gatewayClient.query({
        ownerModuleId: METADATA_OWNER,
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
  const descriptorHash = requireDescriptorHash(schemaId);
  const payload = toBinary(schema, message);
  if (BigInt(payload.length) > MAX_PROTOBUF_BYTES) {
    throw new ProductClientError({
      kind: "invalid_argument",
      message: "The encoded metadata payload exceeds the permitted size.",
      retryable: false,
      safeCode: "METADATA_PROTOBUF_PAYLOAD_TOO_LARGE",
    });
  }
  return create(TypedPayloadSchema, {
    ownerModuleId: METADATA_OWNER,
    schemaId,
    schemaVersion: CONTRACT_VERSION,
    descriptorHash,
    dataClass: "confidential",
    encoding: "protobuf",
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
  const expectedHash = requireDescriptorHash(expectedSchemaId);
  if (
    output.ownerModuleId !== METADATA_OWNER ||
    output.schemaId !== expectedSchemaId ||
    output.schemaVersion !== CONTRACT_VERSION ||
    output.dataClass !== "confidential" ||
    output.encoding !== "protobuf" ||
    output.maximumSizeBytes !== MAX_PROTOBUF_BYTES ||
    output.retentionPolicyId !== RETENTION_POLICY_ID ||
    !equalUint8Arrays(output.descriptorHash, expectedHash) ||
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

function requireIdempotencyKey(value: string): void {
  if (value.trim().length === 0) {
    throw new ProductClientError({
      kind: "invalid_argument",
      message: "A non-empty idempotency key is required for metadata mutations.",
      retryable: false,
      safeCode: "IDEMPOTENCY_KEY_REQUIRED",
    });
  }
}

function contractFailure(message: string): ProductClientError {
  return new ProductClientError({
    kind: "internal",
    message,
    retryable: false,
  });
}

function equalUint8Arrays(left: Uint8Array, right: Uint8Array): boolean {
  if (left.length !== right.length) return false;
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) return false;
  }
  return true;
}
