import test from "node:test";
import assert from "node:assert/strict";
import {DatabaseSync} from "node:sqlite";
import {readFileSync} from "node:fs";

const migration = readFileSync("apps/cloudflare-crm/migrations/0001_initial.sql","utf8");
const reset = readFileSync("apps/cloudflare-crm/migrations/0002_hard_cutover.sql","utf8");
const taskAudit = readFileSync("apps/cloudflare-crm/migrations/0003_task_update_audit.sql","utf8");
const expected = [
 "crm_tenants","crm_users","crm_audit",
 "crm_party_records","crm_party_receipts","crm_party_import_receipts",
 "crm_account_records","crm_account_receipts",
 "crm_contact_point_records","crm_contact_point_receipts",
 "crm_party_relationship_records","crm_party_relationship_receipts",
 "crm_activity_task_records","crm_activity_task_receipts",
 "crm_sales_deal_records","crm_sales_deal_receipts",
 "crm_consent_records","crm_consent_receipts",
 "crm_sales_link_delivery_records",
 "crm_privacy_case_records","crm_privacy_case_receipts",
 "crm_enrichment_profiles","crm_enrichment_mappings","crm_enrichment_receipts",
];
function db(){
 const db = new DatabaseSync(":memory:");
 db.exec("PRAGMA foreign_keys=ON;");
 db.exec(migration);
 db.exec(reset);
 db.exec(taskAudit);
 return db;
}
test("new D1 starts with only original Rust owner tables and no pilot records",()=>{
 const database=db();
 try{
  const names=database.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'crm_%' ORDER BY name").all().map(x=>x.name);
  assert.deepEqual(names.sort(),[...expected].sort());
  for(const old of ["crm_customers","crm_contacts","crm_deals","crm_tasks"]){
   assert.ok(!names.includes(old),"pilot business table survived: "+old);
  }
  assert.deepEqual(database.prepare("PRAGMA foreign_key_check").all(),[]);
  assert.equal(database.prepare("SELECT COUNT(*) AS n FROM crm_party_records").get().n,0);
 }finally{database.close();}
});
test("hard cutover removes populated business state and preserves tenant principals",()=>{
 const database=db();
 try{
  database.exec("INSERT INTO crm_tenants VALUES('tenant-a','A'); INSERT INTO crm_users VALUES('service:fixture.access','tenant-a','admin',1);");
  database.prepare("INSERT INTO crm_party_records VALUES(?,?,?,?,?,?,?)").run("tenant-a","party","{}",1,"party-key","hash","request");
  database.prepare("INSERT INTO crm_account_records VALUES(?,?,?,?,?,?,?,?)").run("tenant-a","account","party","{}",1,"account-key","hash","request");
  database.prepare("INSERT INTO crm_activity_task_records VALUES(?,?,?,?,?,?,?,?)").run("tenant-a","task","account","{}",1,"task-key","hash","request");
  assert.equal(database.prepare("SELECT COUNT(*) AS n FROM crm_audit").get().n,3);
  const principals=database.prepare("SELECT * FROM crm_users").all();
  const tenants=database.prepare("SELECT * FROM crm_tenants").all();
  database.exec("BEGIN");database.exec(reset);database.exec("COMMIT");
  assert.deepEqual(database.prepare("SELECT * FROM crm_users").all(),principals);
  assert.deepEqual(database.prepare("SELECT * FROM crm_tenants").all(),tenants);
  for(const name of expected.filter(n=>n!=="crm_users"&&n!=="crm_tenants"))
   assert.equal(database.prepare("SELECT COUNT(*) AS n FROM "+name).get().n,0,name);
  assert.deepEqual(database.prepare("PRAGMA foreign_key_check").all(),[]);
  database.prepare("INSERT INTO crm_party_records VALUES(?,?,?,?,?,?,?)").run("tenant-a","after","{}",1,"after-key","hash","after-request");
  assert.equal(database.prepare("SELECT request_id FROM crm_audit").get().request_id,"after-request");
  // Failed migration transactions retain the pre-migration business state.
  database.exec("BEGIN");database.exec(reset);
  assert.throws(()=>database.exec("INSERT INTO missing_table VALUES(1)"));
  database.exec("ROLLBACK");
  assert.equal(database.prepare("SELECT id FROM crm_party_records").get().id,"after");
 }finally{database.close();}
});
test("original Party mutation, idempotency and correlated audit use one atomic D1",()=>{
 const database=db();
 try{
  database.exec("INSERT INTO crm_tenants(id,name) VALUES('tenant-a','A')");
  const insert=database.prepare("INSERT INTO crm_party_records(tenant,id,state,version,idem_key,idem_digest,request_id) VALUES(?,?,?,?,?,?,?)");
  insert.run("tenant-a","p-1","{}",1,"idem-create-1","hash-1","request-1");
  assert.equal(database.prepare("SELECT COUNT(*) AS n FROM crm_party_receipts").get().n,1);
  assert.equal(database.prepare("SELECT request_id FROM crm_audit WHERE subject_id='p-1'").get().request_id,"request-1");
  const change=database.prepare("UPDATE crm_party_records SET state=?,version=?,idem_key=?,idem_digest=?,request_id=? WHERE tenant=? AND id=? AND version=?");
  assert.equal(change.run('{"v":2}',2,"idem-update-1","hash-2","request-2","tenant-a","p-1",1).changes,1);
  assert.equal(change.run('{"v":9}',9,"idem-stale-1","hash-9","request-3","tenant-a","p-1",1).changes,0);
  assert.equal(database.prepare("SELECT COUNT(*) AS n FROM crm_party_receipts").get().n,2);
  assert.deepEqual(database.prepare("SELECT request_id FROM crm_audit WHERE subject_id='p-1' ORDER BY rowid").all().map(x=>x.request_id),["request-1","request-2"]);
  assert.throws(()=>insert.run("tenant-a","p-2","{}",1,"idem-create-1","other","request-4"),/constraint/i);
  assert.equal(database.prepare("SELECT COUNT(*) AS n FROM crm_party_records").get().n,1);
  assert.deepEqual(database.prepare("PRAGMA foreign_key_check").all(),[]);
 }finally{database.close();}
});

