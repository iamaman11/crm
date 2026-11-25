import assert from "node:assert/strict";
import {expect} from "playwright/test";
import type {Locator, Page, Response} from "playwright";
import {AutomationService} from "../../../../packages/client/gen/crm/activities/v2/automation_pb.js";
import {fixtureRpcPath} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import {string} from "../../scripts/json-boundary.mts";
import type {BrowserSql} from "./browser-boundaries.mts";

type AutomationScenario = {
  page: Page; sql: BrowserSql; tenantA: string; dealId: string; laterCall: string; runId: string;
  navigate: (hash: string, title: string) => Promise<void>;
  reloadDocument: () => Promise<unknown>;
  checkpoint: (label: string) => void;
  assertMutation: (response: Response, method: string, options?: {replay?: boolean}) => Promise<void>;
};
type AutomationMethod = typeof AutomationService.method.putAutomationRule | typeof AutomationService.method.runInteractionAutomation;

export async function verifyBrowserAutomation({page,sql,tenantA,dealId,laterCall,runId,navigate,
  reloadDocument,checkpoint,assertMutation}: AutomationScenario) {
  const mutation=async(button: Locator,method: AutomationMethod,options?: {replay?: boolean})=>{
    const pending=page.waitForResponse(response=>new URL(response.url()).pathname===fixtureRpcPath(method)
      && response.request().method()==="POST");
    await button.click();
    await assertMutation(await pending,method.name,options);
    await page.getByRole("heading",{name:/^Сохранено/}).last().waitFor();
  };
   // SK-08A: use the source link, original typed rule editor, and original Task
   // UI. No browser-created source ID or separate task mutation.
   await navigate("sales/"+dealId,"Карточка сделки");
   const automationSource=page.getByRole("region",{name:"История взаимодействий "+dealId})
     .locator("ol li").filter({hasText:laterCall});
   const sourceLink=automationSource.getByRole("link",{name:"Правила для взаимодействия"});
   const sourceHref=await sourceLink.getAttribute("href");
   assert.ok(sourceHref);
   assert.ok(sourceHref.startsWith("#automation?interactionId="));
   const sourceId=string(new URLSearchParams(sourceHref.split("?")[1]).get("interactionId"));
   await sourceLink.click();
   await page.getByRole("heading",{name:"Автоматизация",exact:true}).waitFor();
   await expect(page.getByLabel("ID существующего взаимодействия")).toHaveValue(sourceId);
   await reloadDocument();
   await expect(page.getByLabel("ID существующего взаимодействия")).toHaveValue(sourceId);
   const autoTitle="Browser SK08A follow-up "+runId;
   await page.getByLabel("Название новой задачи",{exact:true}).fill(autoTitle);
   await page.getByLabel("Назначить активному сотруднику",{exact:true}).selectOption("native-viewer@example.test");
   await page.getByLabel("Срок после выполнения (секунды; 0 — без срока)",{exact:true}).fill("3600");
   await page.getByLabel("Разрешить ручное исполнение",{exact:true}).check();
   await mutation(page.getByRole("button",{name:"Опубликовать правило",exact:true}),AutomationService.method.putAutomationRule);
   const ruleRows=await sql("SELECT id FROM crm_activity_automation_rules WHERE tenant=? AND task_subject=?",tenantA,autoTitle);
   assert.equal(ruleRows.length,1);
   const autoRuleId=string(ruleRows[0]?.id);
   // The isolated browser tenant has exactly one published rule.
   const executeButton=page.getByRole("region",{name:"Правила автоматизации"})
     .getByRole("button",{name:"Выполнить для взаимодействия",exact:true});
   await expect(executeButton).toHaveCount(1);
   await mutation(executeButton,AutomationService.method.runInteractionAutomation);
   const autoRun=(await sql("SELECT task_id,status,task_request_id FROM crm_activity_automation_runs WHERE tenant=? AND rule_id=? AND interaction_id=?",tenantA,autoRuleId,sourceId))[0];
   assert.ok(autoRun);
   const autoTaskId=string(autoRun.task_id);
   assert.equal(autoRun.status,"committed");
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_activity_task_receipts WHERE tenant=? AND key=?",tenantA,autoTaskId))[0]?.n,1);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='activity_tasks.create'",tenantA,autoTaskId))[0]?.n,1);
   await page.getByRole("link",{name:"Открыть созданную задачу",exact:true}).waitFor();
   await mutation(executeButton,AutomationService.method.runInteractionAutomation,{replay:true});
   await page.getByRole("heading",{name:"Задача подтверждена",exact:true}).waitFor();
   await page.getByText("Попыток исполнения: 1 из 5",{exact:true}).waitFor();
   assert.equal(await page.getByRole("button",{name:"Отменить исходный запуск",exact:true}).count(),0);
   assert.equal(await page.getByRole("button",{name:"Восстановить исходный запуск",exact:true}).count(),0);
   await page.getByRole("button",{name:"Обновить историю",exact:true}).click();
   await page.getByRole("heading",{name:"Задача подтверждена",exact:true}).waitFor();
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_activity_automation_run_requests WHERE tenant=? AND rule_id=?",tenantA,autoRuleId))[0]?.n,2);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='activity_tasks.create'",tenantA,autoTaskId))[0]?.n,1);
   await reloadDocument();
   const autoTaskLink=page.getByRole("link",{name:"Открыть созданную задачу",exact:true});
   await autoTaskLink.waitFor();
   await autoTaskLink.click();
   await page.getByText(autoTitle,{exact:true}).first().waitFor();
   checkpoint("SK-08A typed Interaction deep link, rule publication, original Task, D1 receipts and reload");

}
