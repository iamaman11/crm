import {useCallback} from "react";
import {PageHeader} from "@ultimate-crm/ui";
import {api} from "./api";
import {useQuery,QueryNotice} from "./hooks";
import {SearchField} from "./SearchField";
import {navigateTo} from "./navigation";
import {accountHref,dealHref,partyHref,searchHref} from "./routes";

/** Bounded cross-domain discovery, not a second query engine or index.
 * Each original owner authorizes its own signed tenant and viewer reads. */
export function SearchScreen({query}:{query:string}) {
  const term=query.trim();
  const found=useQuery(useCallback(async(signal:AbortSignal)=>{
    if(!term)return undefined;
    const [parties,accounts,deals]=await Promise.all([
      api.partyRecords.listParties({page:{pageSize:15},search:term},{signal}),
      api.accounts.listAccounts({page:{pageSize:15},search:term},{signal}),
      api.deals.listDeals({page:{pageSize:15},search:term},{signal})
    ]);
    return {parties,accounts,deals};
  },[term]));
  return <>
    <PageHeader title="Поиск по CRM"
      description="Найдите человека, организацию, клиента или сделку по части названия либо ID."/>
    <SearchField label="Поиск по CRM" submitted={query}
      onSearch={value=>navigateTo(searchHref(value))}/>
    <p><small>Результаты читаются напрямую из трёх разделов CRM с текущими правами доступа.
      Поиск учитывает регистр букв; показана только первая страница каждого раздела.</small></p>
    {!term?<p>Введите название, имя или ID для поиска.</p>:<>
      <QueryNotice query={found}/>
      {!found.busy&&!found.error&&found.data&&<>
        <section aria-label="Найденные участники">
          <h2>Люди и организации</h2>
          {found.data.parties.parties.length===0?<p>Участники не найдены.</p>:
            <ul>{found.data.parties.parties.map(party=>
              <li key={party.partyId}><a href={partyHref(party.partyId)}>{party.displayName}</a></li>
            )}</ul>}
          {found.data.parties.pageInfo?.nextPageToken&&<p>Есть другие результаты. <a href="#parties">Открыть раздел участников</a>.</p>}
        </section>
        <section aria-label="Найденные клиенты">
          <h2>Клиенты</h2>
          {found.data.accounts.accounts.length===0?<p>Клиенты не найдены.</p>:
            <ul>{found.data.accounts.accounts.map(account=>
              <li key={account.accountId}><a href={accountHref(account.accountId)}>{account.name}</a></li>
            )}</ul>}
          {found.data.accounts.pageInfo?.nextPageToken&&<p>Есть другие результаты. <a href="#accounts">Открыть раздел клиентов</a>.</p>}
        </section>
        <section aria-label="Найденные сделки">
          <h2>Сделки</h2>
          {found.data.deals.deals.length===0?<p>Сделки не найдены.</p>:
            <ul>{found.data.deals.deals.map(deal=>
              <li key={deal.dealId}><a href={dealHref(deal.dealId)}>{deal.name}</a></li>
            )}</ul>}
          {found.data.deals.pageInfo?.nextPageToken&&<p>Есть другие результаты. <a href="#sales">Открыть раздел сделок</a>.</p>}
        </section>
      </>}
    </>}
  </>;
}
