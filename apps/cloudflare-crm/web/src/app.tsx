import {useEffect,useState} from "react";
import {createRoot} from "react-dom/client";
import {AppShell,ThemePicker} from "@ultimate-crm/ui";
import {PartyScreen} from "./PartyScreen";
import {AccountScreen} from "./AccountScreen";
import {ContactScreen} from "./ContactScreen";
import {RelationshipScreen} from "./RelationshipScreen";
import {TaskScreen} from "./TaskScreen";
import {SalesScreen} from "./SalesScreen";
import {ConsentScreen} from "./ConsentScreen";
import {ReviewScreen} from "./ReviewScreen";
import {PrivacyScreen} from "./PrivacyScreen";
import {EnrichmentScreen} from "./EnrichmentScreen";
import {DataScreen} from "./DataScreen";

const screens=[
  {id:"accounts",label:"Клиенты",Component:AccountScreen},
  {id:"parties",label:"Участники",Component:PartyScreen},
  {id:"contacts",label:"Контакты",Component:ContactScreen},
  {id:"relationships",label:"Связи",Component:RelationshipScreen},
  {id:"sales",label:"Сделки",Component:SalesScreen},
  {id:"tasks",label:"Задачи",Component:TaskScreen},
  {id:"consents",label:"Коммуникации",Component:ConsentScreen},
  {id:"review",label:"Качество и совпадения",Component:ReviewScreen},
  {id:"privacy",label:"Запросы о данных",Component:PrivacyScreen},
  {id:"enrichment",label:"Обогащение",Component:EnrichmentScreen},
  {id:"data",label:"Импорт и экспорт",Component:DataScreen},
];
function App() {
  const [route,setRoute]=useState(()=>location.hash.slice(1)||"accounts");
  useEffect(()=>{const changed=()=>setRoute(location.hash.slice(1)||"accounts");
    window.addEventListener("hashchange",changed);return()=>window.removeEventListener("hashchange",changed);},[]);
  const [screen,query=""]=route.split("?");
  const params=new URLSearchParams(query);
  const selected=screens.find(s=>s.id===screen);
  const Component=selected?.Component??AccountScreen;
  const content=screen==="contacts"?<ContactScreen initialPartyId={params.get("partyId")??""}/>:
    screen==="relationships"?<RelationshipScreen initialPartyId={params.get("partyId")??""}/>:
    screen==="tasks"?<TaskScreen initialAccountId={params.get("accountId")??""}/>:
    screen==="sales"?<SalesScreen initialAccountId={params.get("accountId")??""}/>:<Component/>;
  return <AppShell productName="CRM" accountSlot={<ThemePicker/>}
    navigation={screens.map(s=>({id:s.id,label:s.label,href:"#"+s.id,current:s.id===(selected?.id??"accounts")}))}>
    <div key={route}>{content}</div>
  </AppShell>;
}
const root=document.getElementById("root");
if(!root)throw Error("CRM root missing");
createRoot(root).render(<App/>);
