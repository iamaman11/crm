import {useCallback,useState} from "react";
import {AutomationRunDisposition,AutomationRunStatus,AutomationTrigger,MemberRole} from "@ultimate-crm/client/cloudflare";
import type {AutomationRule} from "@ultimate-crm/client/cloudflare";
import {Button,Card,PageHeader,SelectField,TextField} from "@ultimate-crm/ui";
import {api,newId} from "./api";
import {useQuery,useMutation,prepareMutation,MutationNotice,QueryNotice,Paging} from "./hooks";
import {taskHref} from "./routes";
import {allMembers,humanMember} from "./memberCatalog";

// Exhaustive presentation of the server's typed disposition, not a rule engine.
function runDisposition(value:AutomationRunDisposition) {
  switch(value) {
    case AutomationRunDisposition.COMMITTED:
      return {title:"Задача подтверждена",detail:"",retry:false};
    case AutomationRunDisposition.RETRY_READY:
      return {title:"Готово к повтору",detail:"Повторяется только сохранённое намерение.",retry:true};
    case AutomationRunDisposition.WAITING_ASSIGNEE:
      return {title:"Ожидает ответственного",detail:"Ответственный неактивен. Восстановите доступ и обновите историю.",retry:false};
    case AutomationRunDisposition.RULE_SUPERSEDED:
      return {title:"Исполнение прекращено",detail:"Версия правила отключена или заменена. Новая задача по этому запуску запрещена.",retry:false};
    case AutomationRunDisposition.RECONCILE_RECEIPT:
      return {title:"Требуется сверка подтверждения",detail:"Найдено возможное подтверждение Task. Rust сверит исходное намерение без новой задачи.",retry:true};
    case AutomationRunDisposition.EXECUTING:
      return {title:"Выполняется",detail:"Другой запрос владеет временным правом исполнения.",retry:false};
    case AutomationRunDisposition.BACKOFF:
      return {title:"Пауза перед повтором",detail:"Обновите историю после наступления срока следующей попытки.",retry:false};
    case AutomationRunDisposition.ATTEMPTS_EXHAUSTED:
      return {title:"Лимит попыток исчерпан",detail:"Новые попытки запрещены; подтверждение уже выполненной задачи может быть восстановлено.",retry:false};
    case AutomationRunDisposition.CANCELLED:
      return {title:"Запуск отменён",detail:"Новая задача по этому запуску запрещена.",retry:false};
    case AutomationRunDisposition.SOURCE_STALE:
      return {title:"Источник изменился",detail:"Исходная задача была завершена или перенесена до создания результата. Повтор запрещён.",retry:false};
    default:
      return {title:"Состояние неизвестно",detail:"Обновите историю перед продолжением.",retry:false};
  }
}

function triggerLabel(trigger:AutomationTrigger) {
  switch(trigger) {
    case AutomationTrigger.INTERACTION_CREATED: return "Взаимодействие создано";
    case AutomationTrigger.DEAL_CREATED: return "Сделка создана";
    case AutomationTrigger.TASK_OVERDUE: return "Задача просрочена";
    default: return "Неизвестный источник";
  }
}
function runResource(ruleId:string,sourceType:AutomationTrigger,sourceId:string,ruleVersion:bigint) {
  return ruleId+"/"+sourceType+"/"+sourceId+"/"+ruleVersion.toString();
}
function parseRunResource(resourceId:string) {
  const [ruleId,sourceType,sourceId,version]=resourceId.split("/");
  const parsedType=Number(sourceType);
  if(!version)throw Error("Некорректная версия запуска");
  let expectedRuleVersion:bigint;
  try { expectedRuleVersion=BigInt(version); } catch { throw Error("Некорректная версия запуска"); }
  if(!ruleId||!sourceId||expectedRuleVersion<1n||![AutomationTrigger.INTERACTION_CREATED,
    AutomationTrigger.DEAL_CREATED,AutomationTrigger.TASK_OVERDUE].includes(parsedType as AutomationTrigger))
    throw Error("Некорректный исходный запуск");
  return {ruleId,sourceType:parsedType as AutomationTrigger,sourceId,expectedRuleVersion};
}

