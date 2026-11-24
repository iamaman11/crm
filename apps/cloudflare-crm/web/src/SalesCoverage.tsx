import {useCallback,useState} from "react";
import {Button,SelectField,TextField} from "@ultimate-crm/ui";
import {MemberRole,SalesOverlayRole} from "@ultimate-crm/client/cloudflare";
import type {SalesTerritory,SalesAccountAssignment} from "@ultimate-crm/client/cloudflare";
import {api,newId} from "./api";
import {useMutation,useQuery,prepareMutation,MutationNotice,QueryNotice} from "./hooks";
import {allOwners,ownerLabel,ownerValue,ownerWire} from "./ownerCatalog";
import {AccountName} from "./Pickers";
import {SalesRoutingWorkspace} from "./SalesRouting";
import {SalesQuotaWorkspace} from "./SalesQuotas";
import type {ScorecardFilters} from "./routes";

type Choices=Awaited<ReturnType<typeof allOwners>>["choices"];
export async function allTerritories(signal:AbortSignal):Promise<SalesTerritory[]> {
  const result:SalesTerritory[]=[];
  let cursor="";
  do {
    const page=await api.deals.listTerritories({includeArchived:true,page:{pageSize:100,pageToken:cursor}},{signal});
    result.push(...page.territories);
    cursor=page.pageInfo?.nextPageToken??"";
  }while(cursor);
  return result;
}
const territoryLabel=(items:SalesTerritory[],id:string|undefined)=>
  id?(items.find(t=>t.territoryId===id)?.configuration?.name??id):"Без территории";
const overlayLabel=(role:SalesOverlayRole)=>role===SalesOverlayRole.SPECIALIST?"Специалист":
  role===SalesOverlayRole.ACCOUNT_SUPPORT?"Сопровождение":
  role===SalesOverlayRole.EXECUTIVE_SPONSOR?"Куратор":"Неизвестная роль";
function OwnerOptions({choices,current=""}:{choices:Choices;current?:string}) {
  return <><option value="">Выберите ответственного</option>
    {choices.filter(c=>c.eligible||c.value===current).map(c=>
      <option key={c.value} value={c.value} disabled={!c.eligible}>{c.label}</option>)}</>;
}
function TerritoryOptions({items,current="",exclude=""}:{items:SalesTerritory[];current?:string;exclude?:string}) {
  return <><option value="">Без территории</option>
    {items.filter(t=>t.territoryId!==exclude&&(!t.configuration?.archived||t.territoryId===current)).map(t=>
      <option key={t.territoryId} value={t.territoryId} disabled={t.configuration?.archived}>
        {t.configuration?.name??t.territoryId}{t.configuration?.archived?" (архив)":""}
      </option>)}</>;
}

