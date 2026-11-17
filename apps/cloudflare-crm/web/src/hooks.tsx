import {useCallback,useEffect,useRef,useState} from "react";
import {Button,FeedbackPanel} from "@ultimate-crm/ui";
import type {MutationReceipt} from "@ultimate-crm/client/cloudflare";
import {api,newId} from "./api";
import {registerNavigationBlocker} from "./navigation";

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
type MutationWrite=()=>Promise<{receipt?:MutationReceipt|undefined}>;
type MutationIntent={write:MutationWrite;refresh:(()=>Promise<unknown>)|undefined;preserve:boolean};

/** Capture generated request arguments before the first network call. */
export function prepareMutation<Input, Result extends {receipt?:MutationReceipt|undefined}>(
  send:(input:Input)=>Promise<Result>, input:NoInfer<Input>,
):MutationWrite {
  const snapshot=structuredClone(input);
  return ()=>send(snapshot);
}

export function useMutation() {
  const [key,setKey]=useState(newId);
  const inFlight=useRef(false);
  const pendingRef=useRef<MutationIntent|null>(null);
  const [pending,setPending]=useState(false);
  const [busy,setBusy]=useState(false);
  const [notice,setNotice]=useState<{tone:"success"|"warning"|"danger";text:string}>();
  const changed=()=>{if(!pendingRef.current)setKey(newId());};
  const perform=async(intent:MutationIntent)=>{
    if(inFlight.current)return;
    const recovering=pendingRef.current===intent;
    inFlight.current=true;setBusy(true);setNotice(undefined);
    if(intent.preserve){pendingRef.current=intent;setPending(true);}
    const clearPending=()=>{pendingRef.current=null;setPending(false);};
    try {
      const result=await intent.write();
      // A successful HTTP response without commit evidence is not a
      // successful Deal write. Preserve the original typed request.
      if(intent.preserve&&!result.receipt?.requestId)throw Error("Ответ без подтверждения записи.");
      const saved=result.receipt?.replayed?"Сохранено ранее. Повторная запись не выполнялась.":"Сохранено.";
      const correlation=result.receipt?.requestId?" Запрос: "+result.receipt.requestId:"";
      const original=result.receipt?.originalRequestId&&result.receipt.originalRequestId!==result.receipt.requestId?
        " Исходная запись: "+result.receipt.originalRequestId:"";
      clearPending();
      setNotice({tone:"success",text:saved+correlation+original});setKey(newId());
      if(intent.refresh){
        try {await intent.refresh();}
        catch(error){setNotice({tone:"warning",text:saved+correlation+original+" Список устарел: "+api.describeError(error)});}
      }
    } catch(error) {
      const failure=api.decodeError(error);
      // A later rejection cannot establish the outcome of an earlier
      // uncertain attempt. Only the matching receipt can do that here.
      if(intent.preserve&&(recovering||failure.outcome==="unconfirmed")){
        setNotice({tone:"warning",text:"Результат записи не подтверждён. "+failure.text+
          " Повторите именно исходную операцию."});
      }else{
        clearPending();setNotice({tone:"danger",text:failure.text});
      }
    } finally {inFlight.current=false;setBusy(false);}
  };
  const run=async(write:MutationWrite,refresh?:()=>Promise<unknown>)=>{
    if(pendingRef.current)return;
    await perform({write,refresh,preserve:false});
  };
  const runIntent=async(prepare:()=>MutationWrite,refresh?:()=>Promise<unknown>)=>{
    if(inFlight.current||pendingRef.current)return;
    let write:MutationWrite;
    try {write=prepare();}
    catch(error){setNotice({tone:"danger",text:api.describeError(error)});return;}
    await perform({write,refresh,preserve:true});
  };
  const retryPending=async()=>{
    const intent=pendingRef.current;
    if(intent)await perform(intent);
  };
  useEffect(()=>{
    if(!pending)return;
    const guard=(event:BeforeUnloadEvent)=>{event.preventDefault();event.returnValue="";};
    registerNavigationBlocker(()=>false);
    window.addEventListener("beforeunload",guard);
    return ()=>{
      registerNavigationBlocker(null);
      window.removeEventListener("beforeunload",guard);
    };
  },[pending]);
  return {key,busy,notice,pending,changed,run,runIntent,retryPending};
}
export function MutationNotice({state}:{state:ReturnType<typeof useMutation>}) {
  return state.notice?<div className="crm-mutation-notice">
    <FeedbackPanel tone={state.notice.tone} title={state.notice.text}
      action={state.pending?<Button disabled={state.busy} onClick={()=>void state.retryPending()}>Повторить исходную операцию</Button>:undefined}/>
  </div>:null;
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
