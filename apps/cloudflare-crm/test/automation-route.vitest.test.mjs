import {expect,it} from "vitest";
import {automationHref,parseRoute} from "../web/src/routes.ts";

it("deep-links the original Interaction to the existing typed automation projection",()=>{
  const id="interaction-typed-123";
  expect(automationHref(id)).toBe("#automation?interactionId="+id);
  expect(parseRoute(automationHref(id),[])).toMatchObject({
    kind:"section",screen:"automation",interactionId:id,accountId:""
  });
  expect(parseRoute("#automation",[])).toMatchObject({
    kind:"section",screen:"automation",accountId:""
  });
});

it("rejects ambiguous, unbounded and invented automation URL parameters",()=>{
  const rejected=[
    "#automation?interactionId=first&interactionId=second",
    "#automation?unknown=anything",
    "#automation?interactionId="+encodeURIComponent("../another"),
    "#automation?interactionId="+encodeURIComponent("slash\\path"),
    "#automation?interactionId="+encodeURIComponent("percent%name"),
    "#automation?interactionId="+("a".repeat(181)),
  ];
  for(const hash of rejected){
    expect(parseRoute(hash,[])).toMatchObject({kind:"not-found"});
  }
});
