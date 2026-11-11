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
