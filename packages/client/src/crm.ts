import { type Client } from "@connectrpc/connect";
import { create, fromBinary, toBinary, type DescMessage, type MessageShape } from "@bufbuild/protobuf";
import { ApplicationGatewayService, TypedPayloadSchema } from "../gen/crm/gateway/v1/gateway_pb";
import {
  CreatePartyRequestSchema, CreatePartyResponseSchema, UpdatePartyRequestSchema, UpdatePartyResponseSchema,
  ListPartiesRequestSchema, ListPartiesResponseSchema, GetPartyRequestSchema, GetPartyResponseSchema,
  type Party, PartyKind,
} from "../gen/crm/parties/v1/party_pb";
import {
  CreateAccountRequestSchema, CreateAccountResponseSchema,
  ListAccountsRequestSchema, ListAccountsResponseSchema, AccountPartyRole,
  AccountPartyAssociationSchema, type Account,
} from "../gen/crm/accounts/v1/accounts_pb";
import {
  CreateContactPointRequestSchema, CreateContactPointResponseSchema,
  UpdateContactPointRequestSchema, UpdateContactPointResponseSchema,
  ListContactPointsRequestSchema, ListContactPointsResponseSchema,
  ContactPointKind, ContactPointStatus, type ContactPoint,
} from "../gen/crm/contact_points/v1/contact_points_pb";
import {
  CreateDealRequestSchema, CreateDealResponseSchema, UpdateDealRequestSchema, UpdateDealResponseSchema,
  AdvanceStageRequestSchema, AdvanceStageResponseSchema,
  ListDealsRequestSchema, ListDealsResponseSchema, GetDealRequestSchema, GetDealResponseSchema, type Deal, DealStatus, DealStageSchema,
} from "../gen/crm/sales/v1/deal_pb";
import {
  CreateTaskRequestSchema, CreateTaskResponseSchema, UpdateTaskRequestSchema, UpdateTaskResponseSchema,
  CompleteTaskRequestSchema, CompleteTaskResponseSchema,
  ListTasksRequestSchema, ListTasksResponseSchema, GetTaskRequestSchema, GetTaskResponseSchema, type Task, TaskPriority,
} from "../gen/crm/activities/v1/task_pb";
import { AccountRefSchema, ContactPointRefSchema, PartyRefSchema } from "../gen/crm/customer/v1/reference_pb";
import {
  ActorOrTeamOwnerSchema, PageRequestSchema, ResourceRefSchema, StringPatchSchema,
} from "../gen/crm/core/v1/common_pb";
import { CONTRACT_HASHES } from "./contract_hashes";
import { ProductClientError, mapGatewayError, type GovernedGatewayClientOptions } from "./gateway";
import { requireAuthenticatedSession, type SessionProvider } from "./session";
import { createApplicationGatewayClient } from "./transport";

export type { Party, Account, ContactPoint, Deal, Task };
export { PartyKind, ContactPointKind, DealStatus, TaskPriority };
export interface Page<T> { items: T[]; next: string; }

type Contract<I extends DescMessage, O extends DescMessage> = {
  owner: string;
  capability: string;
  version?: string;
  inputName: string;
  input: I;
  outputName: string;
  output: O;
  dataClass: "personal" | "confidential";
};

function contract<I extends DescMessage, O extends DescMessage>(
  owner: string, capability: string, inputName: string, input: I,
  outputName: string, output: O, dataClass: "personal" | "confidential",
  version = "1.0.0",
): Contract<I, O> {
  return { owner, capability, version, inputName, input, outputName, output, dataClass };
}

const parties = <I extends DescMessage, O extends DescMessage>(name: string, input: I, output: O) =>
  contract("crm.parties", "parties.party." + name, "crm.parties.v1." + input.name, input,
    "crm.parties.v1." + output.name, output, "personal");
const accounts = <I extends DescMessage, O extends DescMessage>(name: string, input: I, output: O) =>
  contract("crm.customer-accounts", "accounts.account." + name,
    "crm.accounts.v1." + input.name, input, "crm.accounts.v1." + output.name, output, "personal");
