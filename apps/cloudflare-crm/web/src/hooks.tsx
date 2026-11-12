import {useCallback,useEffect,useRef,useState} from "react";
import {Button,FeedbackPanel} from "@ultimate-crm/ui";
import type {MutationReceipt} from "@ultimate-crm/client/cloudflare";
import {api,newId} from "./api";

export function useQuery<T>(load:()=>Promise<T>) {
  const [data,setData]=useState<T>();
  const [error,setError]=useState("");
  const [busy,setBusy]=useState(false);
  const sequence=useRef(0);
  const reload=useCallback(async()=>{
    const current=++sequence.current;
    setBusy(true);setError("");
    try {
      const result=await load();
      if(current===sequence.current)setData(result);
      return result;
    } catch(error) {
      if(current===sequence.current)setError(api.describeError(error));
      throw error;
    } finally {if(current===sequence.current)setBusy(false);}
  },[load]);
  useEffect(()=>{void reload().catch(()=>{});return()=>{sequence.current++;};},[reload]);
  return {data,error,busy,reload};
}
export function useMutation() {
  const [key,setKey]=useState(newId);
  const inFlight=useRef(false);
  const [busy,setBusy]=useState(false);
  const [notice,setNotice]=useState<{tone:"success"|"warning"|"danger";text:string}>();
  const changed=()=>{setKey(newId());};
  const run=async(write:()=>Promise<{receipt?:MutationReceipt|undefined}>,refresh?:()=>Promise<unknown>)=>{
    if(inFlight.current)return;
    inFlight.current=true;setBusy(true);setNotice(undefined);
    try {
      const result=await write();
      const saved=result.receipt?.replayed?"Сохранено ранее. Повторная запись не выполнялась.":"Сохранено.";
      const correlation=result.receipt?.requestId?" Запрос: "+result.receipt.requestId:"";
      setNotice({tone:"success",text:saved+correlation});setKey(newId());
      if(refresh){
        try {await refresh();}
        catch(error){setNotice({tone:"warning",text:saved+correlation+" Список устарел: "+api.describeError(error)});}
      }
    } catch(error) {setNotice({tone:"danger",text:api.describeError(error)});}
    finally {inFlight.current=false;setBusy(false);}
  };
  return {key,busy,notice,changed,run};
}
export function MutationNotice({state}:{state:ReturnType<typeof useMutation>}) {
  return state.notice?<FeedbackPanel tone={state.notice.tone} title={state.notice.text}/>:null;
}
export function QueryNotice({query}:{query:{busy:boolean;error:string;reload:()=>Promise<unknown>}}) {
  return query.error?<FeedbackPanel tone="danger" title={query.error}
    action={<Button onClick={()=>void query.reload().catch(()=>{})}>Повторить чтение</Button>}/>:
    query.busy?<p role="status">Загрузка…</p>:null;
}
export function Paging({next,busy,onNext,onFirst}:{next:string;busy:boolean;onNext:()=>void;onFirst:()=>void}) {
  return <p><Button variant="secondary" disabled={busy} onClick={onFirst}>В начало</Button>{" "}
    <Button variant="secondary" disabled={busy||!next} onClick={onNext}>Следующая страница</Button></p>;
}

export function useReadAction<T>() {
  const [data,setData]=useState<T>();
  const [error,setError]=useState("");
  const [busy,setBusy]=useState(false);
  const run=async(read:()=>Promise<T>)=>{
    setBusy(true);setError("");setData(undefined);
    try {setData(await read());}catch(error){setError(api.describeError(error));}
    finally{setBusy(false);}
  };
  return {data,error,busy,run};
}
