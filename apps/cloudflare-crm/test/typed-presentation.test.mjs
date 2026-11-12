import test from "node:test";
import assert from "node:assert/strict";
import {minorUnits,decimalUnits,basisPoints} from "../web/src/money.ts";

test("money preserves int64 above JavaScript safe integer",()=>{
  const units=9007199254740993n;
  assert.equal(minorUnits("90071992547409.93","USD"),units);
  assert.equal(decimalUnits(units,"USD"),"90071992547409.93");
  assert.equal(minorUnits("92233720368547758.07","USD"),9223372036854775807n);
  assert.throws(()=>minorUnits("92233720368547758.08","USD"),/диапазон/);
});
test("currency exponent controls decimal input without rounding",()=>{
  assert.equal(minorUnits("123","JPY"),123n);
  assert.equal(decimalUnits(123n,"JPY"),"123");
  assert.equal(minorUnits("123.456","KWD"),123456n);
  assert.equal(decimalUnits(123456n,"KWD"),"123.456");
  assert.throws(()=>minorUnits("1.1","JPY"),/знаков/);
  assert.throws(()=>minorUnits("1.001","USD"),/знаков/);
  for(const text of ["1e3","NaN","Infinity","-1","1.2.3"])assert.throws(()=>minorUnits(text,"USD"));
});
test("percent input preserves basis points and rejects overflow",()=>{
  assert.equal(basisPoints("37,45"),3745);
  assert.equal(basisPoints("100"),10000);
  assert.throws(()=>basisPoints("100.01"));
  assert.throws(()=>basisPoints("0.001"));
});
