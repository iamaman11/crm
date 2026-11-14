import test from "node:test";
import assert from "node:assert/strict";
import { selectWorkspaceId, workspaceObjectName } from "../src/browser-workspaces.ts";

const first = "2b446971-5936-4251-aef8-b86e4827b9b7";
const second = "3e944193-3e32-428e-a73d-b8cfc40ecb9d";

test("new open creates an independent workspace while reopen preserves the exact explicit handle", () => {
  assert.equal(selectWorkspaceId("open",undefined,()=>first),first);
  assert.equal(selectWorkspaceId("open",second),second);
  assert.equal(selectWorkspaceId("open",first,()=>{throw Error("should not allocate");}),first);
});

test("all later commands require a handle, with no implicit owner-wide fallback", () => {
  for (const op of ["status","close","run","assert","file","tabs","snapshot","session","wait","navigate"]) {
    assert.throws(()=>selectWorkspaceId(op,undefined),/BROWSER_WORKSPACE_REQUIRED/);
  }
  for (const bad of ["", "user@example.com", "../root", "abcd",
    "00000000-0000-0000-0000-000000000000"]) {
    assert.throws(()=>selectWorkspaceId("open",bad),/BROWSER_WORKSPACE_INVALID/);
  }
});

test("different owner+workspace tuples cannot map to the same DO", () => {
  const a=workspaceObjectName("Owner@Example.com ",first);
  const b=workspaceObjectName("owner@example.com",second);
  assert.notEqual(a,b);
  assert.equal(a,workspaceObjectName("owner@example.com",first));
  assert.notEqual(a,workspaceObjectName("other@example.com",first));
  assert.equal(a,workspaceObjectName("owner@example.com",first.toUpperCase()));
  assert.throws(()=>workspaceObjectName("",first),/BROWSER_WORKSPACE_INVALID/);
});

test("explicit handle restores an existing workspace; absent handle never shares it", () => {
  const id=selectWorkspaceId("open",undefined,()=>first);
  assert.equal(selectWorkspaceId("status",id),id);
  assert.equal(workspaceObjectName("owner@example.com",selectWorkspaceId("status",id)),
    workspaceObjectName("owner@example.com",id));
  assert.throws(()=>selectWorkspaceId("status",undefined),/WORKSPACE_REQUIRED/);
});
