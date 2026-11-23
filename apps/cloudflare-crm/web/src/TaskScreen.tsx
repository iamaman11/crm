import {useCallback,useState} from "react";
import {Button,Card,RecordPanel,FormGrid,PageHeader,SelectField,TextField} from "@ultimate-crm/ui";
import type {Task} from "@ultimate-crm/client/cloudflare";
import {api,field,newId,timestamp,dateText} from "./api";
import {MemberRole,TaskPriority,TaskStatus} from "@ultimate-crm/client/cloudflare";
import {enumValue} from "./enumFields";
import {useQuery,useMutation,prepareMutation,MutationNotice,QueryNotice,Paging} from "./hooks";
import {AccountPicker,AccountName} from "./Pickers";
import {TimeFields} from "./TimeFields";
import {timePatch} from "./timePatch";
import {accountHref,scopeHref,taskHref,dealHref,taskSearchHref} from "./routes";
import {SearchField} from "./SearchField";
import {navigateTo} from "./navigation";
import {allMembers,humanMember} from "./memberCatalog";
import {allTeams,ownerFields} from "./teamCatalog";

export function TaskScreen({initialAccountId="",focusedTaskId="",initialDealId="",initialSearch="",initialOverdue=false,
  initialAssigneeEmail="",initialMine=false,initialTeamId=""}:{
  initialAccountId?:string;focusedTaskId?:string;initialDealId?:string;initialSearch?:string;
  initialOverdue?:boolean;initialAssigneeEmail?:string;initialMine?:boolean;initialTeamId?:string;
}) {
  const [cursor,setCursor]=useState("");
  const [status,setStatus]=useState("");
  const [priority,setPriority]=useState("");
  const [accountId,setAccountId]=useState(initialAccountId);
  const [formAccountId,setFormAccountId]=useState(initialAccountId);
  const [draftId,setDraftId]=useState(newId);
  const [selected,setSelected]=useState<Task>();
  const me=useQuery(useCallback(async(signal:AbortSignal)=>
    (await api.members.getMyMembership({}, {signal})).member,[]));
  const isAdmin=me.data?.active===true&&me.data.role===MemberRole.ADMIN;
  const canWorkOn=(task:Task)=>isAdmin||(me.data?.active===true&&
    me.data.role===MemberRole.VIEWER&&humanMember(me.data.email)&&task.assigneeIdentity===me.data.email);
  const membersQuery=useQuery(useCallback(allMembers,[]));
  const members=membersQuery.data?.filter(member=>humanMember(member.email))??[];
  const teamsQuery=useQuery(useCallback(allTeams,[]));
  const teams=teamsQuery.data??[];
  const eligibleTeams=teams.filter(team=>team.active&&team.memberEmails.some(email=>members.some(m=>m.active&&m.email===email)));
  const canClaim=(task:Task)=>me.data?.active===true&&humanMember(me.data.email)&&
    teams.some(team=>team.active&&team.teamId===task.ownerTeamId&&team.memberEmails.includes(me.data?.email??""));
  const [editorMode,setEditorMode]=useState<"create"|"edit"|"reminder"|null>(null);
  const query=useQuery(useCallback(async(signal:AbortSignal)=>{
    if(focusedTaskId)return undefined; // Direct-link view never loads a full task list.
    return api.tasks.listTasks({page:{pageSize:25,pageToken:cursor},search:initialSearch.trim(),
      overdueOnly:initialOverdue,
      ...(initialAssigneeEmail?{assigneeEmail:initialAssigneeEmail}:{}),
      ...(initialMine?{assignedToMe:true}:{}),
      ...(initialTeamId?{ownerTeamId:initialTeamId}:{}),
      ...(initialDealId?{dealId:initialDealId}:{}),
      ...(accountId?{accountId}:{}),
      ...(status&&!initialOverdue?{status:enumValue(status,[TaskStatus.OPEN,TaskStatus.COMPLETED],"Статус задачи")}:{}),
      ...(priority?{priority:enumValue(priority,[TaskPriority.LOW,TaskPriority.NORMAL,TaskPriority.HIGH,TaskPriority.URGENT],"Приоритет задачи")}:{})},{signal});
  },[cursor,status,priority,accountId,focusedTaskId,initialDealId,initialSearch,
      initialOverdue,initialAssigneeEmail,initialMine,initialTeamId]));
  const focused=useQuery(useCallback(async(signal:AbortSignal)=>{
    if(!focusedTaskId)return undefined;
    const task=(await api.tasks.getTask({taskId:focusedTaskId},{signal})).task;
    if(!task)throw Error("Задача не найдена");
    if(initialAccountId&&task.accountId!==initialAccountId)
      throw Error("Задача не принадлежит выбранному клиенту");
    return task;
  },[focusedTaskId,initialAccountId]));
  // A Deal-scoped view verifies the real tenant-bound Deal and its Account
  // before giving the user a named relationship context.
  const dealContext=useQuery(useCallback(async(signal:AbortSignal)=>{
    if(!initialDealId)return undefined;
    const deal=(await api.deals.getDeal({dealId:initialDealId},{signal})).deal;
    if(!deal)throw Error("Сделка не найдена");
    if(initialAccountId&&deal.accountId!==initialAccountId)
      throw Error("Сделка не принадлежит выбранному клиенту");
    return deal;
  },[initialDealId,initialAccountId]));
  const mutation=useMutation({
    storageKey:"crm.task.pending.v1",
    lookup:async({resourceId,key},signal)=>(await api.tasks.getTask({
      taskId:resourceId,receiptIdempotencyKey:key
    },{signal})).confirmedMutation,
    refresh:async()=>{await query.reload();await focused.reload();}
  });
  const visibleTasks=focusedTaskId?(focused.data?[focused.data]:[]):query.data?.tasks??[];
  return <>
    <PageHeader title={focusedTaskId?"Карточка задачи":initialDealId?"Задачи сделки":"Задачи"}
      description={initialDealId?"Все задачи, связанные с конкретной сделкой.":"Работа по клиентам, назначение участников и задачи из сделок."}
      actions={focusedTaskId
        ?<a href={scopeHref("tasks",initialAccountId||focused.data?.accountId||"")}>К задачам клиента</a>
        :initialDealId?<><a href={dealHref(initialDealId)}>К сделке</a>{" "}
          {isAdmin&&<Button disabled={mutation.busy||mutation.pending||!dealContext.data} onClick={()=>{
            const deal=dealContext.data;
            if(!deal)return;
            setSelected(undefined);setFormAccountId(deal.accountId);setEditorMode("create");mutation.changed();
          }}>Новая задача сделки</Button>}</>
        :isAdmin?<Button disabled={mutation.busy||mutation.pending} onClick={()=>{setSelected(undefined);setFormAccountId(accountId);setEditorMode("create");mutation.changed();}}>Новая задача</Button>:undefined}/>
    <p><a href="#team">Команда, сотрудники и нагрузка</a></p>
    <QueryNotice query={me}/>
    {initialDealId&&!focusedTaskId&&<>
      <QueryNotice query={dealContext}/>
      {dealContext.data&&<p>Сделка: <strong>{dealContext.data.name}</strong> ·
        Клиент: <AccountName accountId={dealContext.data.accountId}/></p>}
      <p>Задачи этой сделки могут быть созданы вручную или автоматически при смене этапа.
        Общие задачи клиента — в <a href={scopeHref("tasks",initialAccountId||dealContext.data?.accountId||"")}>списке задач клиента</a>.</p>
    </>}
    {!focusedTaskId&&<>
    <SearchField label="Поиск задач" submitted={initialSearch}
      searchLabel="Найти задачу по названию" resetLabel="Сбросить поиск задач"
      onSearch={value=>{if(navigateTo(taskSearchHref(accountId,value,initialDealId,initialOverdue,
        initialAssigneeEmail,initialMine,initialTeamId)))setCursor("");}}/>
    <p><label><input type="checkbox" aria-label="Только просроченные задачи" checked={initialOverdue}
      onChange={event=>{if(navigateTo(taskSearchHref(accountId,initialSearch,initialDealId,event.target.checked,
        initialAssigneeEmail,initialMine,initialTeamId)))setCursor("");}}/>
      {" "}Только просроченные задачи</label></p>
    {initialOverdue&&<p>Показаны только открытые задачи с истёкшим сроком.</p>}
    <QueryNotice query={membersQuery}/><QueryNotice query={teamsQuery}/>
    <SelectField label="Ответственный: фильтр" value={initialMine?"__mine":initialTeamId?"team:"+initialTeamId:initialAssigneeEmail}
      onChange={event=>{
        const next=event.target.value;
        if(navigateTo(taskSearchHref(accountId,initialSearch,initialDealId,initialOverdue,
          next==="__mine"||next.startsWith("team:")?"":next,next==="__mine",next.startsWith("team:")?next.slice(5):"")))setCursor("");
      }}>
      <option value="">Все ответственные</option>
      <option value="__mine">Назначено мне по подписанной сессии</option>
      {members.map(m=><option value={m.email} key={m.email}>
        {m.email}{!m.active?" (неактивен)":""}</option>)}
      {teams.map(team=><option key={team.teamId} value={"team:"+team.teamId}>Команда: {team.name}{team.active?"":" (архив)"}</option>)}
    </SelectField>
    {!initialDealId&&<AccountPicker label="Фильтр по клиенту" value={accountId} required={false}
      onSelect={id=>{setAccountId(id);setCursor("");}}/>}
    {!initialOverdue&&<SelectField label="Статус задачи" value={status} onChange={e=>{setStatus(e.target.value);setCursor("");}}>
      <option value="">Все</option><option value={TaskStatus.OPEN}>Открытые</option><option value={TaskStatus.COMPLETED}>Выполненные</option>
    </SelectField>}
    <SelectField label="Приоритет задач" value={priority} onChange={e=>{setPriority(e.target.value);setCursor("");}}>
      <option value="">Все приоритеты</option><option value={TaskPriority.LOW}>Низкий</option><option value={TaskPriority.NORMAL}>Обычный</option><option value={TaskPriority.HIGH}>Высокий</option><option value={TaskPriority.URGENT}>Срочный</option>
    </SelectField></>}
    <QueryNotice query={focusedTaskId?focused:query}/>
    {visibleTasks.map(t=><Card title={t.subject} key={t.taskId}>
      {focusedTaskId&&<p>ID: {t.taskId} · версия {t.version.toString()}</p>}
      <p>{t.description}</p><p>Клиент: {focusedTaskId?<AccountName accountId={t.accountId}/>:<a href={accountHref(t.accountId)}>{query.data?.accountNames[t.accountId]}</a>}</p>
      <p>Срок: {dateText(t.dueAt)} · {t.status===TaskStatus.COMPLETED?"Выполнена":t.status===TaskStatus.OPEN?"Открыта":"Неизвестный статус"}
        {t.status===TaskStatus.OPEN&&t.dueAt&&
          Number(t.dueAt.seconds)*1000+Math.floor(t.dueAt.nanos/1000000)<Date.now()&&
          <> · <strong>Просрочена</strong></>}</p>
      <p>Напоминание: {dateText(t.reminderAt)}</p>
      <p>Ответственный: {t.assigneeIdentity?
        (humanMember(t.assigneeIdentity)?t.assigneeIdentity:"Технический агент") :
        t.ownerTeamId?"Команда: "+(teams.find(team=>team.teamId===t.ownerTeamId)?.name??t.ownerTeamId):"неизвестен"}</p>
      {t.relatedDealId&&<p>Связанная сделка: <a href={dealHref(t.relatedDealId)}>Открыть сделку</a></p>}
      {!focusedTaskId&&<p><a href={taskHref(t.taskId,t.accountId)}>Открыть карточку задачи</a></p>}
      {canClaim(t)&&t.status===TaskStatus.OPEN&&<Button disabled={mutation.busy||mutation.pending}
        onClick={()=>void mutation.runIntent(()=>prepareMutation(api.tasks.claimTask,{
          taskId:t.taskId,precondition:{idempotencyKey:mutation.key,expectedVersion:t.version}
        },t.taskId),async()=>{await query.reload();await focused.reload();})}>Забрать задачу себе</Button>}{" "}
      {isAdmin&&<Button variant="secondary" disabled={mutation.busy||mutation.pending} onClick={async()=>{
        try {
          setSelected((await api.tasks.getTask({taskId:t.taskId})).task);
          setEditorMode("edit");mutation.changed();
        } catch(error) {
          await mutation.reportError(error);
        }
      }}>Редактировать задачу</Button>}{" "}
      {canWorkOn(t)&&t.status===TaskStatus.OPEN&&<Button variant="secondary" disabled={mutation.busy||mutation.pending}
        onClick={async()=>{
          try {
            const task=(await api.tasks.getTask({taskId:t.taskId})).task;
            if(!task||task.status!==TaskStatus.OPEN)throw Error("Задача уже завершена");
            setSelected(task);setEditorMode("reminder");mutation.changed();
          }catch(error){await mutation.reportError(error);}
        }}>{t.reminderAt?"Изменить напоминание":"Назначить напоминание"}</Button>}{" "}
      {canWorkOn(t)&&t.status===TaskStatus.OPEN&&<Button disabled={mutation.busy||mutation.pending}
        onClick={()=>void mutation.runIntent(()=>prepareMutation(api.tasks.completeTask,{
          taskId:t.taskId,
          precondition:{idempotencyKey:mutation.key,expectedVersion:t.version}
        },t.taskId),async()=>{await query.reload();await focused.reload();})}>Выполнить задачу</Button>}
    </Card>)}
    {!focusedTaskId&&<>
      {!query.busy&&query.data?.tasks.length===0&&<p>{initialOverdue
        ?"Просроченных задач нет.":initialDealId
        ?"По этой сделке пока нет связанных задач.":"Задач пока нет."}</p>}
      <Paging next={query.data?.pageInfo?.nextPageToken??""} busy={query.busy}
        onFirst={()=>setCursor("")} onNext={()=>setCursor(query.data?.pageInfo?.nextPageToken??"")}/>
    </>}
    <RecordPanel open={editorMode==="reminder"&&!!selected&&canWorkOn(selected)} title="Напоминание задачи"
      onClose={()=>{if(mutation.busy||mutation.pending)return;setEditorMode(null);setSelected(undefined);}}>
      {selected&&<form key={selected.taskId+":"+selected.version.toString()} onChange={mutation.changed}
        onSubmit={e=>{
          e.preventDefault();const data=new FormData(e.currentTarget);
          void mutation.runIntent(()=>{
            const reminderAt=timestamp(field(data,"scheduleReminderAt"));
            if(!reminderAt)throw Error("Укажите время напоминания");
            return prepareMutation(api.tasks.scheduleReminder,{
              taskId:selected.taskId,reminderAt,
              precondition:{idempotencyKey:mutation.key,expectedVersion:selected.version}
            },selected.taskId);
          },async()=>{setSelected(undefined);setEditorMode(null);
            await query.reload();await focused.reload();});
        }}>
        <p>Напоминание задачи: {dateText(selected.reminderAt)} · срок: {dateText(selected.dueAt)}.</p>
        <p>Выберите будущую дату. Напоминание не должно быть позднее срока задачи.
          Для уже назначенного времени его можно перенести; завершение задачи сбрасывает напоминание.</p>
        <TextField label="Время напоминания" name="scheduleReminderAt" type="datetime-local" step="1" required/>
        <Button type="submit" disabled={mutation.busy||mutation.pending}>Сохранить напоминание</Button>{" "}
        <Button type="button" variant="secondary" disabled={mutation.busy||mutation.pending}
          onClick={()=>{setEditorMode(null);setSelected(undefined);mutation.changed();}}>Отмена</Button>
      </form>}
      <MutationNotice state={mutation}/>
    </RecordPanel>
    <RecordPanel open={isAdmin&&editorMode==="edit"&&!!selected} title="Изменить задачу" onClose={()=>{if(mutation.busy||mutation.pending)return;setEditorMode(null);setSelected(undefined);}}>{selected&&<>
      <p>Текущий срок: {dateText(selected.dueAt)} · Напоминание: {dateText(selected.reminderAt)}</p>
      <form key={selected.taskId+selected.version.toString()} onChange={mutation.changed} onSubmit={e=>{
        e.preventDefault();
        const data=new FormData(e.currentTarget);
        void mutation.runIntent(()=>{
          const description=field(data,"editDescription"),dueAt=timePatch(data,"editDue");
          return prepareMutation(api.tasks.updateTask,{
            taskId:selected.taskId,
            subject:{operation:{case:"set",value:field(data,"editSubject")}},
            description:{operation:description
              ?{case:"set",value:description}:{case:"clear",value:true}},
            priority:enumValue(field(data,"editPriority"),[TaskPriority.LOW,TaskPriority.NORMAL,TaskPriority.HIGH,TaskPriority.URGENT],"Приоритет задачи"),
            ...ownerFields(field(data,"editAssigneeEmail")),
            ...(dueAt?{dueAt}:{}),
            precondition:{idempotencyKey:mutation.key,expectedVersion:selected.version}
          },selected.taskId);
        },async()=>{setSelected(undefined);setEditorMode(null);
          await query.reload();await focused.reload();});
      }}><FormGrid>
        <TextField label="Тема задачи: изменение" name="editSubject" defaultValue={selected.subject} required/>
        <TextField label="Описание задачи: изменение" name="editDescription" defaultValue={selected.description}/>
        <SelectField label="Приоритет: изменение" name="editPriority" defaultValue={selected.priority}>
          <option value={TaskPriority.LOW}>Низкий</option><option value={TaskPriority.NORMAL}>Обычный</option>
          <option value={TaskPriority.HIGH}>Высокий</option><option value={TaskPriority.URGENT}>Срочный</option>
        </SelectField>
        <SelectField label="Переназначить задачу" name="editAssigneeEmail" defaultValue="">
          <option value="">Оставить действующего ответственного</option>
          {members.filter(m=>m.active).map(m=><option value={m.email} key={m.email}>
            {m.email}</option>)}
          {eligibleTeams.map(team=><option value={"team:"+team.teamId} key={team.teamId}>Команда: {team.name}</option>)}
        </SelectField>
        <TimeFields name="editDue" label="Срок задачи" editing/>
      </FormGrid>
        <Button type="submit" disabled={mutation.busy||mutation.pending}>Сохранить изменения задачи</Button>{" "}
        <Button type="button" variant="secondary" disabled={mutation.busy||mutation.pending} onClick={()=>{setSelected(undefined);setEditorMode(null);mutation.changed();}}>Отмена</Button>
      </form><MutationNotice state={mutation}/>
    </>}</RecordPanel>
    <RecordPanel open={isAdmin&&editorMode==="create"} title="Новая задача" onClose={()=>{if(!mutation.busy&&!mutation.pending)setEditorMode(null);}}><form key={draftId} onChange={mutation.changed} onSubmit={e=>{
      e.preventDefault();const data=new FormData(e.currentTarget);
      void mutation.runIntent(()=>{
        const dueAt=timestamp(field(data,"due")),reminderAt=timestamp(field(data,"reminder"));
        const description=field(data,"description");
        const targetAccount=initialDealId?dealContext.data?.accountId:field(data,"accountId");
        if(!targetAccount)throw Error("Выберите клиента задачи");
        return prepareMutation(api.tasks.createTask,{taskId:draftId,subject:field(data,"subject"),accountId:targetAccount,
          ...(initialDealId?{dealId:initialDealId}:{}),
          priority:enumValue(field(data,"priority"),[TaskPriority.LOW,TaskPriority.NORMAL,TaskPriority.HIGH,TaskPriority.URGENT],"Приоритет задачи"),...(description?{description}:{}),
          ...ownerFields(field(data,"assigneeEmail")),
          ...(dueAt?{dueAt}:{}),...(reminderAt?{reminderAt}:{}),precondition:{idempotencyKey:mutation.key}},draftId);
      },async()=>{setEditorMode(null);setDraftId(newId());await query.reload();});
    }}><FormGrid>
      <TextField label="Тема задачи" name="subject" required/>{initialDealId
        ?<p>Клиент сделки: <AccountName accountId={dealContext.data?.accountId||""}/></p>
        :<AccountPicker value={formAccountId} onSelect={setFormAccountId}/>}
      <TextField label="Описание" name="description"/>
      <SelectField label="Приоритет" name="priority" defaultValue="2">
        <option value={TaskPriority.LOW}>Низкий</option><option value={TaskPriority.NORMAL}>Обычный</option><option value={TaskPriority.HIGH}>Высокий</option><option value={TaskPriority.URGENT}>Срочный</option>
      </SelectField>
      <SelectField label="Назначить сотруднику" name="assigneeEmail" defaultValue={initialTeamId&&eligibleTeams.some(team=>team.teamId===initialTeamId)?"team:"+initialTeamId:""}>
        <option value="">Создатель — действующий подписанный актор</option>
        {members.filter(m=>m.active).map(m=><option value={m.email} key={m.email}>
          {m.email}</option>)}
        {eligibleTeams.map(team=><option value={"team:"+team.teamId} key={team.teamId}>Команда: {team.name}</option>)}
      </SelectField>
      <TextField label="Срок" name="due" type="datetime-local" step="1"/>
      <TextField label="Напоминание" name="reminder" type="datetime-local" step="1"/>
    </FormGrid><Button type="submit" disabled={mutation.busy||mutation.pending}>Создать задачу</Button></form>
      <MutationNotice state={mutation}/>
    </RecordPanel>
    {editorMode===null&&<MutationNotice state={mutation}/>}
  </>;
}
