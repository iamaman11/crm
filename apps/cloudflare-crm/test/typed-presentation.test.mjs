import test from "node:test";
import assert from "node:assert/strict";
import {currencyCatalog,minorUnits,decimalUnits,basisPoints,weightedMoneyText} from "../web/src/money.ts";
import {inputDate,parseDate} from "../web/src/calendarInput.ts";
import {panelFailureCode} from "../rust-party/test/ui-diagnostics.mjs";

const currencies=currencyCatalog([
  {currencyCode:"USD",minorUnitExponent:2},
  {currencyCode:"JPY",minorUnitExponent:0},
  {currencyCode:"KWD",minorUnitExponent:3},
  {currencyCode:"CLF",minorUnitExponent:4},
]);
test("money preserves int64 above JavaScript safe integer",()=>{
  const units=9007199254740993n;
  assert.equal(minorUnits("90071992547409.93","USD",currencies),units);
  assert.equal(decimalUnits(units,"USD",currencies),"90071992547409.93");
  assert.equal(minorUnits("92233720368547758.07","USD",currencies),9223372036854775807n);
  assert.throws(()=>minorUnits("92233720368547758.08","USD",currencies),/диапазон/);
});
test("currency exponent controls decimal input without rounding",()=>{
  assert.equal(minorUnits("123","JPY",currencies),123n);
  assert.equal(decimalUnits(123n,"JPY",currencies),"123");
  assert.equal(minorUnits("123.456","KWD",currencies),123456n);
  assert.equal(decimalUnits(123456n,"KWD",currencies),"123.456");
  assert.throws(()=>minorUnits("1.1","JPY",currencies),/знаков/);
  assert.throws(()=>minorUnits("1.001","USD",currencies),/знаков/);
  for(const text of ["1e3","NaN","Infinity","-1","1.2.3"])assert.throws(()=>minorUnits(text,"USD",currencies));
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
  assert.equal(route("#tasks?accountId=a&accountId=b").kind,"not-found");
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


test("UI panel diagnostics distinguish native desync, guard rejection and timeout",()=>{
  const panel=(desired,nativeOpen)=>({desired,nativeOpen});
  assert.equal(panelFailureCode({panels:[],expected:"closed"}),"CRM_UI_PANEL_NOT_FOUND");
  assert.equal(panelFailureCode({panels:[panel("open",false)],expected:"open"}),"CRM_UI_PANEL_NATIVE_DESYNC");
  assert.equal(panelFailureCode({panels:[panel("open",true)],expected:"closed"}),"CRM_UI_PANEL_TRANSITION_REJECTED");
  assert.equal(panelFailureCode({panels:[panel("closed",false)],expected:"closed"}),"CRM_UI_PANEL_TIMEOUT");
  assert.equal(panelFailureCode({panels:[panel("closed",true)],expected:"closed"}),"CRM_UI_PANEL_NATIVE_DESYNC");
});

test("calendar input preserves date-only wire values and delegates civil validity",()=>{
  assert.equal(parseDate(""),undefined);
  assert.equal(inputDate(undefined),"");
  for(const raw of ["0001-01-01","2024-02-29","9999-12-31"])
    assert.equal(inputDate(parseDate(raw)),raw);
  // Negative domain fixtures must reach the Rust validator, not another calendar.
  for(const raw of ["0000-01-01","2100-02-29","2024-04-31","2024-13-00"])
    assert.equal(inputDate(parseDate(raw)),raw);
  for(const raw of ["2024-2-09","2024-02-9","2024-02-29T00:00:00Z","2024/02/29"," 2024-02-29","10000-01-01"])
    assert.throws(()=>parseDate(raw),/Неверная дата/);
});

test("currency precision is an explicit server projection, never an Intl fallback",()=>{
  assert.equal(minorUnits("1.2345","CLF",currencies),12345n);
  assert.equal(decimalUnits(12345n,"CLF",currencies),"1.2345");
  assert.equal(weightedMoneyText("90071992547409930000","USD",currencies),"90071992547409.930000 USD");
  assert.throws(()=>minorUnits("1.00","ZZZ",currencies),/Неизвестная точность/);
  assert.throws(()=>decimalUnits(1n,"EUR",currencies),/Неизвестная точность/);
  assert.throws(()=>currencyCatalog([]),/не получен/);
  assert.throws(()=>currencyCatalog([{currencyCode:"JPY"}]),/Некорректный/);
  assert.throws(()=>currencyCatalog([{currencyCode:"JPY",minorUnitExponent:0},{currencyCode:"JPY",minorUnitExponent:2}]),/Некорректный/);
});
