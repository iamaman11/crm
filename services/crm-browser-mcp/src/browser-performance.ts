/** Install BEFORE document navigation via Playwright context.addInitScript.
 * This is provisional per-page observation, NOT official finalized CrUX INP. */
export const WEB_VITALS_INIT = `(() => {
  const types = typeof PerformanceObserver === 'function'
    ? (PerformanceObserver.supportedEntryTypes || []) : [];
  const supported = kind => types.includes(kind);
  const state = {
    source:'PerformanceObserver', phase:'provisional',
    supported:{ lcp:supported('largest-contentful-paint'), cls:supported('layout-shift'),
                inpCandidate:supported('event') },
    lcpMs:null, cls:supported('layout-shift') ? 0 : null,
    inpCandidateMs:null, observedInteractions:0
  };
  Object.defineProperty(window,'__mcpVitals',{value:state, configurable:true});
  if (typeof PerformanceObserver !== 'function') return;
  if (state.supported.lcp) try {
    new PerformanceObserver(list => {
      for (const e of list.getEntries()) state.lcpMs = Math.round(e.startTime);
    }).observe({type:'largest-contentful-paint',buffered:true});
  } catch { state.supported.lcp=false; }
  if (state.supported.cls) try {
    let start=-1,last=-1,score=0;
    new PerformanceObserver(list => {
      for (const e of list.getEntries()) {
        if (e.hadRecentInput || !Number.isFinite(e.value)) continue;
        if (start<0 || e.startTime-last>1000 || e.startTime-start>5000) {
          start=e.startTime;score=0;
        }
        score+=e.value;last=e.startTime;
        state.cls=Math.round(Math.max(state.cls||0,score)*10000)/10000;
      }
    }).observe({type:'layout-shift',buffered:true});
  } catch { state.supported.cls=false;state.cls=null; }
  if (state.supported.inpCandidate) try {
    const interactions=new Map();
    new PerformanceObserver(list => {
      for (const e of list.getEntries()) {
        if (!e.interactionId || !Number.isFinite(e.duration)) continue;
        interactions.set(e.interactionId,Math.max(interactions.get(e.interactionId)||0,e.duration));
        if (interactions.size>2000) interactions.delete(interactions.keys().next().value);
      }
      const ordered=Array.from(interactions.values()).sort((a,b)=>b-a);
      state.observedInteractions=ordered.length;
      state.inpCandidateMs=ordered.length
        ? Math.round(ordered[Math.min(ordered.length-1,Math.floor(ordered.length/50))])
        : null;
    }).observe({type:'event',durationThreshold:40,buffered:true});
  } catch { state.supported.inpCandidate=false; }
})()`;
export type WebVitals = {
  source:"PerformanceObserver";phase:"provisional";
  supported:{lcp:boolean;cls:boolean;inpCandidate:boolean};
  lcpMs:number|null;cls:number|null;inpCandidateMs:number|null;
  observedInteractions:number;
};
export function readVitals(input: WebVitals | undefined): WebVitals | null {
  if (!input || input.source!=="PerformanceObserver" || input.phase!=="provisional") return null;
  if (!input.supported || Object.values(input.supported).some(v=>typeof v!=="boolean")) return null;
  const valid=(v:number|null)=>v===null || (typeof v==="number" && Number.isFinite(v) && v>=0);
  if (!valid(input.lcpMs)||!valid(input.cls)||!valid(input.inpCandidateMs)||
    !Number.isSafeInteger(input.observedInteractions)||input.observedInteractions<0) return null;
  return {source:"PerformanceObserver",phase:"provisional",
    supported:{...input.supported},lcpMs:input.lcpMs,cls:input.cls,
    inpCandidateMs:input.inpCandidateMs,observedInteractions:input.observedInteractions};
}
