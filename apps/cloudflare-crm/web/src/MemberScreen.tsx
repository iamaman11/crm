import {useCallback,useState} from "react";
import type {ReactNode} from "react";
import {Button,PageHeader,SelectField,TextField} from "@ultimate-crm/ui";
import type {Member} from "@ultimate-crm/client/cloudflare";
import {MemberRole,TaskStatus} from "@ultimate-crm/client/cloudflare";
import {api,dateText} from "./api";
import {useQuery,useMutation,prepareMutation,MutationNotice,QueryNotice} from "./hooks";
import {taskHref,taskAssigneeHref,taskTeamHref,workdayHref,salesPipelineHref,intelligenceHref} from "./routes";
import {allMembers,humanMember} from "./memberCatalog";
import {TeamWorkspace} from "./TeamWorkspace";
import {allTeams} from "./teamCatalog";
import {ownerValue} from "./ownerCatalog";

const roleName=(role:MemberRole)=>role===MemberRole.ADMIN?"Администратор":
  role===MemberRole.VIEWER?"Чтение всех полей":"Неизвестная роль";
function ScrollTable({label,children}:{label:string;children:ReactNode}) {
  return <div role="region" aria-label={label} tabIndex={0}
    style={{maxWidth:"100%",width:"100%",overflowX:"auto"}}>{children}</div>;
}