export function TerritoryWorkspace({isAdmin,choices,ownersUnavailable,reportId,scorecard}:{
  isAdmin:boolean;choices:Choices;ownersUnavailable:boolean;reportId:string;scorecard:ScorecardFilters|undefined;
}) {
  const territories=useQuery(useCallback(allTerritories,[]));
  const [selected,setSelected]=useState("");
  const [ownerFilter,setOwnerFilter]=useState("");
  const [cursor,setCursor]=useState("");
  const [includeArchived,setIncludeArchived]=useState(false);
  const assignments=useQuery(useCallback((signal:AbortSignal)=>{
    const primaryOwnerFilter=ownerWire(ownerFilter);
    return api.deals.listSalesAccountAssignments({
      page:{pageSize:25,pageToken:cursor},includeArchived,
      ...(selected?{territoryId:selected}:{}),
      ...(primaryOwnerFilter?{primaryOwnerFilter}:{})
    },{signal});
  },[selected,ownerFilter,cursor,includeArchived]));
  const items=territories.data??[];
  const [editing,setEditing]=useState<SalesTerritory>();
  const [creating,setCreating]=useState(false);
  const [id,setId]=useState<SalesTerritory["territoryId"]>(newId);
  const [name,setName]=useState("");
  const [parent,setParent]=useState("");
  const [owner,setOwner]=useState("");
  const [archived,setArchived]=useState(false);
  const close=()=>{setEditing(undefined);setCreating(false);};
  const mutation=useMutation({
    storageKey:"crm.sales.territory.pending.v1",
    lookup:async({resourceId,key},signal)=>(await api.deals.getTerritory({
      territoryId:resourceId,receiptIdempotencyKey:key},{signal})).confirmedMutation,
    refresh:async()=>{close();await territories.reload();await assignments.reload();}
  });
  const pending=mutation.busy||mutation.pending;
  const start=(value?:SalesTerritory)=>{
    setEditing(value);setCreating(!value);setId(value?.territoryId??newId());
    setName(value?.configuration?.name??"");setParent(value?.configuration?.parentId??"");
    setOwner(ownerValue(value?.configuration?.defaultOwner));setArchived(value?.configuration?.archived??false);
    mutation.changed();
  };
  return <section aria-label="Территории продаж">
    <h2>Территории и клиентские портфели</h2>
    <p>Территория объединяет клиентов и имеет ответственного по умолчанию. Владелец клиентского портфеля назначается в карточке клиента. Изменение портфеля не меняет владельцев существующих сделок.</p>
    <QueryNotice query={territories}/>
    {isAdmin&&<Button disabled={pending||ownersUnavailable||territories.busy} onClick={()=>start()}>Создать территорию</Button>}
    {!territories.busy&&!territories.error&&items.length===0&&<p>Территорий пока нет.</p>}
    <div role="region" aria-label="Территории продаж — таблица" tabIndex={0} style={{maxWidth:"100%",overflowX:"auto"}}>
      <table><thead><tr><th scope="col">Территория</th><th scope="col">Родительская территория</th>
        <th scope="col">Ответственный</th><th scope="col">Состояние</th><th scope="col">Управление</th></tr></thead>
        <tbody>{items.map(t=><tr key={t.territoryId}>
          <th scope="row">{t.configuration?.name}</th>
          <td>{territoryLabel(items,t.configuration?.parentId)}</td>
          <td>{ownerLabel(t.configuration?.defaultOwner,choices)}</td><td>{t.configuration?.archived?"Архив":"Активна"}</td>
          <td><Button variant="secondary" onClick={()=>{setSelected(t.territoryId);setCursor("");}}>Клиенты территории</Button>{" "}
            {isAdmin&&<Button variant="secondary" disabled={pending||ownersUnavailable} onClick={()=>start(t)}>Изменить территорию</Button>}</td>
        </tr>)}</tbody></table>
    </div>
    {isAdmin&&(creating||editing)&&<form aria-label="Настройка территории" onSubmit={event=>{
      event.preventDefault();const defaultOwner=ownerWire(owner);if(!defaultOwner)return;
      const configuration={name:name.trim(),...(parent?{parentId:parent}:{}),defaultOwner,archived};
      const done=async()=>{close();await territories.reload();await assignments.reload();};
      if(editing){
        void mutation.runIntent(()=>prepareMutation(api.deals.updateTerritory,{
          territoryId:id,configuration,precondition:{idempotencyKey:mutation.key,expectedVersion:editing.version}
        },id),done);
      }else{
        void mutation.runIntent(()=>prepareMutation(api.deals.createTerritory,{
          territoryId:id,configuration,precondition:{idempotencyKey:mutation.key}
        },id),done);
      }
    }}>
      <h3>{editing?"Редактирование территории":"Новая территория"}</h3>
      <fieldset disabled={pending||archived}>
        <TextField label="Название территории" value={name} required onChange={e=>{setName(e.target.value);mutation.changed();}}/>
        <SelectField label="Родительская территория" value={parent} onChange={e=>{setParent(e.target.value);mutation.changed();}}>
          <TerritoryOptions items={items} current={parent} exclude={id}/>
        </SelectField>
        <SelectField label="Ответственный территории" value={owner} onChange={e=>{setOwner(e.target.value);mutation.changed();}}>
          <OwnerOptions choices={choices} current={owner}/>
        </SelectField>
      </fieldset>
      {editing&&<p><label><input type="checkbox" checked={archived} disabled={pending} onChange={e=>{
        const value=e.target.checked;
        if(value){setName(editing.configuration?.name??"");setParent(editing.configuration?.parentId??"");
          setOwner(ownerValue(editing.configuration?.defaultOwner));}
        setArchived(value);mutation.changed();
      }}/> Архивная территория</label></p>}
      <Button type="submit" disabled={pending||ownersUnavailable||!name.trim()||!owner}>Сохранить территорию</Button>{" "}
      <Button type="button" variant="secondary" disabled={pending} onClick={close}>Отмена</Button>
    </form>}
    <MutationNotice state={mutation}/>
    <h3>Клиентские портфели</h3>
    <SelectField label="Территория портфеля: фильтр" value={selected} onChange={e=>{setSelected(e.target.value);setCursor("");}}>
      <option value="">Все территории</option>
      {items.map(t=><option key={t.territoryId} value={t.territoryId}>{t.configuration?.name}</option>)}
    </SelectField>
    <SelectField label="Владелец портфеля: фильтр" value={ownerFilter} onChange={e=>{setOwnerFilter(e.target.value);setCursor("");}}>
      <option value="">Все владельцы</option>
      {choices.map(c=><option key={c.value} value={c.value}>{c.label}</option>)}
    </SelectField>
    <p><label><input type="checkbox" checked={includeArchived} onChange={e=>{setIncludeArchived(e.target.checked);setCursor("");}}/> Показать архивные назначения</label></p>
    <QueryNotice query={assignments}/>
    {!assignments.busy&&!assignments.error&&assignments.data?.assignments.length===0&&<p>Назначений клиентов нет.</p>}
    <div role="region" aria-label="Клиентские портфели — таблица" tabIndex={0} style={{maxWidth:"100%",overflowX:"auto"}}>
      <table><thead><tr><th scope="col">Клиент</th><th scope="col">Территория</th><th scope="col">Владелец</th>
        <th scope="col">Дополнительные участники</th><th scope="col">Состояние</th></tr></thead>
        <tbody>{assignments.data?.assignments.map(a=><tr key={a.accountId}>
          <th scope="row"><AccountName accountId={a.accountId}/></th>
          <td>{territoryLabel(items,a.configuration?.territoryId)}</td>
          <td>{ownerLabel(a.configuration?.primaryOwner,choices)}</td>
          <td>{a.configuration?.overlays.map(o=>overlayLabel(o.role)+": "+ownerLabel(o.owner,choices)).join("; ")||"—"}</td>
          <td>{a.configuration?.archived?"Архив":"Активен"}</td>
        </tr>)}</tbody></table>
    </div>
    {cursor&&<Button variant="secondary" disabled={assignments.busy} onClick={()=>setCursor("")}>Первая страница портфелей</Button>}
    {assignments.data?.pageInfo?.nextPageToken&&<Button variant="secondary" disabled={assignments.busy}
      onClick={()=>setCursor(assignments.data?.pageInfo?.nextPageToken??"")}>Следующая страница портфелей</Button>}
    <SalesQuotaWorkspace isAdmin={isAdmin} choices={choices} ownersUnavailable={ownersUnavailable||territories.busy||!!territories.error} territories={items} reportId={reportId} scorecard={scorecard}/>
    <SalesRoutingWorkspace isAdmin={isAdmin} choices={choices} ownersUnavailable={ownersUnavailable||territories.busy||!!territories.error} territories={items}/>
  </section>;
}

