import {useCallback,useState} from "react";
import {Button,Card,RecordPanel,FormGrid,PageHeader,SelectField,TextField} from "@ultimate-crm/ui";
import type {Task} from "@ultimate-crm/client/cloudflare";
import {api,field,newId,timestamp,dateText} from "./api";
import {TaskPriority,TaskStatus} from "@ultimate-crm/client/cloudflare";
import {enumValue} from "./enumFields";
import {useQuery,useMutation,MutationNotice,QueryNotice,Paging} from "./hooks";
import {AccountPicker} from "./Pickers";
import {TimeFields} from "./TimeFields";
import {timePatch} from "./timePatch";

export function TaskScreen({initialAccountId=""}:{initialAccountId?:string}) {
  const [cursor,setCursor]=useState("");
  const [status,setStatus]=useState("");
  const [priority,setPriority]=useState("");
  const [accountId,setAccountId]=useState(initialAccountId);
  const [draftId,setDraftId]=useState(newId);
  const [selected,setSelected]=useState<Task>();
  const [editorMode,setEditorMode]=useState<"create"|"edit"|null>(null);
  const mutation=useMutation();
  const query=useQuery(useCallback(()=>api.tasks.listTasks({page:{pageSize:25,pageToken:cursor},
    ...(accountId?{accountId}:{}),...(status?{status:enumValue(status,[TaskStatus.OPEN,TaskStatus.COMPLETED],"Статус задачи")}:{}),...(priority?{priority:enumValue(priority,[TaskPriority.LOW,TaskPriority.NORMAL,TaskPriority.HIGH,TaskPriority.URGENT],"Приоритет задачи")}:{})}),[cursor,status,priority,accountId]));
  return <>
    <PageHeader title="Задачи" description="Работа по клиентам и задачи, созданные из сделок." actions={<Button onClick={()=>{setSelected(undefined);setEditorMode("create");mutation.changed();}}>Новая задача</Button>}/>
    <TextField label="Фильтр по ID клиента" value={accountId} onChange={e=>{setAccountId(e.target.value);setCursor("");}}/>
    <SelectField label="Статус задачи" value={status} onChange={e=>{setStatus(e.target.value);setCursor("");}}>
      <option value="">Все</option><option value={TaskStatus.OPEN}>Открытые</option><option value={TaskStatus.COMPLETED}>Выполненные</option>
    </SelectField>
    <SelectField label="Приоритет задач" value={priority} onChange={e=>{setPriority(e.target.value);setCursor("");}}>
      <option value="">Все приоритеты</option><option value={TaskPriority.LOW}>Низкий</option><option value={TaskPriority.NORMAL}>Обычный</option><option value={TaskPriority.HIGH}>Высокий</option><option value={TaskPriority.URGENT}>Срочный</option>
    </SelectField><QueryNotice query={query}/>
    {query.data?.tasks.map(t=><Card title={t.subject} key={t.taskId}>
      <p>{t.description}</p><p>Клиент: {t.accountId}</p>
      <p>Срок: {dateText(t.dueAt)} · {t.status===TaskStatus.COMPLETED?"Выполнена":t.status===TaskStatus.OPEN?"Открыта":"Неизвестный статус"}</p>
      <p>Напоминание: {dateText(t.reminderAt)}</p>
      <Button variant="secondary" disabled={mutation.busy} onClick={async()=>{
        try {
          setSelected((await api.tasks.getTask({taskId:t.taskId})).task);
          setEditorMode("edit");mutation.changed();
        } catch(error) {
          await mutation.run(()=>Promise.reject(error));
        }
      }}>Редактировать задачу</Button>{" "}
      {t.status===TaskStatus.OPEN&&<Button disabled={mutation.busy} onClick={()=>void mutation.run(async()=>{
        const fresh=await api.tasks.getTask({taskId:t.taskId});
        if(!fresh.task)throw Error("Задача не найдена");
        return api.tasks.completeTask({taskId:t.taskId,
          precondition:{idempotencyKey:mutation.key,expectedVersion:t.version}});
      },query.reload)}>Выполнить задачу</Button>}
    </Card>)}
    {!query.busy&&query.data?.tasks.length===0&&<p>Задач пока нет.</p>}
    <Paging next={query.data?.pageInfo?.nextPageToken??""} busy={query.busy} onFirst={()=>setCursor("")} onNext={()=>setCursor(query.data?.pageInfo?.nextPageToken??"")}/>
    <RecordPanel open={editorMode==="edit"&&!!selected} title="Изменить задачу" onClose={()=>{setEditorMode(null);setSelected(undefined);}}>{selected&&<>
      <p>Текущий срок: {dateText(selected.dueAt)} · Напоминание: {dateText(selected.reminderAt)}</p>
      <form key={selected.taskId+selected.version.toString()} onChange={mutation.changed} onSubmit={e=>{
        e.preventDefault();
        const data=new FormData(e.currentTarget);
        void mutation.run(()=>{
          const description=field(data,"editDescription"),dueAt=timePatch(data,"editDue");
          return api.tasks.updateTask({
            taskId:selected.taskId,
            subject:{operation:{case:"set",value:field(data,"editSubject")}},
            description:{operation:description
              ?{case:"set",value:description}:{case:"clear",value:true}},
            priority:enumValue(field(data,"editPriority"),[TaskPriority.LOW,TaskPriority.NORMAL,TaskPriority.HIGH,TaskPriority.URGENT],"Приоритет задачи"),
            ...(dueAt?{dueAt}:{}),
            precondition:{idempotencyKey:mutation.key,expectedVersion:selected.version}
          });
        },async()=>{setSelected(undefined);setEditorMode(null);await query.reload();});
      }}><FormGrid>
        <TextField label="Тема задачи: изменение" name="editSubject" defaultValue={selected.subject} required/>
        <TextField label="Описание задачи: изменение" name="editDescription" defaultValue={selected.description}/>
        <SelectField label="Приоритет: изменение" name="editPriority" defaultValue={selected.priority}>
          <option value={TaskPriority.LOW}>Низкий</option><option value={TaskPriority.NORMAL}>Обычный</option>
          <option value={TaskPriority.HIGH}>Высокий</option><option value={TaskPriority.URGENT}>Срочный</option>
        </SelectField>
        <TimeFields name="editDue" label="Срок задачи" editing/>
      </FormGrid>
        <Button type="submit" disabled={mutation.busy}>Сохранить изменения задачи</Button>{" "}
        <Button variant="secondary" onClick={()=>{setSelected(undefined);setEditorMode(null);mutation.changed();}}>Отмена</Button>
      </form>{mutation.notice?.tone==="danger"&&<MutationNotice state={mutation}/>}
    </>}</RecordPanel>
    <RecordPanel open={editorMode==="create"} title="Новая задача" onClose={()=>setEditorMode(null)}><form key={draftId} onChange={mutation.changed} onSubmit={e=>{
      e.preventDefault();const data=new FormData(e.currentTarget);
      void mutation.run(()=>{
        const dueAt=timestamp(field(data,"due")),reminderAt=timestamp(field(data,"reminder"));
        const description=field(data,"description");
        return api.tasks.createTask({taskId:draftId,subject:field(data,"subject"),accountId:field(data,"accountId"),
          priority:enumValue(field(data,"priority"),[TaskPriority.LOW,TaskPriority.NORMAL,TaskPriority.HIGH,TaskPriority.URGENT],"Приоритет задачи"),...(description?{description}:{}),
          ...(dueAt?{dueAt}:{}),...(reminderAt?{reminderAt}:{}),precondition:{idempotencyKey:mutation.key}});
      },async()=>{setEditorMode(null);setDraftId(newId());await query.reload();});
    }}><FormGrid>
      <TextField label="Тема задачи" name="subject" required/><AccountPicker/>
      <TextField label="Описание" name="description"/>
      <SelectField label="Приоритет" name="priority" defaultValue="2">
        <option value={TaskPriority.LOW}>Низкий</option><option value={TaskPriority.NORMAL}>Обычный</option><option value={TaskPriority.HIGH}>Высокий</option><option value={TaskPriority.URGENT}>Срочный</option>
      </SelectField>
      <TextField label="Срок" name="due" type="datetime-local" step="1"/>
      <TextField label="Напоминание" name="reminder" type="datetime-local" step="1"/>
    </FormGrid><Button type="submit" disabled={mutation.busy}>Создать задачу</Button></form>
      {mutation.notice?.tone==="danger"&&<MutationNotice state={mutation}/>}
    </RecordPanel>
    <MutationNotice state={mutation}/>
  </>;
}
