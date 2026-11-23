import {useCallback,useState} from "react";
import {Button,SelectField} from "@ultimate-crm/ui";
import {InteractionKind} from "@ultimate-crm/client/cloudflare";
import type {Interaction} from "@ultimate-crm/client/cloudflare";
import {api,field,newId,timestamp,dateText} from "./api";
import {enumValue} from "./enumFields";
import {useQuery,useMutation,prepareMutation,QueryNotice,MutationNotice} from "./hooks";
import {dealHref} from "./routes";

function localTime():string {
  const now=new Date();
  return new Date(now.getTime()-now.getTimezoneOffset()*60_000).toISOString().slice(0,16);
}
const kinds=[
  [InteractionKind.CALL,"Звонок"],
  [InteractionKind.MEETING,"Встреча"],
  [InteractionKind.NOTE,"Заметка"],
] as const;
function label(kind:InteractionKind):string {
  return kinds.find(([value])=>kind===value)?.[1]??"Неизвестный вид";
}

/** Read and write the same immutable Activities Interaction owner from Account/Deal. */
export function InteractionTimeline({accountId,dealId=""}:{accountId:string;dealId?:string}) {
  const [cursor,setCursor]=useState("");
  const [interactionId,setInteractionId]=useState(newId);
  const [text,setText]=useState("");
  const [when,setWhen]=useState(localTime);
  const history=useQuery(useCallback(
    (signal:AbortSignal)=>api.interactions.listInteractions({
      accountId,dealId,page:{pageSize:25,pageToken:cursor}
    },{signal}),[accountId,dealId,cursor]
  ));
  const mutation=useMutation({
    storageKey:"crm.interaction.create.pending.v1",
    lookup:async({resourceId,key},signal)=>(await api.interactions.getInteraction({
      interactionId:resourceId,receiptIdempotencyKey:key
    },{signal})).confirmedMutation,
    refresh:()=>history.reload()
  });
  const next=history.data?.pageInfo?.nextPageToken??"";
  const items:readonly Interaction[]=history.data?.interactions??[];
  return <section aria-label={"История взаимодействий "+(dealId||accountId)}>
    <h4>История взаимодействий</h4>
    <p>Фактические звонки, встречи и заметки. История сортируется по времени события.</p>
    <QueryNotice query={history}/>
    <ol>{items.map(item=><li key={item.interactionId}>
      <strong>{label(item.kind)}</strong>{" · "}{dateText(item.occurredAt)}
      <p>{item.text}</p>
      <p>Автор: {item.author} · записано: {dateText(item.recordedAt)}
        {!dealId&&item.dealId?<> · <a href={dealHref(item.dealId,accountId)}>Связанная сделка</a></>:null}
      </p>
    </li>)}</ol>
    {!history.busy&&!history.error&&items.length===0&&<p>Записей на этой странице нет.</p>}
    {next&&<Button type="button" variant="secondary" disabled={history.busy}
      onClick={()=>setCursor(next)}>Ранее</Button>}
    {cursor&&<>{" "}<Button type="button" variant="secondary" disabled={history.busy}
      onClick={()=>setCursor("")}>К последним</Button></>}
    <form aria-label={"Записать взаимодействие "+(dealId||accountId)} onSubmit={event=>{
      event.preventDefault();
      const data=new FormData(event.currentTarget);
      void mutation.runIntent(()=>prepareMutation(api.interactions.createInteraction,{
        interactionId,accountId,dealId,
        kind:enumValue(field(data,"kind"),[
          InteractionKind.CALL,InteractionKind.MEETING,InteractionKind.NOTE
        ],"Вид взаимодействия"),
        text:field(data,"text"),
        occurredAt:timestamp(field(data,"occurredAt")),
        precondition:{idempotencyKey:mutation.key},
      },interactionId),async()=>{
        setCursor("");
        await history.reload();
        setInteractionId(newId());
        setText("");
        setWhen(localTime());
      });
    }}>
      <SelectField label="Вид взаимодействия" name="kind" defaultValue={InteractionKind.NOTE}>
        {kinds.map(([id,title])=><option key={id} value={id}>{title}</option>)}
      </SelectField>
      <label>Время события <input name="occurredAt" type="datetime-local" required
        value={when} onChange={event=>{setWhen(event.target.value);mutation.changed();}}/></label>
      <label>Что произошло <textarea name="text" value={text} required maxLength={16384}
        rows={3} onChange={event=>{setText(event.target.value);mutation.changed();}}/></label>
      <Button type="submit" disabled={mutation.busy||mutation.pending}>Добавить в историю</Button>
    </form>
    <MutationNotice state={mutation}/>
  </section>;
}
