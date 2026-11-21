import {Component,useMemo,useSyncExternalStore} from "react";
import type {ReactNode} from "react";
import {createRoot} from "react-dom/client";
import {AppShell,Button,FeedbackPanel,PageHeader,ThemePicker} from "@ultimate-crm/ui";
import {PartyScreen} from "./PartyScreen";
import {AccountScreen,AccountPage,AccountCreatePage,accountFilterStatuses} from "./AccountScreen";
import {ContactScreen} from "./ContactScreen";
import {RelationshipScreen} from "./RelationshipScreen";
import {TaskScreen} from "./TaskScreen";
import {SalesScreen} from "./SalesScreen";
import {ConsentScreen} from "./ConsentScreen";
import {ReviewScreen} from "./ReviewScreen";
import {PrivacyScreen} from "./PrivacyScreen";
import {EnrichmentScreen} from "./EnrichmentScreen";
import {DataScreen} from "./DataScreen";
import {parseRoute} from "./routes";
import {currentRouteHash,subscribeNavigation,registerNavigationBlocker} from "./navigation";

const screens=[
  {id:"accounts",label:"Клиенты"},
  {id:"parties",label:"Участники"},
  {id:"contacts",label:"Контакты"},
  {id:"relationships",label:"Связи"},
  {id:"sales",label:"Сделки"},
  {id:"tasks",label:"Задачи"},
  {id:"consents",label:"Коммуникации"},
  {id:"review",label:"Качество и совпадения"},
  {id:"privacy",label:"Запросы о данных"},
  {id:"enrichment",label:"Обогащение"},
  {id:"data",label:"Импорт и экспорт"},
] as const;
function App() {
  const hash=useSyncExternalStore(subscribeNavigation,currentRouteHash);
  const route=useMemo(()=>parseRoute(hash,accountFilterStatuses),[hash]);
  const current=route.kind==="accounts"||route.kind==="account"||route.kind==="account-create"?"accounts":route.kind==="deal"?"sales":route.kind==="section"?route.screen:"";
  const content=route.kind==="accounts"?<AccountScreen registerBlocker={registerNavigationBlocker} filters={{search:route.search,status:route.status,cursor:route.cursor}}/>:
    route.kind==="account"?<AccountPage registerBlocker={registerNavigationBlocker} accountId={route.accountId}/>:
    route.kind==="account-create"?<AccountCreatePage key={route.partyId} registerBlocker={registerNavigationBlocker} partyId={route.partyId}/>:
    route.kind==="deal"?<SalesScreen key={route.dealId} focusedDealId={route.dealId}/>:
    route.kind==="not-found"?<><PageHeader title="Страница не найдена"/><a href="#accounts">К списку клиентов</a></>:
    route.screen==="parties"?<PartyScreen key={route.partyId} initialPartyId={route.partyId}/>:
    route.screen==="contacts"?<ContactScreen key={route.partyId+"/"+(route.contactPointId??"")}
      initialPartyId={route.partyId} focusedContactPointId={route.contactPointId??""}/>:
    route.screen==="relationships"?<RelationshipScreen key={route.partyId+"/"+(route.relationshipId??"")}
      initialPartyId={route.partyId} focusedRelationshipId={route.relationshipId??""}/>:
    route.screen==="tasks"?<TaskScreen key={route.accountId+"/"+(route.taskId??"")+"/"+(route.dealId??"")}
      initialAccountId={route.accountId} focusedTaskId={route.taskId??""}
      initialDealId={route.dealId??""}/>:
    route.screen==="sales"?<SalesScreen key={route.accountId}
      initialAccountId={route.accountId} initialSearch={route.search??""}
      initialViewId={route.viewId??""}/>:
    route.screen==="consents"?<ConsentScreen/>:
    route.screen==="review"?<ReviewScreen/>:
    route.screen==="privacy"?<PrivacyScreen/>:
    route.screen==="enrichment"?<EnrichmentScreen/>:<DataScreen/>;
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
  state={failed:false};
  static getDerivedStateFromError(){return {failed:true};}
  render(){
    return this.state.failed?<FeedbackPanel tone="danger" title="Не удалось отобразить CRM."
      action={<Button onClick={()=>window.location.reload()}>Перезагрузить приложение</Button>}>
      Результат незавершённой записи необходимо проверить после перезагрузки.
    </FeedbackPanel>:this.props.children;
  }
}
const root=document.getElementById("root");
if(!root)throw Error("CRM root missing");
createRoot(root).render(<AppErrorBoundary><App/></AppErrorBoundary>);
