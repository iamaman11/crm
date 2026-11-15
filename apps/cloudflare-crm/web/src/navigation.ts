/** One native Browser History owner. URL is the only route snapshot; no React route mirror. */
type Listener=()=>void;
type LeaveGuard=()=>boolean;
const listeners=new Set<Listener>();
let subscribed=false;
let acceptedHash=window.location.hash;
let leaveGuard:LeaveGuard|null=null;
let reversing=false;

export function currentRouteHash():string{return window.location.hash;}
function notify():void {for(const listener of listeners)listener();}
function acceptLocation():void {
  const hash=currentRouteHash();
  if(hash===acceptedHash)return;
  acceptedHash=hash;
  notify();
}
function onLocationChanged(event:Event):void {
  const next=currentRouteHash();
  // A restored BFCache document can resume with a different history entry.
  // pageshow is a native resynchronization signal, not a navigation attempt.
  if(event.type==="pageshow") {
    reversing=false;
    acceptLocation();
    return;
  }
  if(reversing) {
    // A rejected browser Back/hash change is being undone without rewriting history.
    if(next===acceptedHash)reversing=false;
    return;
  }
  if(next===acceptedHash)return; // popstate + hashchange may both fire for one transition
  if(leaveGuard!==null&&!leaveGuard()) {
    reversing=true;
    if(event.type==="popstate")window.history.forward();
    else window.history.back();
    return;
  }
  leaveGuard=null;
  acceptLocation();
}
export function subscribeNavigation(listener:Listener):()=>void {
  listeners.add(listener);
  if(!subscribed) {
    window.addEventListener("popstate",onLocationChanged);
    window.addEventListener("hashchange",onLocationChanged);
    window.addEventListener("pageshow",onLocationChanged);
    subscribed=true;
  }
  return ()=>{
    listeners.delete(listener);
    if(subscribed&&listeners.size===0) {
      window.removeEventListener("popstate",onLocationChanged);
      window.removeEventListener("hashchange",onLocationChanged);
      window.removeEventListener("pageshow",onLocationChanged);
      subscribed=false;
    }
  };
}
export function registerNavigationBlocker(blocker:LeaveGuard|null):void {leaveGuard=blocker;}
function mayLeave():boolean {
  if(leaveGuard!==null&&!leaveGuard())return false;
  leaveGuard=null;
  return true;
}
function pathWithHash(hash:string):string {
  return window.location.pathname+window.location.search+hash;
}
export function navigateTo(hash:string):boolean {
  if(hash===currentRouteHash())return true;
  if(!mayLeave())return false;
  window.history.pushState(null,"",pathWithHash(hash));
  acceptLocation(); // pushState does not emit a native navigation event
  return true;
}
export function openAccountFromList(hash:string):boolean {
  if(!mayLeave())return false;
  const from=currentRouteHash()||"#accounts";
  window.history.replaceState({crmAccountScroll:window.scrollY},"",window.location.href);
  window.history.pushState({crmReturnTo:from},"",pathWithHash(hash));
  acceptLocation();
  return true;
}
export function returnToList(fallback:string):boolean {
  if(!mayLeave())return false;
  const state:unknown=window.history.state;
  if(state!==null&&typeof state==="object"&&"crmReturnTo" in state&&
      typeof state.crmReturnTo==="string"&&
      (state.crmReturnTo==="#accounts"||state.crmReturnTo.startsWith("#accounts?"))) {
    window.history.back();
  }else{
    window.history.pushState(null,"",pathWithHash(fallback));
    acceptLocation();
  }
  return true;
}
export function savedAccountListScroll():number|null {
  const state:unknown=window.history.state;
  return state!==null&&typeof state==="object"&&"crmAccountScroll" in state&&
    typeof state.crmAccountScroll==="number"&&Number.isFinite(state.crmAccountScroll)
    ?state.crmAccountScroll:null;
}
