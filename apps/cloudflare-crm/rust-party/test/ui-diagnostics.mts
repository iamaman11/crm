// Read-only, shared Chromium proof of React intent vs the native <dialog>.
// The application owns state in React; this helper adds no production state owner.
import {mkdirSync} from "node:fs";
import {join} from "node:path";
import type {Locator, Page} from "playwright";

type PanelState = "open" | "closed";
type Confirmation = { accepted: boolean; at: number };
type PanelFailureCode =
  | "CRM_UI_PANEL_NOT_FOUND"
  | "CRM_UI_PANEL_NATIVE_DESYNC"
  | "CRM_UI_PANEL_TRANSITION_REJECTED"
  | "CRM_UI_PANEL_TIMEOUT";

declare global {
  interface Window {
    readonly __crmUiConfirmations?: Confirmation[];
  }
}

/** Capture the result of native confirm without changing the UI close decision. */
export async function installUiDiagnostics(page: Page) {
  await page.addInitScript(()=>{
    const confirmations: Confirmation[]=[];
    Object.defineProperty(window,"__crmUiConfirmations",{value:confirmations});
    const original=window.confirm.bind(window);
    window.confirm=message=>{
      const accepted=original(message);
      confirmations.push({accepted,at:performance.now()});
      if(confirmations.length>12)confirmations.shift();
      return accepted;
    };
  });
}

export async function uiState(page: Page) {
  return page.evaluate(()=>{
    const root=document.querySelector("[data-crm-route-kind]");
    return {
      hash:location.hash,
      route:root?.getAttribute("data-crm-route-kind")??null,
      activeElement:{
        tag:document.activeElement?.tagName??null,
        label:document.activeElement?.getAttribute("aria-label")??null,
      },
      panels:[...document.querySelectorAll("dialog")].map(element=>({
        panelId:element.getAttribute("aria-labelledby"),
        title:element.querySelector("header h2")?.textContent?.trim()??"",
        desired:element.getAttribute("data-crm-panel-desired"),
        nativeOpen:element.open,
        dirty:element.getAttribute("data-crm-panel-dirty"),
        closeSource:element.getAttribute("data-crm-panel-last-close"),
      })),
      confirmations:[...(window.__crmUiConfirmations??[])].slice(-6),
    };
  });
}

/** Pure diagnostic classification: testable before compiling Rust or launching Chromium. */
export function panelFailureCode({panels,expected}: {
  panels: ReadonlyArray<{nativeOpen: boolean; desired: string | null}>;
  expected: PanelState;
}): PanelFailureCode {
  const [panel] = panels;
  if(panels.length!==1 || !panel)return "CRM_UI_PANEL_NOT_FOUND";
  const actual=panel.nativeOpen?"open":"closed";
  if(panel.desired!==actual)return "CRM_UI_PANEL_NATIVE_DESYNC";
  if(actual!==expected)return "CRM_UI_PANEL_TRANSITION_REJECTED";
  return "CRM_UI_PANEL_TIMEOUT";
}

type PanelExpectation = {
  page: Page;
  panel: Locator;
  title: string;
  panelId: string;
  expected: PanelState;
  scenario: string;
  evidenceDirectory?: string;
};

export async function assertUiPanel({page,panel,title,panelId,expected,scenario,evidenceDirectory}: PanelExpectation) {
  if(!panelId)throw new Error("CRM_UI_PANEL_ID_MISSING: "+scenario);
  let waitFailed=false;
  try {
    await panel.waitFor({state:expected==="open"?"visible":"hidden",timeout:5000});
  }catch{waitFailed=true;}
  const state=await uiState(page);
  // aria-labelledby originates from React useId, stays stable even when
  // the heading changes from "Изменить участника" to "Новый участник".
  const found=state.panels.filter(x=>x.panelId===panelId);
  const target=found.length===1?found[0]:undefined;
  const actual=target?.nativeOpen?"open":"closed";
  const synced=target?.desired===actual;
  if(!waitFailed&&target!==undefined&&target.desired===expected&&actual===expected&&synced)return state;
  const code=panelFailureCode({panels:found,expected});
  const evidence: {
    event: "crm.ui.panel-invariant";
    result: "FAIL";
    code: PanelFailureCode;
    scenario: string;
    title: string;
    panelId: string;
    expected: PanelState;
    observed: object;
    screenshot?: string;
    screenshotError?: true;
  } = {
    event:"crm.ui.panel-invariant",result:"FAIL",code,scenario,title,panelId,expected,
    observed: {
      route: state.route,
      panels: found.map(({nativeOpen,desired,dirty,closeSource})=>({nativeOpen,desired,dirty,closeSource})),
      confirmations: state.confirmations,
    },
  };
  if(evidenceDirectory){
    try{
      mkdirSync(evidenceDirectory,{recursive:true});
      const file=join(evidenceDirectory,"ui-"+scenario.replace(/[^a-zA-Z0-9-]/g,"-").slice(0,65)+".png");
      await page.screenshot({path:file,fullPage:true});
      evidence.screenshot=file;
    }catch{evidence.screenshotError=true;}
  }
  console.error(JSON.stringify(evidence));
  throw new Error(code+": "+JSON.stringify(evidence));
}
