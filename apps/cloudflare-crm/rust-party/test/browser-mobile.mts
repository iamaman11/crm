import assert from "node:assert/strict";
import {resolve} from "node:path";
import {expect} from "playwright/test";
import type {Page} from "playwright";

type MobileScenario = {
  page: Page;
  directory: string;
  newPipelineId: string;
  navigate: (hash: string, title: string) => Promise<void>;
  reloadDocument: () => Promise<unknown>;
  checkpoint: (label: string) => void;
};

export async function verifyBrowserMobile({page,directory,newPipelineId,navigate,reloadDocument,checkpoint}: MobileScenario) {
   await page.getByLabel("Тема",{exact:true}).selectOption("dark");
   assert.equal(await page.locator("html").getAttribute("data-theme"),"dark");
   await reloadDocument();
   assert.equal(await page.locator("html").getAttribute("data-theme"),"dark");
   await page.setViewportSize({width:390,height:844});
   await navigate("sales","Сделки");
   await page.getByLabel("Воронка продаж",{exact:true}).selectOption(newPipelineId);
   const mobileKanban=page.getByRole("region",{name:"Kanban — этапы и сделки"});
   await expect(mobileKanban).toHaveAttribute("tabindex","0");
   const scrollMetrics=await mobileKanban.evaluate(element=>({
     clientWidth:element.clientWidth,scrollWidth:element.scrollWidth,
     clientHeight:element.clientHeight,scrollHeight:element.scrollHeight,
     viewportWidth:window.innerWidth,
     documentWidth:document.documentElement.scrollWidth
   }));
   assert.ok(scrollMetrics.viewportWidth<=391&&scrollMetrics.documentWidth<=391,
     "the mobile page itself must remain at 390 px: "+JSON.stringify(scrollMetrics));
   assert.ok(scrollMetrics.scrollWidth>scrollMetrics.clientWidth,
     "mobile Kanban must scroll horizontally inside its named region: "+JSON.stringify(scrollMetrics));
   await mobileKanban.focus();
   assert.equal(await mobileKanban.evaluate(element=>element===document.activeElement),true,
     "mobile Kanban scroll region must be keyboard focusable");
   await page.keyboard.press("ArrowRight");
   await expect.poll(()=>mobileKanban.evaluate(element=>element.scrollLeft))
     .toBeGreaterThan(0);
   await page.screenshot({path:resolve(directory,"crm-mobile-dark.png"),fullPage:true});
   assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth+1),"mobile horizontal overflow");
   checkpoint("Mobile Kanban: named keyboard-focusable horizontal scroll region without viewport overflow");
   await navigate("accounts","Клиенты");
   await page.getByRole("button",{name:"Создать клиента",exact:true}).click();
   const mobilePanel=page.getByRole("dialog");
   await mobilePanel.waitFor();
   const bounds=await mobilePanel.evaluate(element=>({
     width:element.getBoundingClientRect().width,viewport:window.innerWidth
   }));
   assert.ok(bounds.width<=bounds.viewport+1 && bounds.width>=bounds.viewport-2,"record panel fills mobile viewport");
   await page.keyboard.press("Escape");
   await mobilePanel.waitFor({state:"hidden"});
   checkpoint("Theme persistence and mobile layout");

}