const contacts = <I extends DescMessage, O extends DescMessage>(name: string, input: I, output: O) =>
  contract("crm.contact-points", "contact-points.contact-point." + name,
    "crm.contact_points.v1." + input.name, input, "crm.contact_points.v1." + output.name, output, "personal");
const deals = <I extends DescMessage, O extends DescMessage>(name: string, input: I, output: O) =>
  contract("crm.sales", "sales.deal." + name, "crm.sales.v1." + input.name, input,
    "crm.sales.v1." + output.name, output, "confidential");
const tasks = <I extends DescMessage, O extends DescMessage>(name: string, input: I, output: O, version = "1.0.0") =>
  contract("crm.activities", "activities.task." + name, "crm.activities.v1." + input.name, input,
    "crm.activities.v1." + output.name, output, "confidential", version);

function id(value: string): string {
  const normalized = value.trim();
  if (!normalized || normalized.length > 128) {
    throw new ProductClientError({ kind: "invalid_argument", message: "A valid record reference is required.", retryable: false });
  }
  return normalized;
}
function name(value: string): string {
  const normalized = value.trim();
  if (!normalized || normalized.length > 200) {
    throw new ProductClientError({ kind: "invalid_argument", message: "Enter a name (1–200 characters).", retryable: false });
  }
  return normalized;
}
function hash(schema: string): Uint8Array {
  const value = CONTRACT_HASHES[schema];
  if (!value) throw new ProductClientError({ kind: "internal", message: "Published CRM contract is unavailable.", retryable: false });
  return value;
}
function contractError(): ProductClientError {
  return new ProductClientError({ kind: "internal", message: "The CRM response contract is incompatible. Please contact support.", retryable: false });
}
function sameBytes(a: Uint8Array, b: Uint8Array): boolean {
  return a.length === b.length && a.every((value, index) => value === b[index]);
}
function page(cursor = "") {
  return create(PageRequestSchema, { pageSize: 50, pageToken: cursor });
}

export class GovernedCrmClient {
  private readonly gateway: Client<typeof ApplicationGatewayService>;
  private readonly session: SessionProvider;
  constructor(options: GovernedGatewayClientOptions) {
    this.gateway = createApplicationGatewayClient(options);
    this.session = options.sessionProvider;
  }
  private async call<I extends DescMessage, O extends DescMessage>(
    entry: Contract<I, O>, request: MessageShape<I>, mutation = false, key?: string,
  ): Promise<MessageShape<O>> {
    try {
      requireAuthenticatedSession(this.session.getSnapshot());
      const bytes = toBinary(entry.input, request);
      if (bytes.length > 1048576) throw contractError();
      const input = create(TypedPayloadSchema, {
        ownerModuleId: entry.owner,
        schemaId: entry.inputName,
        schemaVersion: "1.0.0",
        descriptorHash: hash(entry.inputName),
        dataClass: entry.dataClass,
        encoding: "protobuf",
        maximumSizeBytes: 1048576n,
        retentionPolicyId: "standard",
        payload: bytes,
      });
      const requestOptions = {
        ownerModuleId: entry.owner,
        capabilityId: entry.capability,
        capabilityVersion: entry.version ?? "1.0.0",
        input,
      };
      // Mutation metadata is mandatory for the existing governed ingress.
      // Caller-supplied keys let the same form submission be retried safely.
      const result = mutation
        ? await this.gateway.mutate(requestOptions, { headers: { "idempotency-key": key ?? crypto.randomUUID() } })
        : await this.gateway.query(requestOptions);
      const output = result.output;
      if (!output || output.ownerModuleId !== entry.owner || output.schemaId !== entry.outputName ||
          output.schemaVersion !== "1.0.0" || output.dataClass !== entry.dataClass ||
          output.encoding !== "protobuf" || output.maximumSizeBytes !== 1048576n ||
          output.retentionPolicyId !== "standard" ||
          !sameBytes(output.descriptorHash, hash(entry.outputName)) ||
          BigInt(output.payload.length) > output.maximumSizeBytes) throw contractError();
      return fromBinary(entry.output, output.payload);
    } catch (error) {
      throw mapGatewayError(error);
    }
  }

