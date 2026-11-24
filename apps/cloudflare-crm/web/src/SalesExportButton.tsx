import {Button,FeedbackPanel} from "@ultimate-crm/ui";
import type {ExportSalesAnalyticsResponse,GetSalesScorecardResponse} from "@ultimate-crm/client/cloudflare";
import {api} from "./api";
import {useReadAction} from "./hooks";
import {downloadCsv} from "./downloadCsv";

type Request=Extract<Parameters<typeof api.deals.exportSalesAnalytics>[0],{$typeName?:never}>;
export function SalesExportButton({request,label,onScorecard,disabled=false}:{request:Request;label:string;onScorecard?:(scorecard:GetSalesScorecardResponse)=>void;disabled?:boolean}){
  const exported=useReadAction<ExportSalesAnalyticsResponse>();
  return <div>
    <Button variant="secondary" disabled={disabled||exported.busy} onClick={()=>void exported.run(async signal=>{
      const result=await api.deals.exportSalesAnalytics(request,{signal});
      if(onScorecard){
        if(!result.scorecard)throw Error("Сервер не вернул срез выгрузки");
        onScorecard(result.scorecard);
      }
      downloadCsv(result.csv,result.filename,result.mediaType);
      return result;
    })}>{label}</Button>
    {exported.error&&<FeedbackPanel tone="danger" title={exported.error}/>}
    {exported.data&&<p>Для точного импорта в электронную таблицу задайте столбцу value текстовый формат. Денежные значения записаны в минимальных единицах валюты.</p>}
    {exported.data&&<p role="status">CSV: {exported.data.rowCount} строк · {exported.data.csv.length} байт · SHA-256 <code style={{overflowWrap:"anywhere"}}>{exported.data.csvSha256Hex}</code></p>}
  </div>;
}
