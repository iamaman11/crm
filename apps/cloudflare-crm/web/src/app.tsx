import {Component,useMemo,useSyncExternalStore} from "react";
import type {ReactNode} from "react";
import {createRoot} from "react-dom/client";
import {AppShell,Button,FeedbackPanel,PageHeader,ThemePicker} from "@ultimate-crm/ui";
import {PartyScreen} from "./PartyScreen";
import {AccountScreen,AccountPage,AccountCreatePage,accountFilterStatuses} from "./AccountScreen";
import {ContactScreen} from "./ContactScreen";
import {RelationshipScreen} from "./RelationshipScreen";
import {TaskScreen} from "./TaskScreen";
import {MemberScreen} from "./MemberScreen";
import {WorkdayScreen} from "./WorkdayScreen";
import {IntelligenceScreen} from "./IntelligenceScreen";
import {SalesScreen} from "./SalesScreen";
import {ConsentScreen} from "./ConsentScreen";
import {ReviewScreen} from "./ReviewScreen";
import {PrivacyScreen} from "./PrivacyScreen";
import {EnrichmentScreen} from "./EnrichmentScreen";
import {DataScreen} from "./DataScreen";
import {SearchScreen} from "./SearchScreen";
import {parseRoute,sectionLabels} from "./routes";
import {currentRouteHash,subscribeNavigation,registerNavigationBlocker} from "./navigation";

const screens=[
  {id:"workday",label:"Рабочий день"},
  {id:"insights",label:"Аналитика бизнеса"},
  {id:"search",label:"Поиск"},
  {id:"accounts",label:"Клиенты"},
  ...Object.entries(sectionLabels).map(([id,label])=>({id,label})),
] as const;
function unreachable(screen:never):never {throw Error("Unknown CRM screen "+screen);}
function App() {
  const hash=useSyncExternalStore(subscribeNavigation,currentRouteHash);
  const route=useMemo(()=>parseRoute(hash,accountFilterStatuses),[hash]);
  const current=route.kind==="workday"?"workday":route.kind==="insights"?"insights":route.kind==="accounts"||route.kind==="account"||route.kind==="account-create"?"accounts":route.kind==="deal"?"sales":route.kind==="search"?"search":route.kind==="section"?route.screen:"";
  const content=route.kind==="insights"?<IntelligenceScreen
      key={[route.start,route.end,route.pipelineId,route.includeArchived,route.compare].join("/")}
      start={route.start} end={route.end} pipelineId={route.pipelineId}
      includeArchived={route.includeArchived} compare={route.compare}/>:
    route.kind==="workday"?<WorkdayScreen key={route.day+"/"+route.mine+"/"+(route.teamId??"")} day={route.day} mine={route.mine} teamId={route.teamId??""}/>:
    route.kind==="accounts"?<AccountScreen registerBlocker={registerNavigationBlocker} filters={{search:route.search,status:route.status,cursor:route.cursor}}/>:
    route.kind==="account"?<AccountPage registerBlocker={registerNavigationBlocker} accountId={route.accountId}/>:
    route.kind==="account-create"?<AccountCreatePage key={route.partyId} registerBlocker={registerNavigationBlocker} partyId={route.partyId}/>:
    route.kind==="deal"?<SalesScreen key={route.dealId} focusedDealId={route.dealId}/>:
    route.kind==="search"?<SearchScreen query={route.query}/>:
    route.kind==="not-found"?<><PageHeader title="Страница не найдена"/><a href="#accounts">К списку клиентов</a></>:
    route.screen==="parties"?<PartyScreen key={route.partyId} initialPartyId={route.partyId}/>:
    route.screen==="contacts"?<ContactScreen key={route.partyId+"/"+(route.contactPointId??"")}
      initialPartyId={route.partyId} focusedContactPointId={route.contactPointId??""}/>:
    route.screen==="relationships"?<RelationshipScreen key={route.partyId+"/"+(route.relationshipId??"")}
      initialPartyId={route.partyId} focusedRelationshipId={route.relationshipId??""}/>:
    route.screen==="tasks"?<TaskScreen key={route.accountId+"/"+(route.taskId??"")+"/"+(route.dealId??"")+"/"+(route.overdue?"overdue":"all")+"/"+(route.assigneeEmail??"")+"/"+(route.mine?"mine":"team")+"/"+(route.teamId??"")}
      initialAccountId={route.accountId} focusedTaskId={route.taskId??""}
      initialDealId={route.dealId??""} initialSearch={route.search??""}
      initialOverdue={route.overdue??false}
      initialAssigneeEmail={route.assigneeEmail??""} initialMine={route.mine??false} initialTeamId={route.teamId??""}/>:
    route.screen==="team"?<MemberScreen/>:
    route.screen==="sales"?<SalesScreen key={route.accountId}
      initialAccountId={route.accountId} initialSearch={route.search??""}
      initialViewId={route.viewId??""} initialPipelineId={route.pipelineId??""}/>:
    route.screen==="consents"?<ConsentScreen/>:
    route.screen==="review"?<ReviewScreen key={route.partyId} initialPartyId={route.partyId}/>:
    route.screen==="privacy"?<PrivacyScreen/>:
    route.screen==="enrichment"?<EnrichmentScreen/>:route.screen==="data"?<DataScreen/>:unreachable(route.screen);
  return <AppShell productName="CRM" accountSlot={<ThemePicker/>}
    navigation={screens.map(s=>({id:s.id,label:s.label,href:"#"+s.id,current:s.id===current}))}>
    <div data-crm-route-kind={route.kind} data-crm-account-status={route.kind==="accounts"?(route.status??""):undefined}>
      {content}
    </div>
  </AppShell>;
}
// A render exception is not evidence of a rollback. Keep the pending
// marker and let the original recovery path verify it after reload.
class AppErrorBoundary extends Component<{children:ReactNode},{failed:boolean}> {
  override state={failed:false};
  static getDerivedStateFromError(){return {failed:true};}
  override render(){
    return this.state.failed?<FeedbackPanel tone="danger" title="Не удалось отобразить CRM."
      action={<Button onClick={()=>window.location.reload()}>Перезагрузить приложение</Button>}>
      Результат незавершённой записи необходимо проверить после перезагрузки.
    </FeedbackPanel>:this.props.children;
  }
}
const root=document.getElementById("root");
if(!root)throw Error("CRM root missing");
createRoot(root).render(<AppErrorBoundary><App/></AppErrorBoundary>);
