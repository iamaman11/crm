import test from "node:test";
import assert from "node:assert/strict";
import {DatabaseSync} from "node:sqlite";
import {readFileSync} from "node:fs";

const migration = readFileSync("apps/cloudflare-crm/migrations/0001_initial.sql","utf8");
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