type OverlayDraft={key:string;role:SalesOverlayRole;owner:string};
function overlayDraft(value:SalesAccountAssignment|undefined):OverlayDraft[]{
  return value?.configuration?.overlays.map(o=>({key:newId(),role:o.role,owner:ownerValue(o.owner)}))??[];
}
export function AccountSalesAssignment({accountId}:{accountId:string}) {
  const query=useQuery(useCallback(async(signal:AbortSignal)=>
    (await api.deals.getSalesAccountAssignment({accountId},{signal})).assignment,[accountId]));
  const territories=useQuery(useCallback(allTerritories,[]));
  const owners=useQuery(useCallback(allOwners,[]));
  const membership=useQuery(useCallback(async(signal:AbortSignal)=>
    (await api.members.getMyMembership({},{signal})).member,[]));
  const isAdmin=membership.data?.role===MemberRole.ADMIN;
  const value=query.data;
  const items=territories.data??[];
  const choices=owners.data?.choices??[];
  const [editing,setEditing]=useState(false);
  const [version,setVersion]=useState<bigint>();
  const [territory,setTerritory]=useState("");
  const [owner,setOwner]=useState("");
  const [overlays,setOverlays]=useState<OverlayDraft[]>([]);
  const [archived,setArchived]=useState(false);
  const mutation=useMutation({
    storageKey:"crm.sales.account-assignment.pending.v1."+accountId,
    lookup:async({resourceId,key},signal)=>(await api.deals.getSalesAccountAssignment({
      accountId:resourceId,receiptIdempotencyKey:key},{signal})).confirmedMutation,
    refresh:async()=>{setEditing(false);await query.reload();}
  });
  const pending=mutation.busy||mutation.pending;
  const reset=()=>{
    setVersion(value?.version);setTerritory(value?.configuration?.territoryId??"");
    setOwner(ownerValue(value?.configuration?.primaryOwner));setOverlays(overlayDraft(value));
    setArchived(value?.configuration?.archived??false);
  };
  return <section aria-label="Коммерческая ответственность клиента">
    <h3>Коммерческая ответственность</h3>
    <QueryNotice query={query}/><QueryNotice query={territories}/><QueryNotice query={owners}/><QueryNotice query={membership}/>
    {!query.busy&&!query.error&&(value?<>
      <p>Территория: {territoryLabel(items,value.configuration?.territoryId)}</p>
      <p>Владелец клиентского портфеля: {ownerLabel(value.configuration?.primaryOwner,choices)}</p>
      <p>Назначение: {value.configuration?.archived?"Архив":"Активно"}</p>
      <ul>{value.configuration?.overlays.map(o=><li key={o.role+":"+ownerValue(o.owner)}>{overlayLabel(o.role)}: {ownerLabel(o.owner,choices)}</li>)}</ul>
    </>:<p>Коммерческая ответственность ещё не назначена.</p>)}
    {isAdmin&&!editing&&!query.busy&&!query.error&&<Button disabled={pending||owners.busy||territories.busy}
      onClick={()=>{reset();setEditing(true);mutation.changed();}}>Настроить ответственность</Button>}
    {isAdmin&&editing&&<form aria-label="Настройка ответственности клиента" onSubmit={event=>{
      event.preventDefault();const primaryOwner=ownerWire(owner);if(!primaryOwner)return;
      const typedOverlays=overlays.flatMap(o=>{
        const selectedOwner=ownerWire(o.owner);return selectedOwner?[{role:o.role,owner:selectedOwner}]:[];
      });
      if(typedOverlays.length!==overlays.length)return;
      const configuration={...(territory?{territoryId:territory}:{}),primaryOwner,overlays:typedOverlays,archived};
      const done=async()=>{setEditing(false);await query.reload();};
      if(version!==undefined){
        void mutation.runIntent(()=>prepareMutation(api.deals.updateSalesAccountAssignment,{
          accountId,configuration,precondition:{idempotencyKey:mutation.key,expectedVersion:version}
        },accountId),done);
      }else{
        void mutation.runIntent(()=>prepareMutation(api.deals.createSalesAccountAssignment,{
          accountId,configuration,precondition:{idempotencyKey:mutation.key}
        },accountId),done);
      }
    }}>
      <fieldset disabled={pending||archived}>
        <SelectField label="Территория клиента" value={territory} onChange={e=>{setTerritory(e.target.value);mutation.changed();}}>
          <TerritoryOptions items={items} current={territory}/>
        </SelectField>
        <SelectField label="Владелец клиентского портфеля" value={owner} onChange={e=>{setOwner(e.target.value);mutation.changed();}}>
          <OwnerOptions choices={choices} current={owner}/>
        </SelectField>
        <fieldset><legend>Дополнительные участники</legend>
          {overlays.map((overlay,index)=><div key={overlay.key}>
            <SelectField label={"Роль участника "+(index+1)} value={overlay.role} onChange={e=>{
              const role=[SalesOverlayRole.SPECIALIST,SalesOverlayRole.ACCOUNT_SUPPORT,SalesOverlayRole.EXECUTIVE_SPONSOR]
                .find(r=>String(r)===e.target.value);
              if(role!==undefined){setOverlays(old=>old.map(o=>o.key===overlay.key?{...o,role}:o));mutation.changed();}
            }}>
              {[SalesOverlayRole.SPECIALIST,SalesOverlayRole.ACCOUNT_SUPPORT,SalesOverlayRole.EXECUTIVE_SPONSOR].map(role=>
                <option key={role} value={role}>{overlayLabel(role)}</option>)}
            </SelectField>
            <SelectField label={"Ответственный участник "+(index+1)} value={overlay.owner} onChange={e=>{
              setOverlays(old=>old.map(o=>o.key===overlay.key?{...o,owner:e.target.value}:o));mutation.changed();
            }}><OwnerOptions choices={choices} current={overlay.owner}/></SelectField>
            <Button type="button" variant="secondary" onClick={()=>{setOverlays(old=>old.filter(o=>o.key!==overlay.key));mutation.changed();}}>Удалить участника</Button>
          </div>)}
          <Button type="button" variant="secondary" disabled={overlays.length>=40} onClick={()=>{
            setOverlays(old=>[...old,{key:newId(),role:SalesOverlayRole.SPECIALIST,owner:""}]);mutation.changed();
          }}>Добавить участника портфеля</Button>
        </fieldset>
      </fieldset>
      {version!==undefined&&<p><label><input type="checkbox" checked={archived} disabled={pending} onChange={e=>{
        if(e.target.checked)reset();
        setArchived(e.target.checked);mutation.changed();
      }}/> Архивное назначение</label></p>}
      <Button type="submit" disabled={pending||!owner||overlays.some(o=>!o.owner)}>Сохранить ответственность</Button>{" "}
      <Button type="button" variant="secondary" disabled={pending} onClick={()=>setEditing(false)}>Отмена</Button>
    </form>}
    <MutationNotice state={mutation}/>
  </section>;
}
