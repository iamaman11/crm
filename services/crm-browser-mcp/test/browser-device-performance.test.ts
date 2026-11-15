import test from "node:test";
import assert from "node:assert/strict";
import { DEVICE_TYPES, deviceOptions, chooseDevice } from "../src/browser-device.ts";
import { WEB_VITALS_INIT, readVitals } from "../src/browser-performance.ts";

test("official Playwright device contexts emulate mobile/tablet, not just viewport", () => {
  assert.deepEqual([...DEVICE_TYPES], ["desktop","mobile","tablet"]);
  const pc=deviceOptions("desktop"), phone=deviceOptions("mobile"), tablet=deviceOptions("tablet");
  assert.equal(pc.isMobile,false);assert.equal(pc.hasTouch,false);
  assert.equal(phone.isMobile,true);assert.equal(phone.hasTouch,true);
  assert.equal(phone.deviceScaleFactor,3);assert.equal(phone.viewport.width,390);
  assert.match(phone.userAgent,/Mobile/);
  assert.equal(tablet.isMobile,true);assert.equal(tablet.viewport.width,820);
  assert.equal(tablet.deviceScaleFactor,2);
  for(const d of [pc,phone,tablet]){assert.equal(d.locale,"en-US");assert.equal(d.timezoneId,"UTC");}
});

test("device switching requires explicit Close; no silent cookie/tab destruction", () => {
  assert.equal(chooseDevice(undefined,null),"desktop");
  assert.equal(chooseDevice("mobile",null),"mobile");
  assert.equal(chooseDevice(undefined,"mobile"),"mobile");
  assert.throws(()=>chooseDevice("tablet","mobile"),/BROWSER_DEVICE_CHANGE_REQUIRES_CLOSE/);
  assert.throws(()=>chooseDevice("broken" as never,null),/BROWSER_DEVICE_INVALID/);
});

test("web vitals start before navigation and distinguish unavailable observations", () => {
  for(const term of ["PerformanceObserver","largest-contentful-paint","layout-shift","interactionId","provisional"])
    assert.ok(WEB_VITALS_INIT.includes(term));
  assert.equal(readVitals(undefined),null);
  const observed={source:"PerformanceObserver" as const,phase:"provisional" as const,
    supported:{lcp:true,cls:true,inpCandidate:false},lcpMs:1200,cls:0.03,
    inpCandidateMs:null,observedInteractions:0};
  assert.equal(readVitals(observed)?.lcpMs,1200);
  assert.equal(readVitals({...observed,lcpMs:-7}),null);
});