// The screen is ONLY a generated Connect/Rust projection. No browser rule
// interpreter, callback that mutates Tasks, tenant field, or stored second state.
export function AutomationScreen({initialInteractionId=""}:{initialInteractionId?:string}) {
  const [ruleCursor,setRuleCursor]=useState("");
  const [runCursor,setRunCursor]=useState("");
  const [editing,setEditing]=useState<AutomationRule>();
  const [draftId,setDraftId]=useState<string>(newId);
  const [subject,setSubject]=useState("");
  const [assignee,setAssignee]=useState("");
  const [delay,setDelay]=useState("0");
  const [enabled,setEnabled]=useState(false);
  const [trigger,setTrigger]=useState<AutomationTrigger>(AutomationTrigger.INTERACTION_CREATED);
  const [interactionId,setInteractionId]=useState(initialInteractionId);
  const [runFilter,setRunFilter]=useState("");
  const me=useQuery(useCallback(async(signal:AbortSignal)=>
    (await api.members.getMyMembership({}, {signal})).member,[]));
  const isAdmin=me.data?.active===true&&me.data.role===MemberRole.ADMIN;
  const members=useQuery(useCallback(allMembers,[]));
  const eligible=members.data?.filter(m=>m.active&&humanMember(m.email))??[];
  const rules=useQuery(useCallback(async(signal:AbortSignal)=>
    api.automations.listAutomationRules({page:{pageSize:25,pageToken:ruleCursor}},{signal}),
  [ruleCursor]));
  const runs=useQuery(useCallback(async(signal:AbortSignal)=>
    api.automations.listAutomationRuns({
      ruleId:runFilter,page:{pageSize:25,pageToken:runCursor}
    },{signal}),[runFilter,runCursor]));
  const ruleMutation=useMutation({
    storageKey:"crm.automation.rule.pending.v1",
    lookup:async({resourceId,key},signal)=>
      (await api.automations.getAutomationRule({
        ruleId:resourceId,receiptIdempotencyKey:key
      },{signal})).confirmedMutation,
    refresh:async()=>{
      if(ruleCursor==="")await rules.reload();
      else setRuleCursor("");
    }
  });
  const runMutation=useMutation({
    storageKey:"crm.automation.run.pending.v1",
    lookup:async({resourceId,key},signal)=>{
      const source=parseRunResource(resourceId);
      return (await api.automations.getAutomationRun({
        ...source,receiptIdempotencyKey:key
      },{signal})).confirmedMutation;
    },
    refresh:async(marker)=>{
      const {ruleId}=parseRunResource(marker.resourceId);
      if(runFilter===ruleId&&runCursor==="")await runs.reload();
      else {setRunFilter(ruleId);setRunCursor("");}
    }
  });
  const cancelMutation=useMutation({
    storageKey:"crm.automation.cancel.pending.v1",
    lookup:async({resourceId,key},signal)=>{
      const source=parseRunResource(resourceId);
      return (await api.automations.getAutomationRun({
        ...source,receiptIdempotencyKey:key
      },{signal})).confirmedMutation;
    },
    refresh:async()=>{await runs.reload();}
  });
  const reset=()=>{
    setEditing(undefined);setDraftId(newId());setSubject("");setAssignee("");
    setDelay("0");setEnabled(false);setTrigger(AutomationTrigger.INTERACTION_CREATED);ruleMutation.changed();
  };
  const beginEdit=(rule:AutomationRule)=>{
    setEditing(rule);setDraftId(rule.ruleId);setSubject(rule.taskSubject);
    setAssignee(rule.assigneeEmail);setDelay(String(rule.dueAfterSeconds));
    setEnabled(rule.enabled);setTrigger(rule.trigger);ruleMutation.changed();
  };
  const pending=ruleMutation.busy||ruleMutation.pending||runMutation.busy||runMutation.pending||
    cancelMutation.busy||cancelMutation.pending;
  const executeRun=(ruleId:string,sourceId:string,version:bigint)=>
    runMutation.runIntent(()=>prepareMutation(api.automations.runInteractionAutomation,{
      ruleId,interactionId:sourceId,expectedRuleVersion:version,
      precondition:{idempotencyKey:runMutation.key}
    },runResource(ruleId,AutomationTrigger.INTERACTION_CREATED,sourceId,version)),async()=>{
      if(runFilter===ruleId&&runCursor==="")await runs.reload();
      else {setRunFilter(ruleId);setRunCursor("");}
    });
  return <>
    <PageHeader title="Автоматизация"
      description="Типизированные правила Activities. Создание сделки и просрочка задачи обрабатываются Cron в Rust/D1 даже при закрытом браузере; Interaction сохраняет ручной сценарий."/>
    <p>Плановый запуск выполняется каждые пять минут. История, повторы, отмена и сверка подтверждений принадлежат одному серверному журналу.</p>
    <QueryNotice query={me}/>
    <section aria-label="Правила автоматизации">
      <h2>Правила</h2>
      <QueryNotice query={rules}/>
      {rules.data?.rules.map(rule=><Card key={rule.ruleId} title={rule.taskSubject}>
        <p>ID: {rule.ruleId} · Версия: {rule.version.toString()} ·
          {rule.enabled?" Активно":" Отключено"}</p>
        <p>Событие: {triggerLabel(rule.trigger)} · Ответственный: {rule.assigneeEmail} ·
          Срок после запуска: {rule.dueAfterSeconds.toString()} сек.</p>
        {isAdmin&&<>
          <Button variant="secondary" disabled={pending}
            onClick={()=>beginEdit(rule)}>Редактировать</Button>{" "}
          {rule.trigger===AutomationTrigger.INTERACTION_CREATED&&
            <Button disabled={pending||!rule.enabled||!interactionId.trim()}
              onClick={()=>void executeRun(rule.ruleId,interactionId.trim(),rule.version)}>
              Выполнить для взаимодействия
            </Button>}
        </>}
        <p><Button variant="secondary" disabled={runs.busy} onClick={()=>{
          setRunFilter(rule.ruleId);setRunCursor("");
        }}>История этого правила</Button></p>
      </Card>)}
      <Paging next={rules.data?.pageInfo?.nextPageToken??""} busy={rules.busy}
        onFirst={()=>setRuleCursor("")}
        onNext={()=>setRuleCursor(rules.data?.pageInfo?.nextPageToken??"")}/>
      {isAdmin&&<>
        <form aria-label={editing?"Редактирование правила":"Новое правило"}
          onChange={ruleMutation.changed} onSubmit={event=>{
          event.preventDefault();
          const seconds=Number(delay);
          if(!Number.isSafeInteger(seconds)||seconds<0||seconds>31_536_000){
            void ruleMutation.reportError(new Error("Срок должен быть от 0 до 31536000 секунд"));
            return;
          }
          const id=editing?.ruleId??draftId;
          void ruleMutation.runIntent(()=>prepareMutation(api.automations.putAutomationRule,{
            ruleId:id,trigger,enabled,
            taskSubject:subject.trim(),assigneeEmail:assignee,
            dueAfterSeconds:BigInt(seconds),
            precondition:{idempotencyKey:ruleMutation.key,
              ...(editing?{expectedVersion:editing.version}:{})}
          },id),async()=>{
            reset();
            if(ruleCursor==="")await rules.reload();
            else setRuleCursor("");
          });
        }}>
          <h3>{editing?"Изменить правило":"Создать правило"}</h3>
          <p>Выберите источник и сотрудника. Действие всегда создаётся исходным Rust Task owner.
            Деактивация правила запрещает новые исполнения; история сохраняется.</p>
          <SelectField label="Событие запуска" value={String(trigger)}
            onChange={e=>{setTrigger(Number(e.target.value) as AutomationTrigger);ruleMutation.changed();}}>
            <option value={AutomationTrigger.INTERACTION_CREATED}>Взаимодействие создано (ручной запуск)</option>
            <option value={AutomationTrigger.DEAL_CREATED}>Сделка создана (автоматически)</option>
            <option value={AutomationTrigger.TASK_OVERDUE}>Задача просрочена (автоматически)</option>
          </SelectField>
          <TextField label="Название новой задачи" value={subject} required
            onChange={e=>{setSubject(e.target.value);ruleMutation.changed();}}/>
          <SelectField label="Назначить активному сотруднику" value={assignee}
            onChange={e=>{setAssignee(e.target.value);ruleMutation.changed();}}>
            <option value="">Выберите сотрудника</option>
            {eligible.map(m=><option key={m.email} value={m.email}>{m.email}</option>)}
          </SelectField>
          <QueryNotice query={members}/>
          <TextField label="Срок после выполнения (секунды; 0 — без срока)"
            type="number" value={delay} required
            onChange={e=>{setDelay(e.target.value);ruleMutation.changed();}}/>
          <p><label><input type="checkbox" checked={enabled} onChange={e=>{
            setEnabled(e.target.checked);ruleMutation.changed();
          }}/> Активировать правило</label></p>
          <Button type="submit" disabled={pending||!subject.trim()||!assignee}>
            {editing?"Сохранить новую версию":"Опубликовать правило"}
          </Button>{" "}
          {editing&&<Button type="button" variant="secondary" disabled={pending}
            onClick={reset}>Отмена</Button>}
        </form>
      </>}
      <MutationNotice state={ruleMutation}/>
    </section>
    {isAdmin&&<section aria-label="Ручное исполнение">
      <h2>Взаимодействие — источник</h2>
      <TextField label="ID существующего взаимодействия" value={interactionId}
        onChange={e=>{setInteractionId(e.target.value);runMutation.changed();}}/>
      <p>Выберите активное правило и запустите его. Повторный запуск не создаёт новую задачу:
        сервер использует идентичный Task receipt и зафиксированный источник события.</p>
      <MutationNotice state={runMutation}/>
    </section>}
    <section aria-label="История автоматизации">
      <h2>Запуски</h2>
      {runFilter&&<p>Фильтр по правилу: {runFilter}{" "}
        <Button variant="secondary" onClick={()=>{setRunFilter("");setRunCursor("");}}>
          Сбросить</Button></p>}
      <QueryNotice query={runs}/>
      <Button variant="secondary" disabled={runs.busy}
        onClick={()=>void runs.reload().catch(()=>{})}>Обновить историю</Button>
      {runs.data?.runs.map(run=>{
        const state=runDisposition(run.disposition);
        return <Card key={run.ruleId+"/"+run.sourceEventId} title={state.title}>
        <p>Правило: {run.ruleId} · Версия: {run.ruleVersion.toString()}</p>
        <p>Источник: {triggerLabel(run.sourceType)} · ID: {run.sourceId} · версия: {run.sourceVersion.toString()}</p>
        <p>Оригинальный запрос: {run.originalRequestId}</p>
        <p>Попыток исполнения: {run.attemptCount.toString()} из {run.attemptLimit.toString()}</p>
        {run.disposition===AutomationRunDisposition.BACKOFF&&
          <p>Следующая попытка: {new Date(Number(run.nextAttemptAtUnixMillis)).toLocaleString("ru-RU")}</p>}
        {run.status===AutomationRunStatus.CANCELLED&&
          <p>Отмена: {run.cancelledBy} · Запрос: {run.cancelRequestId}</p>}
        {run.status===AutomationRunStatus.COMMITTED?
          <p><a href={taskHref(run.taskId,"")}>Открыть созданную задачу</a> ·
            Task receipt: {run.taskRequestId}</p>:
          <><p>{state.detail}</p>
            {isAdmin&&state.retry&&run.sourceType===AutomationTrigger.INTERACTION_CREATED&&
              <Button disabled={pending}
                onClick={()=>void executeRun(run.ruleId,run.sourceId,run.ruleVersion)}>
                Восстановить исходный запуск
              </Button>}
            {state.retry&&run.sourceType!==AutomationTrigger.INTERACTION_CREATED&&
              <p>Следующий такт Cron автоматически сверит исходный Task receipt и продолжит тот же запуск.</p>}
            {isAdmin&&run.canCancel&&<Button variant="secondary" disabled={pending}
              onClick={()=>void cancelMutation.runIntent(()=>prepareMutation(
                api.automations.cancelAutomationRun,{
                  ruleId:run.ruleId,sourceType:run.sourceType,sourceId:run.sourceId,
                  expectedRuleVersion:run.ruleVersion,
                  precondition:{idempotencyKey:cancelMutation.key}
                },runResource(run.ruleId,run.sourceType,run.sourceId,run.ruleVersion)
              ),async()=>{await runs.reload();})}>
              Отменить исходный запуск
            </Button>}</>}
      </Card>})}
      <MutationNotice state={cancelMutation}/>
      {!runs.busy&&runs.data?.runs.length===0&&<p>Запусков пока нет.</p>}
      <Paging next={runs.data?.pageInfo?.nextPageToken??""} busy={runs.busy}
        onFirst={()=>setRunCursor("")}
        onNext={()=>setRunCursor(runs.data?.pageInfo?.nextPageToken??"")}/>
    </section>
  </>;
}
