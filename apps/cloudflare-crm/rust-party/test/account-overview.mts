import assert from "node:assert/strict";
import { Customer360Service } from "../../../../packages/client/gen/crm/customer_360/v2/customer_360_pb.js";
import { PartyService } from "../../../../packages/client/gen/crm/parties/v3/party_pb.js";
import { ContactPointKind, ContactPointService } from "../../../../packages/client/gen/crm/contact_points/v2/contact_point_pb.js";
import { PartyRelationshipService } from "../../../../packages/client/gen/crm/party_relationships/v2/relationship_pb.js";
import { DealService } from "../../../../packages/client/gen/crm/sales/v2/deal_pb.js";
import { TaskService, TaskStatus } from "../../../../packages/client/gen/crm/activities/v2/task_pb.js";
import { collectFixturePages, fixtureRpc, sendFixtureRpc } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type { RpcDispatch } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { record, string } from "../../scripts/json-boundary.mts";

type OverviewFixture = {
  dispatch: RpcDispatch;
  sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
  admin: string;
  viewer: string;
  tenantB: string;
  accountId: string;
  partyId: string;
  contactId: string;
  relationshipId: string;
  dealId: string;
  taskId: string;
  otherTaskId: string;
};

export async function verifyAccountOverview({
  dispatch, sql, admin, viewer, tenantB, accountId, partyId, contactId,
  relationshipId, dealId, taskId, otherTaskId,
}: OverviewFixture) {
  const method = Customer360Service.method.getAccountOverview;
  const auditCount = async () => {
    const [row] = await sql("SELECT COUNT(*) AS n FROM crm_audit");
    assert.ok(typeof row?.n === "number");
    return row.n;
  };
  const auditBefore = await auditCount();
  const crossTenant = await sendFixtureRpc(dispatch, method, tenantB, {accountId}, 404);
  assert.equal(record(await crossTenant.json()).code, "not_found");
  const badId = await sendFixtureRpc(dispatch, method, viewer, {accountId: "bad/id"}, 400);
  assert.equal(record(await badId.json()).code, "invalid_argument");
  const missing = await sendFixtureRpc(dispatch, method, viewer, {accountId: "missing-account"}, 404);
  assert.equal(record(await missing.json()).code, "not_found");

  const {body: {overview: card}} = await fixtureRpc(dispatch, method, viewer, {accountId});
  assert.ok(card && card.truncated);
  assert.equal(card.account?.accountId, accountId);
  assert.equal(card.primaryParty?.partyId, partyId);
  assert.equal(card.perCollectionLimit, 100);
  assert.equal(card.truncated.contactPoints, false);
  assert.equal(card.truncated.relationships, false);
  assert.equal(card.truncated.deals, false);
  assert.equal(card.truncated.tasks, false);
  const {body: {party}} = await fixtureRpc(dispatch, PartyService.method.getParty, viewer, {partyId});
  assert.deepEqual(card.primaryParty, party);
  assert.deepEqual(card.contactPoints, await collectFixturePages(async pageToken => {
    const {body} = await fixtureRpc(dispatch, ContactPointService.method.listContactPoints, viewer, {
      partyId, page: {pageSize: 1, pageToken},
    });
    return {items: body.contactPoints, nextPageToken: body.pageInfo?.nextPageToken};
  }));
  assert.deepEqual(card.relationships, await collectFixturePages(async pageToken => {
    const {body} = await fixtureRpc(dispatch, PartyRelationshipService.method.listPartyRelationships, viewer, {
      partyId, page: {pageSize: 1, pageToken},
    });
    return {items: body.relationships, nextPageToken: body.pageInfo?.nextPageToken};
  }));
  const deals = await collectFixturePages(async pageToken => {
    const {body} = await fixtureRpc(dispatch, DealService.method.listDeals, viewer, {
      page: {pageSize: 1, pageToken},
    });
    return {items: body.deals, nextPageToken: body.pageInfo?.nextPageToken};
  });
  assert.deepEqual(card.deals, deals.filter(deal => deal.accountId === accountId));
  assert.deepEqual(card.tasks, await collectFixturePages(async pageToken => {
    const {body} = await fixtureRpc(dispatch, TaskService.method.listTasks, viewer, {
      accountId, page: {pageSize: 1, pageToken},
    });
    return {items: body.tasks, nextPageToken: body.pageInfo?.nextPageToken};
  }));
  assert.ok(card.contactPoints.some(item => item.contactPointId === contactId));
  assert.ok(card.relationships.some(item => item.relationshipId === relationshipId));
  assert.ok(card.deals.some(item => item.dealId === dealId));
  assert.ok(card.tasks.some(item => item.taskId === taskId));
  assert.ok(card.tasks.every(item => item.taskId !== otherTaskId));
  assert.equal(await auditCount(), auditBefore);

  const [linked] = await sql(
    "SELECT task_id FROM crm_sales_link_delivery_records WHERE tenant=? AND deal_id=? " +
    "AND task_id IS NOT NULL ORDER BY task_id LIMIT 1", "tenant-a", dealId,
  );
  const linkedTaskId = string(linked?.task_id);
  const {body: {task: linkedTask}} = await fixtureRpc(dispatch, TaskService.method.getTask, viewer, {taskId: linkedTaskId});
  assert.ok(linkedTask);
  assert.equal(linkedTask.accountId, accountId);
  assert.ok(card.tasks.some(task => task.taskId === linkedTaskId));
  await sendFixtureRpc(dispatch, TaskService.method.getTask, tenantB, {taskId: linkedTaskId}, 404);
  const {body: {task: completed}} = await fixtureRpc(dispatch, TaskService.method.completeTask, admin, {
    taskId: linkedTaskId,
    precondition: {idempotencyKey: "overview-linked-task-complete", expectedVersion: linkedTask.version},
  });
  assert.equal(completed?.accountId, accountId);
  assert.equal(completed?.status, TaskStatus.COMPLETED);

  for (let index = 0; index < 101 - card.contactPoints.length; index++) {
    const suffix = String(index).padStart(3, "0");
    await fixtureRpc(dispatch, ContactPointService.method.createContactPoint, admin, {
      contactPointId: "overview-boundary-" + suffix, partyId, kind: ContactPointKind.EMAIL,
      value: "overview-" + index + "@example.org",
      precondition: {idempotencyKey: "overview-contact-" + suffix},
    });
  }
  const boundedAudit = await auditCount();
  const {body: {overview: bounded}} = await fixtureRpc(dispatch, method, viewer, {accountId});
  assert.ok(bounded && bounded.truncated);
  assert.equal(bounded.contactPoints.length, 100);
  assert.equal(bounded.truncated.contactPoints, true);
  assert.equal(bounded.truncated.tasks, false);
  assert.equal(await auditCount(), boundedAudit);
  console.log(JSON.stringify({event: "crm.native-account-overview-v2", result: "PASS",
    owner: "crm.customer360", account_rooted: true, original_owners: true,
    tenant_ab: true, read_only: true, bounded: true}));
  return {linkedTaskId};
}

export async function verifyOverviewReadParity(
  {dispatch,admin,viewer,tenantB,accountId}: Pick<OverviewFixture, "dispatch" | "admin" | "viewer" | "tenantB" | "accountId">,
) {
  const method = Customer360Service.method.getAccountOverview;
  const {body: fromAdmin} = await fixtureRpc(dispatch, method, admin, {accountId});
  const {body: fromViewer} = await fixtureRpc(dispatch, method, viewer, {accountId});
  assert.ok(fromAdmin.overview && fromViewer.overview);
  assert.deepEqual(fromViewer.overview, fromAdmin.overview, "All Customer360 business fields are visible to viewer");
  await sendFixtureRpc(dispatch, method, tenantB, {accountId}, 404);
  return fromViewer.overview;
}
