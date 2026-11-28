import assert from "node:assert/strict";
import {expect} from "playwright/test";
import type {Locator, Page, Response} from "playwright";
import {AutomationService} from "../../../../packages/client/gen/crm/activities/v2/automation_pb.js";
import {TaskService} from "../../../../packages/client/gen/crm/activities/v2/task_pb.js";
import {fixtureRpcPath} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import {string} from "../../scripts/json-boundary.mts";
import type {BrowserSql} from "./browser-boundaries.mts";

type AutomationScenario = {
  page: Page; sql: BrowserSql; tenantA: string; dealId: string; laterCall: string; runId: string;
  navigate: (hash: string, title: string) => Promise<void>;
  reloadDocument: () => Promise<unknown>;
  checkpoint: (label: string) => void;
  assertMutation: (response: Response, method: string, options?: {replay?: boolean}) => Promise<void>;
  runScheduled: (input:{cron:string;scheduledTime:Date})=>Promise<{outcome:string;noRetry:boolean}>;
};
type AutomationMethod = typeof AutomationService.method.putAutomationRule |
  typeof AutomationService.method.runInteractionAutomation |
  typeof AutomationService.method.cancelAutomationRun | typeof TaskService.method.createTask;

export async function verifyBrowserAutomation({page,sql,tenantA,dealId,laterCall,runId,navigate,
  reloadDocument,checkpoint,assertMutation,runScheduled}: AutomationScenario) {
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
   await page.getByLabel("Активировать правило",{exact:true}).check();
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

   // SK-08A.3: publish an overdue-Task rule, create the source through the
   // original Deal→Task UI, unload the CRM page, and only then invoke the
   // actual scheduled Worker. History is read after the application reloads.
   await navigate("automation","Автоматизация");
   const overdueTitle="Browser overdue follow-up "+runId;
   await page.getByLabel("Событие запуска",{exact:true}).selectOption("2");
   await page.getByLabel("Название новой задачи",{exact:true}).fill(overdueTitle);
   await page.getByLabel("Назначить активному сотруднику",{exact:true})
     .selectOption("native-viewer@example.test");
   await page.getByLabel("Срок после выполнения (секунды; 0 — без срока)",{exact:true}).fill("0");
   await page.getByLabel("Активировать правило",{exact:true}).check();
   await mutation(page.getByRole("button",{name:"Опубликовать правило",exact:true}),
     AutomationService.method.putAutomationRule);
   const overdueRuleRow=(await sql(
     "SELECT id FROM crm_activity_automation_rules WHERE tenant=? AND task_subject=?",
     tenantA,overdueTitle))[0];
   assert.ok(overdueRuleRow);
   const overdueRuleId=string(overdueRuleRow.id);

   await navigate("sales/"+dealId,"Карточка сделки");
   await page.getByRole("link",{name:"Задачи этой сделки",exact:true}).click();
   await page.getByRole("heading",{name:"Задачи сделки",exact:true}).waitFor();
   await page.getByRole("button",{name:"Новая задача сделки",exact:true}).click();
   const overdueForm=page.getByRole("dialog");
   const sourceTitle="Browser overdue source "+runId;
   await overdueForm.getByLabel("Тема задачи",{exact:true}).fill(sourceTitle);
   const past=new Date(Date.now()-120_000);
   const dueLocal=new Date(past.getTime()-past.getTimezoneOffset()*60_000).toISOString().slice(0,16);
   await overdueForm.getByLabel("Срок",{exact:true}).fill(dueLocal);
   await mutation(overdueForm.getByRole("button",{name:"Создать задачу",exact:true}),
     TaskService.method.createTask);
   const sourceRow=(await sql(
     "SELECT id,version FROM crm_activity_task_records WHERE tenant=? AND json_extract(state,'$.subject')=?",
     tenantA,sourceTitle))[0];
   assert.ok(sourceRow);
   const sourceTaskId=string(sourceRow.id);
   assert.equal(sourceRow.version,1);

   const returnUrl=new URL(page.url());
   returnUrl.hash="#automation";
   await page.goto("about:blank");
   const scheduled=await runScheduled({cron:"*/5 * * * *",scheduledTime:new Date(Date.now()+60_000)});
   assert.equal(scheduled.outcome,"ok");
   await page.goto(returnUrl.toString());
   await page.getByRole("heading",{name:"Автоматизация",exact:true}).waitFor();
   const sourceLabel="Источник: Задача просрочена · ID: "+sourceTaskId+" · версия: 1";
   await page.getByText(sourceLabel,{exact:true}).waitFor();
   const overdueRun=(await sql(
     "SELECT task_id,status,source_version FROM crm_activity_automation_runs WHERE tenant=? AND rule_id=? AND source_type='task_overdue' AND source_id=?",
     tenantA,overdueRuleId,sourceTaskId))[0];
   assert.ok(overdueRun);
   assert.equal(overdueRun.status,"committed");
   assert.equal(overdueRun.source_version,1);
   const generatedTaskId=string(overdueRun.task_id);
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_activity_task_receipts WHERE tenant=? AND key=?",
     tenantA,generatedTaskId))[0]?.n,1);
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='activity_tasks.create'",
     tenantA,generatedTaskId))[0]?.n,1);
   await reloadDocument();
   await page.getByRole("heading",{name:"Автоматизация",exact:true}).waitFor();
   await page.getByText(sourceLabel,{exact:true}).waitFor();
   checkpoint("SK-08A.3 real Cron after CRM UI unload, overdue Task owner, D1 receipt and history reload");

   // A second source is intentionally made to fail at the Task INSERT. The
   // pending run remains cancellable, and a later Cron tick cannot cross it.
   const cancelSourceTitle="Browser overdue cancellation source "+runId;
   await navigate("sales/"+dealId,"Карточка сделки");
   await page.getByRole("link",{name:"Задачи этой сделки",exact:true}).click();
   await page.getByRole("heading",{name:"Задачи сделки",exact:true}).waitFor();
   await page.getByRole("button",{name:"Новая задача сделки",exact:true}).click();
   const cancelForm=page.getByRole("dialog");
   await cancelForm.getByLabel("Тема задачи",{exact:true}).fill(cancelSourceTitle);
   await cancelForm.getByLabel("Срок",{exact:true}).fill(dueLocal);
   await mutation(cancelForm.getByRole("button",{name:"Создать задачу",exact:true}),
     TaskService.method.createTask);
   const cancelSourceRow=(await sql(
     "SELECT id FROM crm_activity_task_records WHERE tenant=? AND json_extract(state,'$.subject')=?",
     tenantA,cancelSourceTitle))[0];
   assert.ok(cancelSourceRow);
   const cancelSourceId=string(cancelSourceRow.id);
   await sql("CREATE TRIGGER fixture_browser_fail_automation_task BEFORE INSERT ON crm_activity_task_records "+
     "WHEN NEW.id LIKE 'automation-%' BEGIN SELECT RAISE(ABORT,'fixture browser Automation Task blocked'); END;");
   const cancelReturnUrl=new URL(page.url());
   cancelReturnUrl.hash="#automation";
   await page.goto("about:blank");
   try { await runScheduled({cron:"*/5 * * * *",scheduledTime:new Date()}); }
   finally { await sql("DROP TRIGGER fixture_browser_fail_automation_task"); }
   await page.goto(cancelReturnUrl.toString());
   await page.getByRole("heading",{name:"Автоматизация",exact:true}).waitFor();
   await page.getByText("Источник: Задача просрочена · ID: "+cancelSourceId+" · версия: 1",
     {exact:true}).waitFor();
   const cancelButton=page.getByRole("button",{name:"Отменить исходный запуск",exact:true});
   await expect(cancelButton).toHaveCount(1);
   await mutation(cancelButton,AutomationService.method.cancelAutomationRun);
   await page.getByText("Запуск отменён",{exact:true}).waitFor();
   await page.goto("about:blank");
   await runScheduled({cron:"*/5 * * * *",scheduledTime:new Date()});
   const cancelledSourceRun=(await sql(
     "SELECT task_id,cancel_request_id FROM crm_activity_automation_runs WHERE tenant=? AND rule_id=? AND source_type='task_overdue' AND source_id=?",
     tenantA,overdueRuleId,cancelSourceId))[0];
   assert.ok(cancelledSourceRun?.cancel_request_id);
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_activity_task_records WHERE tenant=? AND id=?",
     tenantA,string(cancelledSourceRun.task_id)))[0]?.n,0);
   // Restore the UI after the autonomous Cron tick and verify cancellation
   // remains visible after a fresh document load.
   await page.goto(cancelReturnUrl.toString());
   await page.getByRole("heading",{name:"Автоматизация",exact:true}).waitFor();
   await page.getByText("Источник: Задача просрочена · ID: "+cancelSourceId+" · версия: 1",
     {exact:true}).waitFor();
   await page.getByText("Запуск отменён",{exact:true}).waitFor();
   checkpoint("SK-08A.3 operator cancellation fences an autonomous pending Task after browser unload");

}
