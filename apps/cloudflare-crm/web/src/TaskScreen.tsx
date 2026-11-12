import {useCallback,useState} from "react";
import {Button,Card,FormGrid,PageHeader,SelectField,TextField} from "@ultimate-crm/ui";
import {api,field,newId,timestamp,dateText} from "./api";
import {useQuery,useMutation,MutationNotice,QueryNotice,Paging} from "./hooks";
import {AccountPicker} from "./Pickers";

export function TaskScreen() {
  const [cursor,setCursor]=useState("");
  const [status,setStatus]=useState("");
  const [draftId,setDraftId]=useState(newId);
  const mutation=useMutation();
  const query=useQuery(useCallback(()=>api.tasks.listTasks({page:{pageSize:25,pageToken:cursor},
    ...(status?{status:Number(status)}:{})}),[cursor,status]));
  return <>
    <PageHeader title="Задачи" description="Работа по клиентам и задачи, созданные из сделок."/>
    <SelectField label="Статус задачи" value={status} onChange={e=>{setStatus(e.target.value);setCursor("");}}>
      <option value="">Все</option><option value="1">Открытые</option><option value="2">Выполненные</option>
    </SelectField><QueryNotice query={query}/>
    {query.data?.tasks.map(t=><Card title={t.subject} key={t.taskId}>
      <p>{t.description}</p><p>Клиент: {t.accountId}</p>
      <p>Срок: {dateText(t.dueAt)} · {t.status===2?"Выполнена":"Открыта"}</p>
      <p>Напоминание: {dateText(t.reminderAt)}</p>
      {t.status===1&&<Button disabled={mutation.busy} onClick={()=>void mutation.run(async()=>{
        const fresh=await api.tasks.getTask({taskId:t.taskId});
        if(!fresh.task)throw Error("Задача не найдена");
        return api.tasks.completeTask({taskId:t.taskId,
          precondition:{idempotencyKey:mutation.key,expectedVersion:t.version}});
      },query.reload)}>Выполнить задачу</Button>}
    </Card>)}
    {!query.busy&&query.data?.tasks.length===0&&<p>Задач пока нет.</p>}
    <Paging next={query.data?.pageInfo?.nextPageToken??""} busy={query.busy} onFirst={()=>setCursor("")} onNext={()=>setCursor(query.data?.pageInfo?.nextPageToken??"")}/>
    <Card title="Новая задача"><form key={draftId} onChange={mutation.changed} onSubmit={e=>{
      e.preventDefault();const data=new FormData(e.currentTarget);
      void mutation.run(()=>{
        const dueAt=timestamp(field(data,"due")),reminderAt=timestamp(field(data,"reminder"));
        const description=field(data,"description");
        return api.tasks.createTask({taskId:draftId,subject:field(data,"subject"),accountId:field(data,"accountId"),
          priority:Number(field(data,"priority")),...(description?{description}:{}),
          ...(dueAt?{dueAt}:{}),...(reminderAt?{reminderAt}:{}),precondition:{idempotencyKey:mutation.key}});
      },async()=>{setDraftId(newId());await query.reload();});
    }}><FormGrid>
      <TextField label="Тема задачи" name="subject" required/><AccountPicker/>
      <TextField label="Описание" name="description"/>
      <SelectField label="Приоритет" name="priority" defaultValue="2">
        <option value="1">Низкий</option><option value="2">Обычный</option><option value="3">Высокий</option><option value="4">Срочный</option>
      </SelectField>
      <TextField label="Срок" name="due" type="datetime-local" step="1"/>
      <TextField label="Напоминание" name="reminder" type="datetime-local" step="1"/>
    </FormGrid><Button type="submit" disabled={mutation.busy}>Создать задачу</Button></form></Card>
    <MutationNotice state={mutation}/>
  </>;
}
