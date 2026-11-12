import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {validateWorkerConfig} from "../scripts/architecture-gate.mjs";
import {amountToMinorUnits,amountFromMinorUnits,formatDealMoney,
 percentToBasisPoints,percentFromBasisPoints} from "../web/src/deal-presentation.mjs";
const config=JSON.parse(readFileSync(new URL("../wrangler.jsonc",import.meta.url),"utf8"));
const local=JSON.parse(readFileSync(new URL("../rust-party/wrangler.jsonc",import.meta.url),"utf8"));
test("canonical Worker configuration has exactly one D1 and matching local routing",()=>{
 assert.doesNotThrow(()=>validateWorkerConfig(config,local));
});
test("second D1 and mismatched routing are rejected",()=>{
 assert.throws(()=>validateWorkerConfig({...config,d1_databases:[...config.d1_databases,...config.d1_databases]},local),/exactly one native D1/);
 assert.throws(()=>validateWorkerConfig(config,{...local,assets:{...local.assets,run_worker_first:["/api/*"]}}),/local Worker-first/);
});
test("wrong account or database cannot be a release target",()=>{
 assert.throws(()=>validateWorkerConfig({...config,account_id:"another-account"},local),/unexpected Cloudflare account/);
 assert.throws(()=>validateWorkerConfig({...config,d1_databases:[{...config.d1_databases[0],database_id:"another-database"}]},local),/unexpected D1/);
});
test("Deal UI converts major amounts to exact Rust minor units without floating point",()=>{
  assert.equal(amountToMinorUnits("127,50"),12750);
  assert.equal(amountToMinorUnits("127.50"),12750);
  assert.equal(amountToMinorUnits("0"),0);
  assert.equal(amountToMinorUnits("0,05"),5);
  assert.equal(amountToMinorUnits("90071992547409.91"),Number.MAX_SAFE_INTEGER);
  for(const bad of ["","12.345","-1.00","1,2,3","1.2e3","01.10","90071992547409.92"]){
    assert.throws(()=>amountToMinorUnits(bad),"must reject "+bad);
  }
  assert.equal(amountFromMinorUnits("12750"),"127.50");
  assert.equal(amountFromMinorUnits("0"),"0.00");
  assert.equal(amountFromMinorUnits("-5"),"-0.05");
  assert.equal(formatDealMoney({minor_units:"12750",currency:"USD"}),"127,50 USD");
  assert.equal(formatDealMoney({minor_units:"123456789",currency:"PLN"}),"1\u00a0234\u00a0567,89 PLN");
  assert.equal(formatDealMoney(null),"—");
});
test("Deal UI shows percent, sends exact basis points and rejects invalid values",()=>{
  assert.equal(percentToBasisPoints("37,5"),3750);
  assert.equal(percentToBasisPoints("12.34"),1234);
  assert.equal(percentToBasisPoints("100"),10000);
  assert.equal(percentToBasisPoints("0.01"),1);
  assert.equal(percentFromBasisPoints(2500),"25");
  assert.equal(percentFromBasisPoints(1234),"12.34");
  assert.equal(percentFromBasisPoints(1),"0.01");
  for(const bad of ["","100,01","-1","12,345","101","1e2","25%"]){
    assert.throws(()=>percentToBasisPoints(bad),"must reject "+bad);
  }
});
