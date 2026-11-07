import test from "node:test";
import assert from "node:assert/strict";
import { readCustomer360, CUSTOMER360_LIMIT } from "../features/customer360/query.mjs";
import { crmFetch } from "../worker/index.mjs";

const tenant = "tenant-a", id = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
function makeDb(options={}) {
  const queries=[];
  const rows=options.rows || {
    contacts:[{ id:"c1", name:"Anna",customer_id:id }],
    deals:[{ id:"d1",title:"Deal",stage:"new",amount:125,customer_id:id }],
    tasks:[{ id:"t1",title:"Call",status:"open",due_date:"2026-01-01",customer_id:id },
      { id:"t2",title:"Closed",status:"done",due_date:"2026-01-01",customer_id:id }],
  };
  const db={queries,prepare(sql){return {bind(...args){queries.push({sql,args});return {
    async first(){
      if(sql.includes("FROM crm_users")) return {tenant_id:tenant,role:"admin"};
      if(sql.includes("FROM crm_customers"))return options.missing ? null : {id,name:"ACME",kind:"organization"};
      const type=Object.keys(rows).find(k=>sql.includes("crm_"+k));
      if(sql.includes("COUNT(*)"))return {total:rows[type]?.length ?? 0};
      return null;
    },
    async all(){const type=Object.keys(rows).find(k=>sql.includes("crm_"+k));
      return {results:(rows[type]||[]).slice(0,CUSTOMER360_LIMIT)};},
  };}}}};
  return db;
}
test("Customer 360 derives a coherent bounded read-only view without writes",async()=>{
  const db=makeDb();
  const view=await readCustomer360(db,tenant,id,"2026-11-08");
  assert.equal(view.customer.name,"ACME");
  assert.deepEqual(view.summary,{contacts:1,deals:1,tasks:2,open_tasks:1,overdue_tasks:1,deal_amount:125});
  assert.equal(view.related.tasks.limit,100);
  assert.equal(db.queries.filter(q=>q.sql.includes("INSERT")||q.sql.includes("UPDATE")||q.sql.includes("DELETE")).length,0);
  for(const query of db.queries) {
    assert.ok(query.sql.includes("tenant_id = ?"),"must scope to tenant");
    if(!query.sql.includes("crm_customers"))assert.ok(query.sql.includes("customer_id = ?"));
    assert.ok(query.args.includes(tenant));
  }
});
test("Customer 360 gives no related queries if customer is missing or foreign",async()=>{
  const db=makeDb({missing:true});
  assert.equal(await readCustomer360(db,tenant,id),null);
  assert.equal(db.queries.length,1);
});
test("Customer 360 does not claim complete totals when window is truncated",async()=>{
  const rows={contacts:[],deals:Array.from({length:101},(_,i)=>({id:String(i),amount:1})),
    tasks:Array.from({length:101},(_,i)=>({id:String(i),status:"open",due_date:"2026-01-01"}))};
  const v=await readCustomer360(makeDb({rows}),tenant,id,"2026-11-08");
  assert.equal(v.summary.deals,101);
  assert.equal(v.summary.deal_amount,null);
  assert.equal(v.summary.open_tasks,null);
  assert.equal(v.summary.overdue_tasks,null);
  assert.equal(v.related.deals.items.length,100);
});
test("Customer 360 HTTP requires Access, returns correlated 404 for foreign ID",async()=>{
  const url="https://crm.test/api/customer360/"+id;
  const denied=await crmFetch(new Request(url),{DB:makeDb()}, {});
  assert.equal(denied.status,401);
  const foreign=await crmFetch(new Request(url),{DB:makeDb({missing:true})},
    {access:{getIdentity:async()=>({email:"a@example.test"})}});
  assert.equal(foreign.status,404);
  assert.equal(foreign.headers.get("x-crm-error-code"),"NOT_FOUND");
  assert.ok(foreign.headers.get("x-request-id"));
});
