import {useCallback,useEffect,useLayoutEffect,useRef,useState} from "react";
import {Button,FeedbackPanel} from "@ultimate-crm/ui";
import type {MutationReceipt, CrmFailure} from "@ultimate-crm/client/cloudflare";
import {api,newId} from "./api";
import {registerNavigationBlocker} from "./navigation";

const READ_TIMEOUT_MS=15_000;
type ReadLoad<T>=(signal:AbortSignal)=>Promise<T>;
type ReadState<T>={load:ReadLoad<T>;data:T|undefined;busy:boolean;error:string};

// One state owner for automatic queries and explicit read actions.
function useRead<T>() {
  const [state,setState]=useState<ReadState<T>>();
  const active=useRef<AbortController|null>(null);
  const cancel=useCallback(()=>{
    const previous=active.current;
    active.current=null;
    previous?.abort();
  },[]);
  const execute=useCallback(async(load:ReadLoad<T>)=>{
    cancel();
    const controller=new AbortController();
    active.current=controller;
    const signal=AbortSignal.any([controller.signal,AbortSignal.timeout(READ_TIMEOUT_MS)]);
    setState(previous=>({load,data:previous&&previous.load===load?previous.data:undefined,busy:true,error:""}));
    try {
      const data=await load(signal);
      signal.throwIfAborted();
      setState({load,data,busy:false,error:""});
      return data;
    }catch(error){
      if(active.current===controller)
        setState(previous=>({load,data:previous&&previous.load===load?previous.data:undefined,busy:false,
          error:signal.aborted&&!controller.signal.aborted?"Время ожидания чтения истекло.":api.describeError(error)}));
      throw error;
    }finally{
      if(active.current===controller)active.current=null;
    }
  },[cancel]);
  useLayoutEffect(()=>cancel,[cancel]);
  return {state,execute,cancel};
}

export function useQuery<T>(load:ReadLoad<T>) {
  const {state,execute,cancel}=useRead<T>();
  const latest=useRef(load);
  useLayoutEffect(()=>{latest.current=load;return cancel;},[load,cancel]);
  const reload=useCallback(()=>execute(latest.current),[execute]);
  useEffect(()=>{void reload().catch(()=>{});},[load,reload]);
  // Never present an old filter/record/page result as the new key's data.
  const current=state?.load===load?state:undefined;
  return {data:current?.data,error:current?.error??"",busy:current?.busy??true,
    stale:current?.data!==undefined&&(current.busy||!!current.error),reload};
}

type ReceiptProof=Pick<MutationReceipt,"requestId"|"originalRequestId"|"resourceVersion">;
type MutationWrite=()=>Promise<{receipt?:MutationReceipt|undefined}>;
type ReceiptMarker={dealId:string;key:string;expectedVersion:string};
type MutationIntent={write:MutationWrite;refresh:(()=>Promise<unknown>)|undefined}&
  ({preserve:false}|{preserve:true;marker:ReceiptMarker});
type PreparedMutation={write:MutationWrite;marker:ReceiptMarker};
type ReceiptInput={
  dealId?:string|undefined;
  precondition?:{idempotencyKey?:string|undefined;expectedVersion?:bigint|undefined}|undefined;
};
type Pending={kind:"live";intent:Extract<MutationIntent,{preserve:true}>}|
  {kind:"stored";marker:ReceiptMarker}|{kind:"unreadable"};
type RecoveryOptions={
  storageKey:string;
  lookup:(marker:ReceiptMarker,signal:AbortSignal)=>Promise<ReceiptProof|undefined>;
  refresh:()=>Promise<unknown>;
};
type MutationFeedback={tone:"success"|"warning"|"danger";text:string;fields?:CrmFailure["fieldViolations"]};

// Only opaque identifiers and an integer version survive a reload of this
// tab. No business payload, access token, form values or client-side write log.
function restoredPending(storageKey:string|undefined):Pending|null {
  if(!storageKey)return null;
  try {
    const raw=sessionStorage.getItem(storageKey);
    if(raw===null)return null;
    const value:unknown=JSON.parse(raw);
    if(value!==null&&typeof value==="object"&&
      "dealId" in value&&typeof value.dealId==="string"&&value.dealId.length>0&&
      "key" in value&&typeof value.key==="string"&&/^[a-f0-9-]{36}$/i.test(value.key)&&
      "expectedVersion" in value&&typeof value.expectedVersion==="string"&&
      /^(0|[1-9][0-9]{0,18})$/.test(value.expectedVersion))
      return {kind:"stored",marker:{dealId:value.dealId,key:value.key,expectedVersion:value.expectedVersion}};
  } catch {/* Unreadable evidence is not proof that no command is pending. */}
  return {kind:"unreadable"};
}

function matchesReceipt(marker:ReceiptMarker,receipt:ReceiptProof|undefined):
  receipt is ReceiptProof&{originalRequestId:string} {
  return !!receipt?.requestId&&!!receipt.originalRequestId&&
    receipt.resourceVersion===BigInt(marker.expectedVersion)+1n;
}

