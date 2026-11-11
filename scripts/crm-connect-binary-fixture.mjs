// Test-only RPC wire data generated from the exact compiled Buf FileDescriptorSet.
// No handwritten Protobuf codec, separate JSON DTO or production network client.
import {readFileSync} from "node:fs";
import {createRequire} from "node:module";
import {dirname,resolve} from "node:path";
import {fileURLToPath} from "node:url";

const root=resolve(dirname(fileURLToPath(import.meta.url)),"..");
const requireClient=createRequire(resolve(root,"packages/client/package.json"));
const {createFileRegistry,create,toBinary,fromBinary}=requireClient("@bufbuild/protobuf");
const {FileDescriptorSetSchema}=requireClient("@bufbuild/protobuf/wkt");
const descriptorPath=process.env.CRM_PROTO_DESCRIPTOR_FILE||"/tmp/crm-proto-descriptors.binpb";
const registry=createFileRegistry(fromBinary(
  FileDescriptorSetSchema,readFileSync(descriptorPath)));
const service=registry.getService("crm.parties.v2.PartyService");
const method=service?.methods.find(item=>item.name==="GetParty");
if(!method)throw Error("compiled Party v2 GetParty descriptor missing");

export const binaryPartyGet=id=>toBinary(method.input,create(method.input,{partyId:id}));
export const decodePartyGet=bytes=>fromBinary(method.output,bytes);
export const partyGetPath="/crm.parties.v2.PartyService/GetParty";


// Same compiled descriptor, no separate v3 handwritten wire layout.
// This G3 candidate is invoked only through the signed native-ci workerd.
const partyV3Service=registry.getService("crm.parties.v3.PartyService");
if(!partyV3Service)throw Error("compiled Party v3 service missing");
const v3Method=name=>{
 const item=partyV3Service.methods.find(method=>method.name===name);
 if(!item)throw Error("compiled Party v3 RPC missing: "+name);
 return item;
};
export const partyV3Path=name=>"/crm.parties.v3.PartyService/"+v3Method(name).name;
export const binaryPartyV3=(name,payload)=>{
 const rpc=v3Method(name);
 return toBinary(rpc.input,create(rpc.input,payload));
};
export const decodePartyV3=(name,bytes)=>fromBinary(v3Method(name).output,bytes);


// Account v2 wire is derived from canonical Buf descriptors, not handwritten.
const accountV2Service=registry.getService("crm.accounts.v2.AccountService");
if(!accountV2Service)throw Error("compiled Account v2 service missing");
const accountV2Method=name=>{
 const item=accountV2Service.methods.find(method=>method.name===name);
 if(!item)throw Error("compiled Account v2 RPC missing: "+name);
 return item;
};
export const accountV2Path=name=>"/crm.accounts.v2.AccountService/"+accountV2Method(name).name;
export const binaryAccountV2=(name,payload)=>{
 const rpc=accountV2Method(name);
 return toBinary(rpc.input,create(rpc.input,payload));
};
export const decodeAccountV2=(name,bytes)=>fromBinary(accountV2Method(name).output,bytes);


// Contact Points v2: real generated Buffer messages from the canonical schema.
const contactV2Service=registry.getService("crm.contact_points.v2.ContactPointService");
if(!contactV2Service)throw Error("compiled ContactPoint v2 service missing");
const contactV2Method=name=>{
 const item=contactV2Service.methods.find(method=>method.name===name);
 if(!item)throw Error("compiled ContactPoint v2 RPC missing: "+name);
 return item;
};
export const contactV2Path=name=>"/crm.contact_points.v2.ContactPointService/"+contactV2Method(name).name;
export const binaryContactV2=(name,payload)=>{
 const rpc=contactV2Method(name);
 return toBinary(rpc.input,create(rpc.input,payload));
};
export const decodeContactV2=(name,bytes)=>fromBinary(contactV2Method(name).output,bytes);

// Party Relationships v2: same descriptor-derived binary Connect wire.
const relationshipService=registry.getService("crm.party_relationships.v2.PartyRelationshipService");
if(!relationshipService)throw Error("compiled PartyRelationship v2 service missing");
const relationshipMethod=name=>{
 const method=relationshipService.methods.find(item=>item.name===name);
 if(!method)throw Error("compiled PartyRelationship v2 RPC missing "+name);
 return method;
};
export const relationshipV2Path=name=>"/crm.party_relationships.v2.PartyRelationshipService/"+relationshipMethod(name).name;
export const binaryRelationshipV2=(name,payload)=>{
 const rpc=relationshipMethod(name);
 return toBinary(rpc.input,create(rpc.input,payload));
};
export const decodeRelationshipV2=(name,bytes)=>fromBinary(relationshipMethod(name).output,bytes);

// Activities/Tasks v2: signed native-ci uses compiled Buf descriptors.
const taskService=registry.getService("crm.activities.v2.TaskService");
if(!taskService)throw Error("compiled Task v2 service missing");
const taskMethod=name=>{
 const method=taskService.methods.find(item=>item.name===name);
 if(!method)throw Error("compiled Task v2 RPC missing "+name);
 return method;
};
export const taskV2Path=name=>"/crm.activities.v2.TaskService/"+taskMethod(name).name;
export const binaryTaskV2=(name,payload)=>{
 const rpc=taskMethod(name);
 return toBinary(rpc.input,create(rpc.input,payload));
};
export const decodeTaskV2=(name,bytes)=>fromBinary(taskMethod(name).output,bytes);
