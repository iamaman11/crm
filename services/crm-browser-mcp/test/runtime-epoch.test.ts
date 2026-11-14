import test from "node:test";
import assert from "node:assert/strict";
import {
  BROWSER_RUNTIME_EPOCH, requireRuntimeEpoch, isVerifiedRuntimeProbe,
} from "../src/runtime-epoch.ts";

test("a compatible DO responds to a read-only runtime probe", () => {
  assert.equal(isVerifiedRuntimeProbe({ok:true,data:{epoch:BROWSER_RUNTIME_EPOCH}}),true);
  assert.doesNotThrow(()=>requireRuntimeEpoch(BROWSER_RUNTIME_EPOCH));
});

test("old, missing, malformed and unavailable DO epochs fail before any browser mutation", () => {
  for (const input of [
    null, undefined, {}, {ok:false,error:"CRM_BROWSER_UNKNOWN_OPERATION"},
    {ok:true}, {ok:true,data:{}},
    {ok:true,data:{epoch:"old-runtime"}}, {ok:true,data:{epoch:null}},
  ]) assert.equal(isVerifiedRuntimeProbe(input),false);
  for (const v of [undefined,"", "old-runtime"]) {
    assert.throws(()=>requireRuntimeEpoch(v), /BROWSER_RUNTIME_EPOCH_MISMATCH/);
  }
});