export function prepareMutation<Input extends ReceiptInput, Result extends {receipt?:MutationReceipt|undefined}>(
  send:(input:Input)=>Promise<Result>, input:NoInfer<Input>,
):PreparedMutation {
  const snapshot=structuredClone(input);
  const {dealId,precondition}=snapshot;
  if(!dealId||!precondition?.idempotencyKey)throw Error("Отсутствует идентификатор исходной операции.");
  return {write:()=>send(snapshot),marker:{dealId,key:precondition.idempotencyKey,
    expectedVersion:String(precondition.expectedVersion??0n)}};
}

export function useMutation(recovery?:RecoveryOptions) {
  const [key,setKey]=useState(newId);
  const inFlight=useRef(false);
  const [pendingState,setPendingState]=useState<Pending|null>(()=>restoredPending(recovery?.storageKey));
  const pendingRef=useRef(pendingState);
  const pending=pendingState!==null;
  const recoveryOnly=pending&&pendingState.kind!=="live";
  const [busy,setBusy]=useState(false);
  const [notice,setNotice]=useState<MutationFeedback|undefined>(()=>pendingState?
    {tone:"warning",text:"Результат записи не подтверждён. После перезагрузки доступна проверка подтверждения, а не повтор записи."}:undefined);
  const changePending=(value:Pending|null)=>{pendingRef.current=value;setPendingState(value);};
  const clearPending=()=>{
    const current=pendingRef.current;
    if(recovery&&current){
      const marker=current.kind==="live"?current.intent.marker:current.kind==="stored"?current.marker:undefined;
      const stored=restoredPending(recovery.storageKey);
      // A read may finish after navigation. Never remove another intent.
      if(stored?.kind==="stored"&&(!marker||stored.marker.key!==marker.key||
        stored.marker.dealId!==marker.dealId||stored.marker.expectedVersion!==marker.expectedVersion)){
        changePending(stored);return false;
      }
      try {
        if(stored?.kind==="unreadable"&&marker){
          changePending(stored);return false;
        }
        sessionStorage.removeItem(recovery.storageKey);
      } catch {
        // A proven result must never offer another write just because
        // browser storage cleanup failed.
        if(marker)changePending({kind:"stored",marker});
        return false;
      }
    }
    changePending(null);return true;
  };
  const changed=()=>{if(!pendingRef.current)setKey(newId());};
  const perform=async(intent:MutationIntent)=>{
    if(inFlight.current)return;
    const recovering=pendingRef.current?.kind==="live"&&pendingRef.current.intent===intent;
    inFlight.current=true;setBusy(true);setNotice(undefined);
    if(intent.preserve)changePending({kind:"live",intent});
    try {
      const result=await intent.write();
      if(intent.preserve&&!matchesReceipt(intent.marker,result.receipt))
        throw Error("Ответ без совпадающего подтверждения записи.");
      const saved=result.receipt?.replayed?"Сохранено ранее. Повторная запись не выполнялась.":"Сохранено.";
      const correlation=result.receipt?.requestId?" Запрос: "+result.receipt.requestId:"";
      const original=result.receipt?.originalRequestId&&result.receipt.originalRequestId!==result.receipt.requestId?
        " Исходная запись: "+result.receipt.originalRequestId:"";
      const cleared=clearPending();
      const text=saved+correlation+original+(cleared?"":" Не удалось удалить отметку восстановления.");
      setNotice({tone:cleared?"success":"warning",text});if(cleared)setKey(newId());
      if(intent.refresh){
        try {await intent.refresh();}
        catch(error){setNotice({tone:"warning",text:text+" Список устарел: "+api.describeError(error)});}
      }
    } catch(error) {
      const failure=api.decodeError(error);
      // A later rejection cannot prove the outcome of an earlier attempt.
      if(intent.preserve&&(recovering||failure.outcome==="unconfirmed")){
        setNotice({tone:"warning",text:"Результат записи не подтверждён. "+failure.text+
          " Повторите именно исходную операцию."});
      }else{
        const cleared=clearPending();
        setNotice({tone:"danger",text:failure.text+(cleared?"":" Не удалось удалить отметку восстановления."),
          fields:failure.fieldViolations});
      }
    } finally {inFlight.current=false;setBusy(false);}
  };
  const run=async(write:MutationWrite,refresh?:()=>Promise<unknown>)=>{
    if(pendingRef.current)return;
    await perform({write,refresh,preserve:false});
  };
  const runIntent=async(prepare:()=>PreparedMutation,refresh?:()=>Promise<unknown>)=>{
    if(inFlight.current||pendingRef.current)return;
    let prepared:PreparedMutation;
    try {
      prepared=prepare();
      if(!recovery)throw Error("Отсутствует настройка восстановления операции.");
      // A newly mounted owner must not overwrite a previously pending key.
      const existing=restoredPending(recovery.storageKey);
      if(existing){
        changePending(existing);
        setNotice({tone:"warning",text:"Результат предыдущей записи не подтверждён. Сначала завершите восстановление."});
        return;
      }
      // If the marker cannot be written, do not send the command.
      sessionStorage.setItem(recovery.storageKey,JSON.stringify(prepared.marker));
    }catch(error){setNotice({tone:"danger",text:api.describeError(error)});return;}
    await perform({write:prepared.write,marker:prepared.marker,refresh,preserve:true});
  };
  const checkStored=async(marker:ReceiptMarker)=>{
    if(!recovery||inFlight.current)return;
    inFlight.current=true;setBusy(true);
    try {
      const receipt=await recovery.lookup(marker,AbortSignal.timeout(READ_TIMEOUT_MS));
      if(!matchesReceipt(marker,receipt)){
        setNotice({tone:"warning",text:"Результат записи не подтверждён. Совпадающее подтверждение отсутствует. Это не доказывает откат записи."});
        return;
      }
      const cleared=clearPending();
      const saved="Сохранено ранее. Подтверждение найдено. Исходная запись: "+receipt.originalRequestId+
        " Запрос проверки: "+receipt.requestId+(cleared?"":" Не удалось удалить отметку восстановления.");
      setNotice({tone:cleared?"success":"warning",text:saved});if(cleared)setKey(newId());
      try {await recovery.refresh();}
      catch(error){setNotice({tone:"warning",text:saved+" Список устарел: "+api.describeError(error)});}
    }catch(error){
      setNotice({tone:"warning",text:"Результат записи не подтверждён. "+api.describeError(error)});
    }finally{inFlight.current=false;setBusy(false);}
  };
  const retryPending=async()=>{
    const value=pendingRef.current;
    if(value?.kind==="live")await perform(value.intent);
    else if(value?.kind==="stored")await checkStored(value.marker);
  };
  const acknowledgeUnknown=()=>{
    if(inFlight.current||!pendingRef.current||pendingRef.current.kind==="live")return;
    if(!window.confirm("Результат прежней записи остаётся неизвестным. Вы сверили данные и понимаете риск повторной записи?"))return;
    if(clearPending()){
      setKey(newId());
      setNotice({tone:"warning",text:"Восстановление прекращено по вашему решению. Результат прежней записи не подтверждён."});
    }else setNotice({tone:"warning",text:"Не удалось удалить отметку восстановления."});
  };
  useEffect(()=>{
    // Permit navigation to inspect related records between recovery checks,
    // but retain the existing navigation guard during a write or lookup.
    if(!pending||(recoveryOnly&&!busy))return;
    const guard=(event:BeforeUnloadEvent)=>{event.preventDefault();event.returnValue="";};
    registerNavigationBlocker(()=>false);
    window.addEventListener("beforeunload",guard);
    return ()=>{
      registerNavigationBlocker(null);
      window.removeEventListener("beforeunload",guard);
    };
  },[pending,recoveryOnly,busy]);
  return {key,busy,notice,pending,recoveryOnly,canCheck:pendingState?.kind!=="unreadable",
    changed,run,runIntent,retryPending,acknowledgeUnknown};
}
export function MutationNotice({state}:{state:ReturnType<typeof useMutation>}) {
  return state.notice?<div className="crm-mutation-notice">
    <FeedbackPanel tone={state.notice.tone} title={state.notice.text}
      action={state.pending?<>{state.canCheck?<Button disabled={state.busy} onClick={()=>void state.retryPending()}>
        {state.recoveryOnly?"Проверить результат записи":"Повторить исходную операцию"}</Button>:null}
        {state.recoveryOnly?<Button variant="secondary" disabled={state.busy} onClick={state.acknowledgeUnknown}>
          Прекратить восстановление</Button>:null}</>:undefined}>
      {state.notice.fields?.length?<ul>{state.notice.fields.map((field,index)=>
        <li key={field.fieldPath+":"+index}>{field.fieldPath}: {field.description}</li>)}</ul>:null}
    </FeedbackPanel>
  </div>:null;
}
export function QueryNotice({query}:{query:{busy:boolean;error:string;stale?:boolean;reload?:()=>Promise<unknown>}}) {
  const {reload}=query;
  return query.error?<FeedbackPanel tone="danger" title={(query.stale?"Данные устарели. ":"")+query.error}
    action={reload?<Button onClick={()=>void reload().catch(()=>{})}>Повторить чтение</Button>:undefined}/>:
    query.busy?<p role="status">{query.stale?"Обновление… Показаны предыдущие данные.":"Загрузка…"}</p>:null;
}
export function Paging({next,busy,onNext,onFirst}:{next:string;busy:boolean;onNext:()=>void;onFirst:()=>void}) {
  return <p><Button variant="secondary" disabled={busy} onClick={onFirst}>В начало</Button>{" "}
    <Button variant="secondary" disabled={busy||!next} onClick={onNext}>Следующая страница</Button></p>;
}

export function useReadAction<T>() {
  const {state,execute}=useRead<T>();
  const run=useCallback(async(read:ReadLoad<T>)=>{
    try{return await execute(read);}catch{return undefined;}
  },[execute]);
  return {data:state?.data,error:state?.error??"",busy:state?.busy??false,run};
}
