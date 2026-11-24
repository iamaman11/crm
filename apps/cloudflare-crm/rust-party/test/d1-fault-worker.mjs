// Test-only entrypoint. Import the sealed production Rust Worker unchanged.
// The real D1 binding retains its prototype, queries, transactions and storage.
// Only the returned acknowledgement of one successful CreateDeal INSERT is lost.
import Worker from "../build/index.js";

// worker-build 0.9.0 exports a WorkerEntrypoint class (via Proxy),
// not an object with a static fetch. Instantiate it with the actual workerd
// execution context and request-scoped bindings, preserving its generated shim.
function dispatch(request,env,ctx){
  return new Worker(ctx,env).fetch(request);
}

function withLostAcknowledgement(db,mode){
  const wrap=statement=>new Proxy(statement,{
    get(target,property){
      if(property==="bind")return (...args)=>wrap(target.bind(...args));
      if(property==="run")return async(...args)=>{
        const committed=await target.run(...args);
        if(!committed.success||!(committed.meta?.changes>0))
          throw Error("D1 fault fixture did not commit");
        if(mode==="receipt-read")
          await db.prepare("ALTER TABLE crm_sales_deal_receipts RENAME TO fixture_fault_receipts").run();
        throw Error("D1_ERROR: test-only lost write acknowledgement");
      };
      const value=Reflect.get(target,property,target);
      return typeof value==="function"&&property!=="constructor"?value.bind(target):value;
    }
  });
  return new Proxy(db,{
    get(target,property){
      if(property==="prepare")return query=>{
        const statement=target.prepare(query);
        return query.startsWith("INSERT INTO crm_sales_deal_records(")?wrap(statement):statement;
      };
      const value=Reflect.get(target,property,target);
      return typeof value==="function"&&property!=="constructor"?value.bind(target):value;
    }
  });
}
export default {
  fetch(request,env,ctx){
    const mode=request.headers.get("x-test-d1-fault");
    if(mode===null)return dispatch(request,env,ctx);
    if(!["lost-ack","receipt-read"].includes(mode)||
        new URL(request.url).pathname!=="/crm.sales.v2.DealService/CreateDeal")
      throw Error("Invalid test-only D1 fault");
    const headers=new Headers(request.headers);
    headers.delete("x-test-d1-fault");
    return dispatch(new Request(request,{headers}),{...env,DB:withLostAcknowledgement(env.DB,mode)},ctx);
  }
};
