import {useState} from "react";
import {Button,TextField} from "@ultimate-crm/ui";
import type {Member,Team} from "@ultimate-crm/client/cloudflare";
import {api,newId} from "./api";
import {useMutation,prepareMutation,MutationNotice,QueryNotice} from "./hooks";
import type {useQuery} from "./hooks";
import {humanMember} from "./memberCatalog";
import {taskTeamHref,workdayHref,salesPipelineHref,intelligenceHref} from "./routes";

export function TeamWorkspace({query,members,isAdmin,actor,onChanged}:{
  query:ReturnType<typeof useQuery<Team[]>>;
  members:Member[];isAdmin:boolean;actor:string;onChanged:()=>Promise<unknown>;
}){
  const [editing,setEditing]=useState<Team>();
  const [creating,setCreating]=useState(false);
  const [draftId,setDraftId]=useState(newId);
  const [name,setName]=useState("");
  const [description,setDescription]=useState("");
  const [active,setActive]=useState(true);
  const [roster,setRoster]=useState<string[]>([]);
  const [search,setSearch]=useState("");
  const mutation=useMutation({
    storageKey:"crm.team.pending.v1",
    lookup:async({resourceId,key},signal)=>(await api.members.getTeam({
      teamId:resourceId,receiptIdempotencyKey:key},{signal})).confirmedMutation,
    refresh:async()=>{await query.reload();await onChanged();}
  });
  const pending=mutation.busy||mutation.pending;
  const close=()=>{setEditing(undefined);setCreating(false);};
  const start=(team?:Team)=>{
    setEditing(team);setCreating(!team);setDraftId(newId());
    setName(team?.name??"");setDescription(team?.description??"");
    setActive(team?.active??true);setRoster(team?.memberEmails??[]);mutation.changed();
  };
  const human=members.filter(m=>humanMember(m.email));
  const filtered=(query.data??[]).filter(t=>
    !search.trim()||t.name.toLocaleLowerCase().includes(search.trim().toLocaleLowerCase()));
  const rosterChanged=JSON.stringify([...roster].sort())!==JSON.stringify([...(editing?.memberEmails??[])].sort());
  return <section aria-label="Организационные команды">
    <h2>Организационные команды</h2>
    <p>Сотрудник может состоять в нескольких командах. После получения из общего пула задача становится личной. Архивирование команды с открытой работой запрещено.</p>
    <QueryNotice query={query}/>
    <TextField label="Поиск команд" value={search} onChange={e=>setSearch(e.target.value)}/>
    {isAdmin&&<Button disabled={pending} onClick={()=>start()}>Создать команду</Button>}
    {!query.busy&&filtered.length===0&&<p>Команды не найдены.</p>}
    <div role="region" aria-label="Организационные команды — таблица" tabIndex={0}
      style={{maxWidth:"100%",overflowX:"auto"}}>
      <table><thead><tr><th scope="col">Команда</th><th scope="col">Состояние</th>
        <th scope="col">Состав</th><th scope="col">Очереди</th><th scope="col">Управление</th>
      </tr></thead><tbody>{filtered.map(team=><tr key={team.teamId}>
        <th scope="row">{team.name}{team.memberEmails.includes(actor)?" · моя команда":""}
          {team.description&&<p>{team.description}</p>}</th>
        <td>{team.active?"Активна":"Архив"}</td>
        <td><ul>{team.memberEmails.map(email=><li key={email}>{email}
          {human.find(m=>m.email===email)?.active?"":" · неактивен"}</li>)}</ul></td>
        <td><a href={taskTeamHref(team.teamId)}>Задачи команды</a>{" · "}
          <a href={workdayHref("",false,team.teamId)}>Рабочий день команды</a>{" · "}
          <a href={salesPipelineHref("","","","team:"+team.teamId)}>Сделки команды</a>{" · "}
          <a href={intelligenceHref({owner:"team:"+team.teamId})}>Портфель команды</a></td>
        <td>{isAdmin&&<Button variant="secondary" disabled={pending}
          onClick={()=>start(team)}>Изменить команду</Button>}</td>
      </tr>)}</tbody></table>
    </div>
    {isAdmin&&(creating||editing)&&<form aria-label="Настройка команды" onSubmit={event=>{
      event.preventDefault();
      const target=editing;
      const id=target?.teamId??draftId;
      const sorted=[...roster].sort();
      const done=async()=>{close();await query.reload();await onChanged();};
      if(target){
        void mutation.runIntent(()=>prepareMutation(api.members.updateTeam,{
          teamId:id,...(name!==target.name?{name}:{}),
          ...(description!==target.description?{description}:{}),
          ...(active!==target.active?{active}:{}),
          ...(rosterChanged?{roster:{memberEmails:sorted}}:{}),
          precondition:{idempotencyKey:mutation.key,expectedVersion:target.version}
        },id),done);
      }else{
        void mutation.runIntent(()=>prepareMutation(api.members.createTeam,{
          teamId:id,name,description,roster:{memberEmails:sorted},
          precondition:{idempotencyKey:mutation.key}
        },id),done);
      }
    }}>
      <h3>{editing?"Редактирование команды":"Новая команда"}</h3>
      <TextField label="Название команды" value={name} onChange={e=>{setName(e.target.value);mutation.changed();}} required/>
      <TextField label="Описание команды" value={description} onChange={e=>{setDescription(e.target.value);mutation.changed();}}/>
      {editing&&<p><label><input type="checkbox" checked={active} onChange={e=>{setActive(e.target.checked);mutation.changed();}}/> Активная команда</label></p>}
      <fieldset><legend>Участники команды</legend>
        {human.filter(m=>m.active||roster.includes(m.email)).map(member=><p key={member.email}>
          <label><input type="checkbox" checked={roster.includes(member.email)}
            onChange={e=>{const checked=e.target.checked;
              setRoster(old=>checked?[...old,member.email]:old.filter(v=>v!==member.email));
              mutation.changed();}}/> {member.email}{member.active?"":" (неактивен)"}</label>
        </p>)}
      </fieldset>
      <Button type="submit" disabled={pending||!name.trim()||!!editing&&name===editing.name&&
        description===editing.description&&active===editing.active&&!rosterChanged}>
        Сохранить команду</Button>{" "}
      <Button type="button" variant="secondary" disabled={pending} onClick={close}>Отмена</Button>
    </form>}
    <MutationNotice state={mutation}/>
  </section>;
}
