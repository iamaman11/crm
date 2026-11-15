import {useMemo,useSyncExternalStore} from "react";
import {createRoot} from "react-dom/client";
import {AppShell,PageHeader,ThemePicker} from "@ultimate-crm/ui";
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
  const current=route.kind==="accounts"||route.kind==="account"||route.kind==="account-create"?"accounts":route.kind==="section"?route.screen:"";
  const content=route.kind==="accounts"?<AccountScreen registerBlocker={registerNavigationBlocker} filters={{search:route.search,status:route.status,cursor:route.cursor}}/>:
    route.kind==="account"?<AccountPage registerBlocker={registerNavigationBlocker} accountId={route.accountId}/>:
    route.kind==="account-create"?<AccountCreatePage key={route.partyId} registerBlocker={registerNavigationBlocker} partyId={route.partyId}/>:
    route.kind==="not-found"?<><PageHeader title="Страница не найдена"/><a href="#accounts">К списку клиентов</a></>:
    route.screen==="parties"?<PartyScreen key={route.partyId} initialPartyId={route.partyId}/>:
    route.screen==="contacts"?<ContactScreen key={route.partyId} initialPartyId={route.partyId}/>:
    route.screen==="relationships"?<RelationshipScreen key={route.partyId} initialPartyId={route.partyId}/>:
    route.screen==="tasks"?<TaskScreen key={route.accountId} initialAccountId={route.accountId}/>:
    route.screen==="sales"?<SalesScreen key={route.accountId} initialAccountId={route.accountId}/>:
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
const root=document.getElementById("root");
if(!root)throw Error("CRM root missing");
createRoot(root).render(<App/>);
