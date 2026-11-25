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
// One descriptor-owned codec for every test RPC. This helper NEVER defines
// protobuf fields, wire tags or a parallel schema: Buf FileDescriptorSet is
// the only source of request/response types and method paths.
export function decodeRpcOutput(path, bytes){
 const [service, name]=path.slice(1).split("/");
 const method=registry.getService(service)?.methods.find(m=>m.name===name);
 if(!method)throw Error("Compiled Buf RPC missing: "+path);
 return fromBinary(method.output,bytes);
}
const serviceWire=name=>{
 const service=registry.getService(name);
 if(!service)throw Error("Compiled Buf service missing: "+name);
 const method=methodName=>{
   const item=service.methods.find(x=>x.name===methodName);
   if(!item)throw Error("Compiled Buf RPC missing: "+name+"/"+methodName);
   return item;
 };
 return {
   path:methodName=>"/"+name+"/"+method(methodName).name,
   binary:(methodName,payload)=>{
     const input=method(methodName).input;
     return toBinary(input,create(input,payload));
   },
   decode:(methodName,bytes)=>fromBinary(method(methodName).output,bytes)
 };
};
const partyV3=serviceWire("crm.parties.v3.PartyService");
export const partyV3Path=partyV3.path;
export const binaryPartyV3=partyV3.binary;
export const decodePartyV3=partyV3.decode;
const accountV2=serviceWire("crm.accounts.v2.AccountService");
export const accountV2Path=accountV2.path;
export const binaryAccountV2=accountV2.binary;
export const decodeAccountV2=accountV2.decode;
const contactV2=serviceWire("crm.contact_points.v2.ContactPointService");
export const contactV2Path=contactV2.path;
export const binaryContactV2=contactV2.binary;
export const decodeContactV2=contactV2.decode;
const relationshipV2=serviceWire("crm.party_relationships.v2.PartyRelationshipService");
export const relationshipV2Path=relationshipV2.path;
export const binaryRelationshipV2=relationshipV2.binary;
export const decodeRelationshipV2=relationshipV2.decode;
const memberV2=serviceWire("crm.access.v2.MemberService");
export const memberV2Path=memberV2.path;
export const binaryMemberV2=memberV2.binary;
export const decodeMemberV2=memberV2.decode;
const taskV2=serviceWire("crm.activities.v2.TaskService");
export const taskV2Path=taskV2.path;
export const binaryTaskV2=taskV2.binary;
export const decodeTaskV2=taskV2.decode;
const automationV2=serviceWire("crm.activities.v2.AutomationService");
export const automationV2Path=automationV2.path;
export const binaryAutomationV2=automationV2.binary;
export const decodeAutomationV2=automationV2.decode;
const interactionV2=serviceWire("crm.activities.v2.InteractionService");
export const interactionV2Path=interactionV2.path;
export const binaryInteractionV2=interactionV2.binary;
export const decodeInteractionV2=interactionV2.decode;
const identityV2=serviceWire("crm.identity_resolution.v2.IdentityResolutionService");
export const identityV2Path=identityV2.path;
export const binaryIdentityV2=identityV2.binary;
export const decodeIdentityV2=identityV2.decode;
const consentV2=serviceWire("crm.consents.v2.ConsentAuthorizationService");
export const consentV2Path=consentV2.path;
export const binaryConsentV2=consentV2.binary;
export const decodeConsentV2=consentV2.decode;
const salesV2=serviceWire("crm.sales.v2.DealService");
export const salesV2Path=salesV2.path;
export const binarySalesV2=salesV2.binary;
export const decodeSalesV2=salesV2.decode;
const privacyV2=serviceWire("crm.customer_privacy.v2.PrivacyCaseService");
export const privacyV2Path=privacyV2.path;
export const binaryPrivacyV2=privacyV2.binary;
export const decodePrivacyV2=privacyV2.decode;
const enrichmentV2=serviceWire("crm.customer_enrichment.v2.EnrichmentConfigurationService");
export const enrichmentV2Path=enrichmentV2.path;
export const binaryEnrichmentV2=enrichmentV2.binary;
export const decodeEnrichmentV2=enrichmentV2.decode;
const csvV2=serviceWire("crm.customer_data_operations.v2.PartyCsvService");
export const csvV2Path=csvV2.path;
export const binaryCsvV2=csvV2.binary;
export const decodeCsvV2=csvV2.decode;

const overviewService=registry.getService("crm.customer_360.v2.Customer360Service");
if(!overviewService)throw Error("compiled Customer360 v2 service missing");
const overviewMethod=overviewService.methods.find(item=>item.name==="GetAccountOverview");
if(!overviewMethod)throw Error("compiled GetAccountOverview missing");
export const overviewV2Path=()=>"/crm.customer_360.v2.Customer360Service/GetAccountOverview";
export const binaryOverviewV2=(_name,payload)=>toBinary(overviewMethod.input,create(overviewMethod.input,payload));
export const decodeOverviewV2=(_name,bytes)=>fromBinary(overviewMethod.output,bytes);

// Decode the real standard Connect detail from the same compiled descriptor.
export function decodeDomainErrorDetail(detail){
 const schema=registry.getMessage("crm.core.v2.DomainErrorDetail");
 if(!schema||detail.type!==schema.typeName)throw Error("missing generated DomainErrorDetail");
 return fromBinary(schema,Buffer.from(detail.value,"base64"));
}
