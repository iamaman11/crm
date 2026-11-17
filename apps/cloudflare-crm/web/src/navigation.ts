import {accountCreateHref,partyHref} from "./routes";

/** One native Browser History owner. URL is the only route snapshot; no React route mirror. */
type Listener=()=>void;
type LeaveGuard=()=>boolean;
const listeners=new Set<Listener>();
let subscribed=false;
let acceptedHash=window.location.hash;
let acceptedKey=window.navigation?.currentEntry?.key;
let acceptedState:unknown=window.history.state;
let leaveGuard:LeaveGuard|null=null;
let reversing=false;

export function currentRouteHash():string{return acceptedHash;}
function notify():void {for(const listener of listeners)listener();}
function acceptLocation():void {
  const hash=window.location.hash;
  acceptedKey=window.navigation?.currentEntry?.key;
  acceptedState=window.history.state;
  if(hash===acceptedHash)return;
  acceptedHash=hash;
  notify();
}
function restoreAcceptedLocation():void {
  // Both a newly assigned hash and Back/Forward can emit popstate.
  // Use the browser's actual entry identity instead of guessing direction.
  reversing=true;
  const navigation=window.navigation;
  const restoreUrl=()=>{
    if(!reversing)return;
    window.history.replaceState(acceptedState,"",pathWithHash(acceptedHash));
    acceptedKey=window.navigation?.currentEntry?.key;
    reversing=false;
  };
  if(navigation&&acceptedKey&&navigation.entries().some(entry=>entry.key===acceptedKey)){
    try {
      const traversal=navigation.traverseTo(acceptedKey);
      void traversal.committed.catch(restoreUrl);
      void traversal.finished.catch(restoreUrl);
    } catch {
      restoreUrl();
    }
  }else{
    // Fail closed for an unavailable/evicted entry: retain the form and URL,
    // rather than risk navigating away by guessing a history direction.
    restoreUrl();
  }
}
function onLocationChanged():void {
  const next=window.location.hash;
  if(reversing) {
    if(next===acceptedHash)reversing=false;
    return;
  }
  if(next===acceptedHash)return;
  if(leaveGuard!==null&&!leaveGuard()) {
    restoreAcceptedLocation();
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
    subscribed=true;
  }
  return ()=>{
    listeners.delete(listener);
    if(subscribed&&listeners.size===0) {
      window.removeEventListener("popstate",onLocationChanged);
      window.removeEventListener("hashchange",onLocationChanged);
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

/** Preserve the source Party as an addressable URL before entering Account creation. */
export function openAccountFromParty(partyId:string):boolean {
  if(!mayLeave())return false;
  const from=partyHref(partyId);
  window.history.replaceState({crmPartyScroll:window.scrollY},"",pathWithHash(from));
  window.history.pushState({crmReturnToParty:from},"",pathWithHash(accountCreateHref(partyId)));
  acceptLocation();
  return true;
}
export function returnToParty(partyId:string):boolean {
  if(!mayLeave())return false;
  const from=partyHref(partyId);
  const state:unknown=window.history.state;
  if(state!==null&&typeof state==="object"&&"crmReturnToParty" in state&&
      state.crmReturnToParty===from){
    window.history.back();
  }else{
    window.history.pushState(null,"",pathWithHash(from));
    acceptLocation();
  }
  return true;
}
export function savedPartyListScroll():number|null {
  const state:unknown=window.history.state;
  return state!==null&&typeof state==="object"&&"crmPartyScroll" in state&&
    typeof state.crmPartyScroll==="number"&&Number.isFinite(state.crmPartyScroll)
    ?state.crmPartyScroll:null;
}
/** Create was already confirmed: replace draft URL, never replay its form via Back. */
export function replaceCommittedAccount(hash:string):void {
  window.history.replaceState(null,"",pathWithHash(hash));
  acceptLocation();
}
