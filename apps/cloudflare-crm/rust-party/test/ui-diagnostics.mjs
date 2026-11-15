// Read-only, shared Chromium proof of React intent vs the native <dialog>.
// The application owns state in React; this helper adds no production state owner.
import {mkdirSync} from "node:fs";
import {join} from "node:path";

/** @typedef {"open"|"closed"} PanelState */

/** Capture the result of native confirm without changing the UI close decision. */
export async function installUiDiagnostics(page) {
  await page.addInitScript(()=>{
    const confirmations=[];
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

export async function uiState(page) {
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

/** @typedef {"CRM_UI_PANEL_NOT_FOUND"|"CRM_UI_PANEL_NATIVE_DESYNC"|"CRM_UI_PANEL_TRANSITION_REJECTED"|"CRM_UI_PANEL_TIMEOUT"} PanelFailureCode */

/** Pure diagnostic classification: testable before compiling Rust or launching Chromium. */
export function panelFailureCode({panels,expected}) {
  if(panels.length!==1)return "CRM_UI_PANEL_NOT_FOUND";
  const panel=panels[0];
  const actual=panel.nativeOpen?"open":"closed";
  if(panel.desired!==actual)return "CRM_UI_PANEL_NATIVE_DESYNC";
  if(actual!==expected)return "CRM_UI_PANEL_TRANSITION_REJECTED";
  return "CRM_UI_PANEL_TIMEOUT";
}

/**
 * Fail with the actual panel state, route and confirmation outcome, not a
 * naked Playwright 30-second timeout. Do not capture field values or secrets.
 * @param {{
 *   page: import("playwright").Page,
 *   panel: import("playwright").Locator,
 *   title: string,
 *   expected: PanelState,
 *   scenario: string,
 *   evidenceDirectory?: string
 * }} options
 */
export async function assertUiPanel({page,panel,title,expected,scenario,evidenceDirectory}) {
  let timeoutError=null;
  try {
    await panel.waitFor({state:expected==="open"?"visible":"hidden",timeout:5000});
  }catch(error){timeoutError=error;}
  const state=await uiState(page);
  const found=state.panels.filter(x=>x.title===title);
  const target=found.length===1?found[0]:null;
  const actual=target?.nativeOpen?"open":"closed";
  const synced=target?.desired===actual;
  if(!timeoutError&&target!==null&&target.desired===expected&&actual===expected&&synced)return state;
  const code=panelFailureCode({panels:found,expected});
  const evidence={event:"crm.ui.panel-invariant",result:"FAIL",code,
    scenario,title,expected,observed:state};
  if(evidenceDirectory){
    try{
      mkdirSync(evidenceDirectory,{recursive:true});
      const file=join(evidenceDirectory,"ui-"+scenario.replace(/[^a-zA-Z0-9-]/g,"-").slice(0,65)+".png");
      await page.screenshot({path:file,fullPage:true});
      evidence.screenshot=file;
    }catch(error){evidence.screenshotError=String(error);}
  }
  console.error(JSON.stringify(evidence));
  throw new Error(code+": "+JSON.stringify(evidence),timeoutError?{cause:timeoutError}:undefined);
}