/** The original Access enrollment is the single source for a human identity. */
export function MemberScreen(){
  const [memberEmail,setMemberEmail]=useState("");
  const [memberRole,setMemberRole]=useState(MemberRole.VIEWER);
  const [editing,setEditing]=useState<Member>();
  const [editRole,setEditRole]=useState(MemberRole.VIEWER);
  const [editActive,setEditActive]=useState(true);
  const [myCursor,setMyCursor]=useState("");
  const roster=useQuery(useCallback(allMembers,[]));
  const teams=useQuery(useCallback(allTeams,[]));
  const me=useQuery(useCallback(async(signal:AbortSignal)=>
    (await api.members.getMyMembership({}, {signal})).member,[]));
  const workload=useQuery(useCallback((signal:AbortSignal)=>
    api.tasks.listTaskWorkload({}, {signal}),[]));
  const mine=useQuery(useCallback((signal:AbortSignal)=>
    api.tasks.listTasks({assignedToMe:true,status:TaskStatus.OPEN,
      page:{pageSize:25,pageToken:myCursor}},{signal}),[myCursor]));
  const mutation=useMutation({
    storageKey:"crm.member.pending.v1",
    lookup:async({resourceId,key},signal)=>
      (await api.members.getMember({email:resourceId,receiptIdempotencyKey:key},
        {signal})).confirmedMutation,
    refresh:async()=>{await Promise.all([roster.reload(),workload.reload(),me.reload()]);}
  });
  const isAdmin=me.data?.role===MemberRole.ADMIN;
  const eligible=roster.data?.filter(m=>humanMember(m.email))??[];
  const members=roster.data??[];
  const overview=workload.data?.buckets??[];
  const actor=me.data?.email??"";
  const staffCount=eligible.filter(m=>m.active).length;
  const pending=mutation.pending||mutation.busy;
  return <>
    <PageHeader title="Команда и распределение работы"
      description="Действительные участники tenant, подтверждённые роли, назначение задач и общая нагрузка. Источник доступа — подписанный Cloudflare Access и одна D1."/>
    <p><a href={workdayHref()}>Общая очередь</a>{" · "}
      <a href="#tasks">Все задачи</a>{" · "}
      <a href="#insights">Выручка и портфель продаж</a></p>
    <section aria-label="Моя проверенная сессия">
      <h2>Моя проверенная сессия</h2>
      <QueryNotice query={me}/>
      {me.data&&<p>{humanMember(me.data.email)?
        "Человеческая учётная запись: ":"Служебный агент (не сотрудник): "}
        <strong>{me.data.email}</strong>
        {" · "}{roleName(me.data.role)}
        {" · "}{me.data.active?"активна":"неактивна"}</p>}
      <p>Идентификатор сотрудника здесь приходит только из подписанной сессии.
        Другой участник получает доступ, лишь войдя самостоятельно через Cloudflare Access
        с выданным ему адресом; добавление в список не подменяет вход.</p>
    </section>
    <section aria-label="Сотрудники и права">
      <h2>Сотрудники и роли</h2>
      <QueryNotice query={roster}/>
      <p>Активных сотрудников (без технических аккаунтов): {staffCount}.
        Неактивный участник не проходит повторную проверку доступа;
        его старые задачи не исчезают и доступны для переназначения администратором.</p>
      {!roster.busy&&members.length===0&&<p>Участников пока нет.</p>}
      <ScrollTable label="Участники tenant — таблица">
        <table><thead><tr><th scope="col">Участник</th>
          <th scope="col">Роль</th><th scope="col">Доступ</th>
          <th scope="col">Работа</th><th scope="col">Управление</th></tr></thead>
          <tbody>{members.map(m=><tr key={m.email}>
            <th scope="row">{m.email}{!humanMember(m.email)?" · технический агент":""}</th>
            <td>{roleName(m.role)}</td><td>{m.active?"Активен":"Деактивирован"}</td>
            <td>{humanMember(m.email)?<a href={taskAssigneeHref(m.email)}>
              Задачи участника</a>:"Системные задания"}{humanMember(m.email)&&<><span>{" · "}</span>
              <a href={salesPipelineHref("","","","actor:"+m.email)}>Сделки участника</a>{" · "}
              <a href={intelligenceHref({owner:"actor:"+m.email})}>Портфель участника</a></>}</td>
            <td>{isAdmin&&humanMember(m.email)&&m.email!==actor?
              <Button variant="secondary" disabled={pending} onClick={()=>{
                setEditing(m);setEditRole(m.role);setEditActive(m.active);mutation.changed();
              }}>Изменить доступ</Button>:"—"}</td>
          </tr>)}</tbody></table>
      </ScrollTable>
      {isAdmin&&<form aria-label="Пригласить сотрудника" onSubmit={event=>{
        event.preventDefault();
        const email=memberEmail.trim().toLowerCase();
        void mutation.runIntent(()=>prepareMutation(api.members.provisionMember,{
          email,role:memberRole,precondition:{idempotencyKey:mutation.key}
        },email),async()=>{setMemberEmail("");await roster.reload();await workload.reload();});
      }}>
        <h3>Приглашение в существующий tenant</h3>
        <p>Cloudflare Access должен разрешать этому адресу вход. Мы лишь записываем
          права в CRM; не создаём сессию и не отправляем вымышленное письмо.</p>
        <TextField label="Email нового сотрудника" type="email" value={memberEmail}
          onChange={e=>{setMemberEmail(e.target.value);mutation.changed();}} required/>
        <SelectField label="Роль нового сотрудника" value={memberRole}
          onChange={e=>{setMemberRole(Number(e.target.value) as MemberRole);mutation.changed();}}>
          <option value={MemberRole.VIEWER}>Чтение всех полей</option>
          <option value={MemberRole.ADMIN}>Администратор</option>
        </SelectField>
        <Button type="submit" disabled={pending||!memberEmail.includes("@")}>
          Добавить участника</Button>
      </form>}
      {editing&&isAdmin&&<form aria-label="Изменить доступ сотрудника" onSubmit={e=>{
        e.preventDefault();
        const target=editing;
        void mutation.runIntent(()=>prepareMutation(api.members.updateMember,{
          email:target.email,
          ...(editRole!==target.role?{role:editRole}:{}),
          ...(editActive!==target.active?{active:editActive}:{}),
          precondition:{idempotencyKey:mutation.key,expectedVersion:target.version}
        },target.email),async()=>{setEditing(undefined);await roster.reload();await workload.reload();});
      }}>
        <h3>Доступ: {editing.email}</h3>
        <SelectField label="Роль сотрудника" value={editRole}
          onChange={e=>{setEditRole(Number(e.target.value) as MemberRole);mutation.changed();}}>
          <option value={MemberRole.VIEWER}>Чтение всех полей</option>
          <option value={MemberRole.ADMIN}>Администратор</option>
        </SelectField>
        <p><label><input type="checkbox" checked={editActive} onChange={e=>{
          setEditActive(e.target.checked);mutation.changed();
        }}/> Активный доступ</label></p>
        <Button type="submit" disabled={pending||
          (editRole===editing.role&&editActive===editing.active)}>Сохранить доступ</Button>{" "}
        <Button type="button" variant="secondary" disabled={pending}
          onClick={()=>setEditing(undefined)}>Отмена</Button>
      </form>}
      <MutationNotice state={mutation}/>
    </section>
    <TeamWorkspace query={teams} members={members} isAdmin={isAdmin} actor={actor} onChanged={workload.reload}/>
    <section aria-label="Реальная нагрузка команды">
      <h2>Распределение и нагрузка</h2>
      <p>Авторитетные числа по всем задачам до LIMIT, включая завершённые.
        Срок считается по серверному времени; нельзя вывести нагрузку команды
        суммой 25 карточек. Нет «псевдоперсональной» метрики.</p>
      <QueryNotice query={workload}/>
      {workload.data&&<p>Срез UTC: {new Date(Number(workload.data.asOfUnixSeconds)*1000)
        .toISOString().slice(0,19)}.</p>}
      <ScrollTable label="Нагрузка по владельцам — таблица">
        <table><thead><tr><th scope="col">Владелец</th>
          <th scope="col">Открыто</th><th scope="col">Завершено</th>
          <th scope="col">Просрочено</th><th scope="col">До 7 дней</th>
          <th scope="col">Действие</th></tr></thead><tbody>
          {overview.map(bucket=>{
            const email=bucket.owner?.owner.case==="assigneeIdentity"?bucket.owner.owner.value:"";
            const teamId=bucket.owner?.owner.case==="teamId"?bucket.owner.owner.value:"";
            const team=teams.data?.find(t=>t.teamId===teamId);
            const member=members.find(m=>m.email===email);
            const key=ownerValue(bucket.owner);
            return <tr key={key}>
              <th scope="row">{email||"Команда: "+(team?.name??teamId)}{team&&!team.active?" · архив":""}
                {email&&!humanMember(email)?" · агент":""}
                {member&&!member.active?" · доступ отключён":""}
                {email&&humanMember(email)&&!member?" · нет активного членства":""}</th>
              <td>{bucket.openCount.toString()}</td>
              <td>{bucket.completedCount.toString()}</td>
              <td>{bucket.overdueOpenCount.toString()}</td>
              <td>{bucket.dueNext7DaysCount.toString()}</td>
              <td>{email&&humanMember(email)?
                <a href={taskAssigneeHref(email)}>Открыть задачи</a>:teamId?
                <a href={taskTeamHref(teamId)}>Открыть задачи команды</a>:"—"}</td>
            </tr>;
          })}
        </tbody></table>
      </ScrollTable>
      {!workload.busy&&!workload.error&&overview.length===0&&<p>В tenant нет задач.</p>}
    </section>
    <section aria-label="Моя очередь по подписанной сессии">
      <h2>{humanMember(actor)?"Мои открытые задачи":"Открытые задачи служебного агента"}</h2>
      <p>Фильтр выполняется исходным Task owner по подписанному актору
        до курсора и ограничения страницы, а не по адресу из браузера.</p>
      <QueryNotice query={mine}/>
      <ul>{mine.data?.tasks.map(t=><li key={t.taskId}>
        <a href={taskHref(t.taskId,t.accountId)}>{t.subject}</a>
        {" · "}{dateText(t.dueAt)}
      </li>)}</ul>
      {!mine.busy&&!mine.error&&mine.data?.tasks.length===0&&<p>Открытых задач нет.</p>}
      {mine.data?.pageInfo?.nextPageToken&&<Button variant="secondary" disabled={mine.busy}
        onClick={()=>setMyCursor(mine.data?.pageInfo?.nextPageToken??"")}>
        Следующие мои задачи</Button>}
      {myCursor&&<Button variant="secondary" onClick={()=>setMyCursor("")}>К началу</Button>}
    </section>
  </>;
}
