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

// Quality and identity read-only original Rust owners, same generated descriptors.
const qualityService=registry.getService("crm.data_quality.v2.DataQualityService");
if(!qualityService)throw Error("compiled DataQuality v2 service missing");
const qualityMethod=name=>{
 const method=qualityService.methods.find(item=>item.name===name);
 if(!method)throw Error("compiled DataQuality v2 RPC missing "+name);
 return method;
};
export const qualityV2Path=name=>"/crm.data_quality.v2.DataQualityService/"+qualityMethod(name).name;
export const binaryQualityV2=(name,payload)=>{
 const method=qualityMethod(name);
 return toBinary(method.input,create(method.input,payload));
};
export const decodeQualityV2=(name,bytes)=>fromBinary(qualityMethod(name).output,bytes);

const identityService=registry.getService("crm.identity_resolution.v2.IdentityResolutionService");
if(!identityService)throw Error("compiled IdentityResolution v2 service missing");
const identityMethod=name=>{
 const method=identityService.methods.find(item=>item.name===name);
 if(!method)throw Error("compiled IdentityResolution v2 RPC missing "+name);
 return method;
};
export const identityV2Path=name=>"/crm.identity_resolution.v2.IdentityResolutionService/"+identityMethod(name).name;
export const binaryIdentityV2=(name,payload)=>{
 const method=identityMethod(name);
 return toBinary(method.input,create(method.input,payload));
};
export const decodeIdentityV2=(name,bytes)=>fromBinary(identityMethod(name).output,bytes);

// Original Consent Authorization service, descriptor-derived binary Connect.
const consentService=registry.getService("crm.consents.v2.ConsentAuthorizationService");
if(!consentService)throw Error("compiled Consents v2 service missing");
const consentMethod=name=>{
 const method=consentService.methods.find(item=>item.name===name);
 if(!method)throw Error("compiled Consent v2 RPC missing "+name);
 return method;
};
export const consentV2Path=name=>"/crm.consents.v2.ConsentAuthorizationService/"+consentMethod(name).name;
export const binaryConsentV2=(name,payload)=>{
 const method=consentMethod(name);
 return toBinary(method.input,create(method.input,payload));
};
export const decodeConsentV2=(name,bytes)=>fromBinary(consentMethod(name).output,bytes);

// Original Deal v2 binary Connect generated through the same Buf descriptor.
const salesV2Service=registry.getService("crm.sales.v2.DealService");
if(!salesV2Service)throw Error("compiled Sales v2 service missing");
const salesV2Method=name=>{
 const method=salesV2Service.methods.find(item=>item.name===name);
 if(!method)throw Error("compiled Sales v2 RPC missing "+name);
 return method;
};
export const salesV2Path=name=>"/crm.sales.v2.DealService/"+salesV2Method(name).name;
export const binarySalesV2=(name,payload)=>{
 const method=salesV2Method(name);
 return toBinary(method.input,create(method.input,payload));
};
export const decodeSalesV2=(name,bytes)=>fromBinary(salesV2Method(name).output,bytes);



const privacyService=registry.getService("crm.customer_privacy.v2.PrivacyCaseService");
if(!privacyService)throw Error("compiled PrivacyCase v2 service missing");
const privacyMethod=name=>{
 const method=privacyService.methods.find(item=>item.name===name);
 if(!method)throw Error("compiled PrivacyCase v2 RPC missing "+name);
 return method;
};
export const privacyV2Path=name=>"/crm.customer_privacy.v2.PrivacyCaseService/"+privacyMethod(name).name;
export const binaryPrivacyV2=(name,payload)=>{
 const method=privacyMethod(name);
 return toBinary(method.input,create(method.input,payload));
};
export const decodePrivacyV2=(name,bytes)=>fromBinary(privacyMethod(name).output,bytes);

const enrichmentService=registry.getService("crm.customer_enrichment.v2.EnrichmentConfigurationService");
if(!enrichmentService)throw Error("compiled EnrichmentConfiguration v2 service missing");
const enrichmentMethod=name=>{
 const method=enrichmentService.methods.find(item=>item.name===name);
 if(!method)throw Error("compiled EnrichmentConfiguration v2 RPC missing "+name);
 return method;
};
export const enrichmentV2Path=name=>"/crm.customer_enrichment.v2.EnrichmentConfigurationService/"+enrichmentMethod(name).name;
export const binaryEnrichmentV2=(name,payload)=>{
 const method=enrichmentMethod(name);
 return toBinary(method.input,create(method.input,payload));
};
export const decodeEnrichmentV2=(name,bytes)=>fromBinary(enrichmentMethod(name).output,bytes);

const csvService=registry.getService("crm.customer_data_operations.v2.PartyCsvService");
if(!csvService)throw Error("compiled PartyCsv v2 service missing");
const csvMethod=name=>{
 const method=csvService.methods.find(item=>item.name===name);
 if(!method)throw Error("compiled PartyCsv v2 RPC missing "+name);
 return method;
};
export const csvV2Path=name=>"/crm.customer_data_operations.v2.PartyCsvService/"+csvMethod(name).name;
export const binaryCsvV2=(name,payload)=>{
 const method=csvMethod(name);return toBinary(method.input,create(method.input,payload));
};
export const decodeCsvV2=(name,bytes)=>fromBinary(csvMethod(name).output,bytes);

const overviewService=registry.getService("crm.customer_360.v2.Customer360Service");
if(!overviewService)throw Error("compiled Customer360 v2 service missing");
const overviewMethod=overviewService.methods.find(item=>item.name==="GetAccountOverview");
if(!overviewMethod)throw Error("compiled GetAccountOverview missing");
export const overviewV2Path=()=>"/crm.customer_360.v2.Customer360Service/GetAccountOverview";
export const binaryOverviewV2=(_name,payload)=>toBinary(overviewMethod.input,create(overviewMethod.input,payload));
export const decodeOverviewV2=(_name,bytes)=>fromBinary(overviewMethod.output,bytes);
