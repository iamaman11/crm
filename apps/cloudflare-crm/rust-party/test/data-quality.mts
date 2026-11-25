import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { DataQualityService } from "../../../../packages/client/gen/crm/data_quality/v2/quality_pb.js";
import { PartyKind, PartyService } from "../../../../packages/client/gen/crm/parties/v3/party_pb.js";
import { fixtureRpc, sendFixtureRpc } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type { RpcDispatch } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";

export async function verifyPartyNameQuality(
  dispatch: RpcDispatch,
  sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>,
  partyId: string,
  admin: string,
  viewer: string,
  otherTenant: string,
) {
  const method = DataQualityService.method.evaluatePartyName;
  await sendFixtureRpc(dispatch, method, null, { partyId }, 401);
  await sendFixtureRpc(dispatch, method, otherTenant, { partyId }, 404);
  await sendFixtureRpc(dispatch, method, viewer, { partyId: "" }, 400);
  await sendFixtureRpc(dispatch, method, viewer, { partyId: "missing-party" }, 404);

  const { body: quality } = await fixtureRpc(dispatch, method, viewer, { partyId });
  assert.equal(quality.party?.partyId, partyId);
  assert.ok(quality.ruleSetVersion);
  assert.equal(quality.checks.length, 2);
  assert.equal(quality.failedCount, 0);
  assert.ok(quality.checks.every(check => check.passed));

  const shortPartyId = "quality-short-" + randomUUID();
  await fixtureRpc(dispatch, PartyService.method.createParty, admin, {
    partyId: shortPartyId,
    kind: PartyKind.PERSON,
    displayName: "A",
    precondition: { idempotencyKey: "quality-short-create" },
  });
  const { body: poorQuality } = await fixtureRpc(dispatch, method, viewer, { partyId: shortPartyId });
  assert.equal(poorQuality.failedCount, 1);
  assert.equal(poorQuality.checks.filter(check => !check.passed).length, 1);
  const [audit] = await sql(
    "SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id=? AND subject_id=?", "tenant-a", shortPartyId,
  );
  assert.equal(audit?.n, 1, "quality read must not append audit or finding");
  console.log(JSON.stringify({
    event: "crm.native-g3-data-quality-v2",
    result: "PASS",
    rpc_methods: [method.name],
    generated_binary: true,
    original_owner: "crm.data-quality",
    tenant_ab: true,
    viewer_read: true,
    no_duplicate_rules: true,
    no_writes: true,
    published_active: true,
  }));
}
