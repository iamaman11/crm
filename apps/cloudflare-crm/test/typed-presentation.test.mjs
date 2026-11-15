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

import {enumValue,boundedInt} from "../web/src/enumFields.ts";
import {parseRoute,accountsHref,accountHref,accountCreateHref,partyHref,scopeHref,accountFilterStatus} from "../web/src/routes.ts";

test("HTML enum boundary accepts only declared generated-style values",()=>{
  assert.equal(enumValue("1",[1,2],"status"),1);
  assert.equal(enumValue("2",[1,2],"status"),2);
  for(const raw of ["","0","3","1.0","01","NaN","1e0"])
    assert.throws(()=>enumValue(raw,[1,2],"status"),/Недопустимое значение/);
});
test("bounded numeric input rejects malformed and overflow values",()=>{
  assert.equal(boundedInt("65535",1,65535,"stage"),65535);
  for(const raw of ["","0","65536","-1","1.1","NaN","Infinity","1e2"," 1","1 "])
    assert.throws(()=>boundedInt(raw,1,65535,"stage"),/Некорректное число/);
});
test("typed CRM routes preserve filters and deep links without a second router",()=>{
  const allowed=[1,2]; // Test fixture: production passes generated enum members.
  const route=hash=>parseRoute(hash,allowed);
  const hash=accountsHref({search:"A&B",status:2,cursor:"opaque+cursor"});
  assert.deepEqual(route(hash),{kind:"accounts",search:"A&B",status:2,cursor:"opaque+cursor"});
  assert.deepEqual(route(accountHref("id-123")),{kind:"account",accountId:"id-123"});
  assert.deepEqual(route("#accounts"),{kind:"accounts",search:"",status:null,cursor:""});
  for(const invalid of ["0","3","01","1.0","NaN","1e0","UNSPECIFIED"])
    assert.equal(route("#accounts?status="+invalid).kind,"not-found",invalid);
  assert.equal(accountFilterStatus("2",allowed),2);
  assert.equal(accountFilterStatus("",allowed),null);
  assert.throws(()=>accountFilterStatus("3",allowed),/Недопустимый статус/);
  assert.equal(route("#accounts/%ZZ").kind,"not-found");
  assert.equal(route("#accounts/%2F").kind,"not-found");
  assert.equal(route("#accounts/").kind,"not-found");
  assert.deepEqual(route(scopeHref("contacts","party?1")),{kind:"section",screen:"contacts",partyId:"party?1",accountId:""});
  assert.deepEqual(route(scopeHref("sales","account&2")),{kind:"section",screen:"sales",partyId:"",accountId:"account&2"});
  assert.equal(route("#not-a-screen").kind,"not-found");
});

test("Party-to-Account intent is typed, URL-addressable and rejects invalid source parties",()=>{
  const allowed=[1,2];
  const id="87e4c6bf-5885-4591-83d9-0de3ac25a802";
  const source=partyHref(id);
  const creation=accountCreateHref(id);
  assert.equal(source,"#parties?partyId="+id);
  assert.deepEqual(parseRoute(source,allowed),
    {kind:"section",screen:"parties",partyId:id,accountId:""});
  assert.deepEqual(parseRoute(creation,allowed),{kind:"account-create",partyId:id});
  for(const invalid of ["#accounts/new","#accounts/new?partyId=",
    "#accounts/new?partyId=x%2Fy","#accounts/new?partyId=%25ZZ",
    "#accounts/new?partyId=x&partyId=y","#accounts/new?partyId="+("x".repeat(129))])
    assert.equal(parseRoute(invalid,allowed).kind,"not-found",invalid);
});
