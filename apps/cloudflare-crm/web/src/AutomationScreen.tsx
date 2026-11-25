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
    default:
      return {title:"Состояние неизвестно",detail:"Обновите историю перед продолжением.",retry:false};
  }
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
      const split=resourceId.indexOf("/");
      if(split<1)throw Error("Некорректный исходный запуск");
      return (await api.automations.getAutomationRun({
        ruleId:resourceId.slice(0,split),interactionId:resourceId.slice(split+1),
        receiptIdempotencyKey:key
      },{signal})).confirmedMutation;
    },
    refresh:async(marker)=>{
      const separator=marker.resourceId.indexOf("/");
      if(separator<1)throw Error("Некорректный исходный запуск");
      const id=marker.resourceId.slice(0,separator);
      if(runFilter===id&&runCursor==="")await runs.reload();
      else {setRunFilter(id);setRunCursor("");}
    }
  });
  const reset=()=>{
    setEditing(undefined);setDraftId(newId());setSubject("");setAssignee("");
    setDelay("0");setEnabled(false);ruleMutation.changed();
  };
  const beginEdit=(rule:AutomationRule)=>{
    setEditing(rule);setDraftId(rule.ruleId);setSubject(rule.taskSubject);
    setAssignee(rule.assigneeEmail);setDelay(String(rule.dueAfterSeconds));
    setEnabled(rule.enabled);ruleMutation.changed();
  };
  const pending=ruleMutation.busy||ruleMutation.pending||runMutation.busy||runMutation.pending;
  const executeRun=(ruleId:string,sourceId:string,version:bigint)=>
    runMutation.runIntent(()=>prepareMutation(api.automations.runInteractionAutomation,{
      ruleId,interactionId:sourceId,expectedRuleVersion:version,
      precondition:{idempotencyKey:runMutation.key}
    },ruleId+"/"+sourceId),async()=>{
      if(runFilter===ruleId&&runCursor==="")await runs.reload();
      else {setRunFilter(ruleId);setRunCursor("");}
    });
  return <>
    <PageHeader title="Автоматизация"
      description="Типизированные правила Activities, подписанная публикация и восстановимая история. Источник — Rust и D1; браузер не исполняет правила."/>
    <p>Доступно только явное исполнение администратором для записанного взаимодействия.
      Автоматическое выполнение по расписанию ещё не включено.</p>
    <QueryNotice query={me}/>
    <section aria-label="Правила автоматизации">
      <h2>Правила</h2>
      <QueryNotice query={rules}/>
      {rules.data?.rules.map(rule=><Card key={rule.ruleId} title={rule.taskSubject}>
        <p>ID: {rule.ruleId} · Версия: {rule.version.toString()} ·
          {rule.enabled?" Активно":" Отключено"}</p>
        <p>Событие: создано взаимодействие · Ответственный: {rule.assigneeEmail} ·
          Срок после запуска: {rule.dueAfterSeconds.toString()} сек.</p>
        {isAdmin&&<>
          <Button variant="secondary" disabled={pending}
            onClick={()=>beginEdit(rule)}>Редактировать</Button>{" "}
          <Button disabled={pending||!rule.enabled||!interactionId.trim()}
            onClick={()=>void executeRun(rule.ruleId,interactionId.trim(),rule.version)}>
            Выполнить для взаимодействия
          </Button>
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
            ruleId:id,trigger:AutomationTrigger.INTERACTION_CREATED,enabled,
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
          <p>Только конкретное событие и действие CreateTask через оригинальный Rust owner.
            Деактивация правила запрещает новые исполнения; история сохраняется.</p>
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
          }}/> Разрешить ручное исполнение</label></p>
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
        <p>Источник Interaction: {run.interactionId}</p>
        <p>Оригинальный запрос: {run.originalRequestId}</p>
        {run.status===AutomationRunStatus.COMMITTED?
          <p><a href={taskHref(run.taskId,"")}>Открыть созданную задачу</a> ·
            Task receipt: {run.taskRequestId}</p>:
          <><p>{state.detail}</p>
            {isAdmin&&state.retry&&<Button disabled={pending}
              onClick={()=>void executeRun(run.ruleId,run.interactionId,run.ruleVersion)}>
              Восстановить исходный запуск
            </Button>}</>}
      </Card>})}
      {!runs.busy&&runs.data?.runs.length===0&&<p>Запусков пока нет.</p>}
      <Paging next={runs.data?.pageInfo?.nextPageToken??""} busy={runs.busy}
        onFirst={()=>setRunCursor("")}
        onNext={()=>setRunCursor(runs.data?.pageInfo?.nextPageToken??"")}/>
    </section>
  </>;
}
