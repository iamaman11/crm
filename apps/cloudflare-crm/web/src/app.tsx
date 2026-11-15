import {useCallback,useEffect,useRef,useState} from "react";
import {createRoot} from "react-dom/client";
import {AppShell,PageHeader,ThemePicker} from "@ultimate-crm/ui";
import {PartyScreen} from "./PartyScreen";
import {AccountScreen,AccountPage} from "./AccountScreen";
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
  const [route,setRoute]=useState(()=>parseRoute(window.location.hash));
  const lastAcceptedHash=useRef(window.location.hash);
  const lastAcceptedState=useRef<unknown>(window.history.state);
  const restoringBack=useRef(false);
  const routeBlocker=useRef<(()=>boolean)|null>(null);
  const registerBlocker=useCallback((blocker:(()=>boolean)|null)=>{routeBlocker.current=blocker;},[]);
  useEffect(()=>{
    const changed=(event:Event)=>{
      const target=window.location.hash;
      if(restoringBack.current){
        // Back was refused: do not display the transient previous route.
        if(target===lastAcceptedHash.current)restoringBack.current=false;
        return;
      }
      if(target!==lastAcceptedHash.current&&routeBlocker.current&&!routeBlocker.current()){
        const accepted=lastAcceptedState.current;
        if(event.type==="popstate"&&accepted!==null&&typeof accepted==="object"&&
          "crmReturnTo" in accepted&&accepted.crmReturnTo===target){
          // Undo Back without replacing the previous history entry or its filters.
          restoringBack.current=true;
          window.history.forward();
        }else{
          window.history.replaceState(accepted,"",
            window.location.pathname+window.location.search+lastAcceptedHash.current);
        }
        return;
      }
      lastAcceptedHash.current=target;
      lastAcceptedState.current=window.history.state;
      setRoute(parseRoute(target));
    };
    window.addEventListener("hashchange",changed);
    window.addEventListener("popstate",changed);
    return ()=>{window.removeEventListener("hashchange",changed);window.removeEventListener("popstate",changed);};
  },[]);
  const current=route.kind==="accounts"||route.kind==="account"?"accounts":route.kind==="section"?route.screen:"";
  const content=route.kind==="accounts"?<AccountScreen registerBlocker={registerBlocker} filters={{search:route.search,status:route.status,cursor:route.cursor}}/>:
    route.kind==="account"?<AccountPage registerBlocker={registerBlocker} accountId={route.accountId}/>:
    route.kind==="not-found"?<><PageHeader title="Страница не найдена"/><a href="#accounts">К списку клиентов</a></>:
    route.screen==="parties"?<PartyScreen/>:
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
    {content}
  </AppShell>;
}
const root=document.getElementById("root");
if(!root)throw Error("CRM root missing");
createRoot(root).render(<App/>);