  async listParties(cursor = ""): Promise<Page<Party>> {
    const result = await this.call(parties("list", ListPartiesRequestSchema, ListPartiesResponseSchema),
      create(ListPartiesRequestSchema, { page: page(cursor) }));
    return { items: result.parties, next: result.page?.nextPageToken ?? "" };
  }
  async getParty(partyId: string): Promise<Party | undefined> {
    const result = await this.call(parties("get", GetPartyRequestSchema, GetPartyResponseSchema),
      create(GetPartyRequestSchema, { partyRef: create(PartyRefSchema, { partyId: id(partyId) }) }));
    return result.party;
  }
  async createParty(partyId: string, kind: PartyKind, displayName: string): Promise<Party | undefined> {
    const result = await this.call(parties("create", CreatePartyRequestSchema, CreatePartyResponseSchema),
      create(CreatePartyRequestSchema, {
        partyRef: create(PartyRefSchema, { partyId: id(partyId) }), kind, displayName: name(displayName),
      }), true, partyId);
    return result.party;
  }
  async updateParty(party: Party, displayName: string): Promise<Party | undefined> {
    const result = await this.call(parties("update", UpdatePartyRequestSchema, UpdatePartyResponseSchema),
      create(UpdatePartyRequestSchema, {
        partyRef: party.partyRef, expectedVersion: party.resourceVersion?.version ?? 0n, displayName: name(displayName),
      }), true);
    return result.party;
  }
  async listAccounts(cursor = ""): Promise<Page<Account>> {
    const result = await this.call(accounts("list", ListAccountsRequestSchema, ListAccountsResponseSchema),
      create(ListAccountsRequestSchema, { page: page(cursor) }));
    return { items: result.accounts, next: result.page?.nextPageToken ?? "" };
  }
  async createAccount(accountId: string, partyId: string, displayName: string): Promise<Account | undefined> {
    const result = await this.call(accounts("create", CreateAccountRequestSchema, CreateAccountResponseSchema),
      create(CreateAccountRequestSchema, {
        accountRef: create(AccountRefSchema, { accountId: id(accountId) }),
        name: name(displayName),
        partyAssociations: [create(AccountPartyAssociationSchema, {
          partyRef: create(PartyRefSchema, { partyId: id(partyId) }), role: AccountPartyRole.PRIMARY,
        })],
      }), true, accountId);
    return result.account;
  }
  async listContacts(partyId: string, cursor = ""): Promise<Page<ContactPoint>> {
    const result = await this.call(contacts("list", ListContactPointsRequestSchema, ListContactPointsResponseSchema),
      create(ListContactPointsRequestSchema, {
        page: page(cursor), partyRef: create(PartyRefSchema, { partyId: id(partyId) }),
      }));
    return { items: result.contactPoints, next: result.page?.nextPageToken ?? "" };
  }
  async createContact(partyId: string, contactId: string, kind: ContactPointKind, value: string): Promise<ContactPoint | undefined> {
    const result = await this.call(contacts("create", CreateContactPointRequestSchema, CreateContactPointResponseSchema),
      create(CreateContactPointRequestSchema, {
        contactPointRef: create(ContactPointRefSchema, { contactPointId: id(contactId) }),
        partyRef: create(PartyRefSchema, { partyId: id(partyId) }),
        kind, value: name(value),
      }), true, contactId);
    return result.contactPoint;
  }
  async updateContact(contact: ContactPoint, value: string): Promise<ContactPoint | undefined> {
    const result = await this.call(contacts("update", UpdateContactPointRequestSchema, UpdateContactPointResponseSchema),
      create(UpdateContactPointRequestSchema, {
        contactPointRef: contact.contactPointRef,
        expectedVersion: contact.resourceVersion?.version ?? 0n,
        value: name(value), status: contact.status || ContactPointStatus.ACTIVE, preferred: contact.preferred,
        validFrom: contact.validFrom, validUntil: contact.validUntil,
      }), true);
    return result.contactPoint;
  }
  async listDeals(cursor = ""): Promise<Page<Deal>> {
    const result = await this.call(deals("list", ListDealsRequestSchema, ListDealsResponseSchema),
      create(ListDealsRequestSchema, { page: page(cursor) }));
    return { items: result.deals, next: result.page?.nextPageToken ?? "" };
  }
  async getDeal(dealId: string): Promise<Deal | undefined> {
    const result = await this.call(deals("get", GetDealRequestSchema, GetDealResponseSchema),
      create(GetDealRequestSchema, { dealId: id(dealId) }));
    return result.deal;
  }
  async createDeal(dealId: string, title: string, actorId: string, tenantId: string, accountId: string): Promise<Deal | undefined> {
    const result = await this.call(deals("create", CreateDealRequestSchema, CreateDealResponseSchema),
      create(CreateDealRequestSchema, {
        dealId: id(dealId), name: name(title),
        owner: create(ActorOrTeamOwnerSchema, { owner: { case: "actorId", value: id(actorId) } }),
        account: create(ResourceRefSchema, { tenantId: id(tenantId), resourceType: "accounts.account", resourceId: id(accountId) }),
        stage: create(DealStageSchema, { pipelineId: "default", stageId: "new", ordinal: 1 }),
      }), true, dealId);
    return result.deal;
  }
  async updateDeal(deal: Deal, title: string): Promise<Deal | undefined> {
    const result = await this.call(deals("update", UpdateDealRequestSchema, UpdateDealResponseSchema),
      create(UpdateDealRequestSchema, {
        dealId: id(deal.dealId), expectedVersion: deal.version,
        name: create(StringPatchSchema, { operation: { case: "set", value: name(title) } }),
      }), true);
    return result.deal;
  }
  async advanceDeal(deal: Deal, stage: "qualified" | "proposal" | "won"): Promise<Deal | undefined> {
    const ordinal = { qualified: 2, proposal: 3, won: 4 }[stage];
    const result = await this.call(deals("advance_stage", AdvanceStageRequestSchema, AdvanceStageResponseSchema),
      create(AdvanceStageRequestSchema, {
        dealId: id(deal.dealId), expectedVersion: deal.version,
        targetStage: create(DealStageSchema, { pipelineId: "default", stageId: stage, ordinal }),
        targetStatus: stage === "won" ? DealStatus.WON : DealStatus.OPEN,
        ...(stage === "won" ? { closeReasonCode: "successfully_closed" } : {}),
      }), true);
    return result.deal;
  }
  async listTasks(cursor = ""): Promise<Page<Task>> {
    const result = await this.call(tasks("list", ListTasksRequestSchema, ListTasksResponseSchema),
      create(ListTasksRequestSchema, { page: page(cursor) }));
    return { items: result.tasks, next: result.page?.nextPageToken ?? "" };
  }
  async getTask(taskId: string): Promise<Task | undefined> {
    const result = await this.call(tasks("get", GetTaskRequestSchema, GetTaskResponseSchema),
      create(GetTaskRequestSchema, { taskId: id(taskId) }));
    return result.task;
  }
  async createTask(taskId: string, subject: string, actorId: string, tenantId: string,
    relatedType: "sales.deal" | "parties.party", relatedId: string): Promise<Task | undefined> {
    const result = await this.call(tasks("create", CreateTaskRequestSchema, CreateTaskResponseSchema, "1.1.0"),
      create(CreateTaskRequestSchema, {
        taskId: id(taskId), subject: name(subject),
        owner: create(ActorOrTeamOwnerSchema, { owner: { case: "actorId", value: id(actorId) } }),
        relatedResources: [create(ResourceRefSchema, {
          tenantId: id(tenantId), resourceType: relatedType, resourceId: id(relatedId),
        })],
        priority: TaskPriority.NORMAL,
      }), true, taskId);
    return result.task;
  }
  async updateTask(task: Task, subject: string): Promise<Task | undefined> {
    const result = await this.call(tasks("update", UpdateTaskRequestSchema, UpdateTaskResponseSchema),
      create(UpdateTaskRequestSchema, {
        taskId: id(task.taskId), expectedVersion: task.version,
        subject: create(StringPatchSchema, { operation: { case: "set", value: name(subject) } }),
      }), true);
    return result.task;
  }
  async completeTask(task: Task): Promise<Task | undefined> {
    const result = await this.call(tasks("complete", CompleteTaskRequestSchema, CompleteTaskResponseSchema),
      create(CompleteTaskRequestSchema, { taskId: id(task.taskId), expectedVersion: task.version }), true);
    return result.task;
  }
}