test("task audit upgrade from 0002 and fresh 0003 distinguish update/complete atomically",()=>{
 for(const upgrade of [false,true]){
  const database=upgrade?new DatabaseSync(":memory:"):db();
  try{
   if(upgrade){
    database.exec("PRAGMA foreign_keys=ON;");
    database.exec(migration);
    database.exec(reset);
   }
   database.exec("INSERT INTO crm_tenants(id,name) VALUES('tenant-a','A')");
   database.prepare("INSERT INTO crm_party_records VALUES(?,?,?,?,?,?,?)")
    .run("tenant-a","party","{}",1,"party-key","hash","party-request");
   database.prepare("INSERT INTO crm_account_records VALUES(?,?,?,?,?,?,?,?)")
    .run("tenant-a","account","party","{}",1,"account-key","hash","account-request");
   database.prepare("INSERT INTO crm_activity_task_records VALUES(?,?,?,?,?,?,?,?)")
    .run("tenant-a","task","account",JSON.stringify({status:"open",subject:"Original"}),1,
      "task-create","hash-create","task-request-create");
   if(upgrade)database.exec(taskAudit);
   const change=database.prepare("UPDATE crm_activity_task_records SET state=?,version=?,idem_key=?,idem_digest=?,request_id=? WHERE tenant=? AND id=? AND version=?");
   assert.equal(change.run(JSON.stringify({status:"open",subject:"Edited"}),2,
     "task-update","hash-update","task-request-update","tenant-a","task",1).changes,1);
   assert.equal(change.run(JSON.stringify({status:"open",subject:"Stale"}),99,
     "task-stale","hash-stale","task-request-stale","tenant-a","task",1).changes,0);
   assert.equal(change.run(JSON.stringify({status:"completed",subject:"Edited"}),3,
     "task-complete","hash-complete","task-request-complete","tenant-a","task",2).changes,1);
   const actions=database.prepare("SELECT action,request_id FROM crm_audit WHERE subject_id='task' ORDER BY rowid").all().map(({action,request_id})=>({action,request_id}));
   assert.deepEqual(actions,[
     {action:"activity_tasks.create",request_id:"task-request-create"},
     {action:"activity_tasks.update",request_id:"task-request-update"},
     {action:"activity_tasks.complete",request_id:"task-request-complete"},
   ]);
   assert.equal(database.prepare("SELECT COUNT(*) AS n FROM crm_activity_task_receipts WHERE tenant='tenant-a'").get().n,3);
   const stateBefore=database.prepare("SELECT state,version FROM crm_activity_task_records WHERE tenant='tenant-a' AND id='task'").get();
   assert.throws(()=>change.run(JSON.stringify({status:"completed",subject:"Wrong replay"}),4,
     "task-update","other-digest","bad-replay","tenant-a","task",3),/constraint/i);
   assert.deepEqual(database.prepare("SELECT state,version FROM crm_activity_task_records WHERE tenant='tenant-a' AND id='task'").get(),stateBefore);
   assert.deepEqual(database.prepare("SELECT action,request_id FROM crm_audit WHERE subject_id='task' ORDER BY rowid").all().map(({action,request_id})=>({action,request_id})),actions);
   assert.deepEqual(database.prepare("PRAGMA foreign_key_check").all(),[]);
  }finally{database.close();}
 }
});
