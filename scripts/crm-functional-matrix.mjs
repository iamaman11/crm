// Generated evidence for G0: inspect the real React AST, Buf descriptor and
// canonical module report. Never maintain a second handwritten capability list.
import ts from "typescript";
import {readFileSync,readdirSync,writeFileSync} from "node:fs";
import {execFileSync} from "node:child_process";
import {resolve,dirname,relative} from "node:path";
import {fileURLToPath} from "node:url";
import {createRequire} from "node:module";

const root=resolve(dirname(fileURLToPath(import.meta.url)),"..");
const requireClient=createRequire(resolve(root,"packages/client/package.json"));
const {fromBinary}=requireClient("@bufbuild/protobuf");
const {FileDescriptorSetSchema}=requireClient("@bufbuild/protobuf/wkt");
const webRoot=resolve(root,"apps/cloudflare-crm/web/src");
const fileText=file=>readFileSync(resolve(root,file),"utf8");
const fail=message=>{throw Error("CRM_FUNCTIONAL_MATRIX_VIOLATION: "+message);};

function walkFiles(dir){
 const result=[];
 for(const entry of readdirSync(dir,{withFileTypes:true})){
  const path=resolve(dir,entry.name);
  if(entry.isDirectory())result.push(...walkFiles(path));
  else if(entry.isFile()&&/\.(?:jsx|tsx|js|ts|mjs)$/.test(entry.name))
   result.push(path);
 }
 return result.sort();
}

function position(source,node){
 const p=source.getLineAndCharacterOfPosition(node.getStart(source));
 return {line:p.line+1,column:p.character+1};
}
function literal(node){
 if(!node)return null;
 if(ts.isStringLiteral(node)||ts.isNoSubstitutionTemplateLiteral(node))return node.text;
 return null;
}
function staticPrefix(node){
 const value=literal(node);
 if(value!==null)return value;
 if(ts.isBinaryExpression(node)&&node.operatorToken.kind===ts.SyntaxKind.PlusToken)
  return staticPrefix(node.left);
 return null;
}
function methodVariants(node){
 if(!node)return ["GET"];
 if(ts.isParenthesizedExpression(node))return methodVariants(node.expression);
 const value=literal(node);
 if(value!==null)return [value.toUpperCase()];
 if(ts.isConditionalExpression(node))
  return [...new Set([...methodVariants(node.whenTrue),...methodVariants(node.whenFalse)])].sort();
 return ["DYNAMIC"];
}
function callMethods(options){
 if(!options)return ["GET"];
 if(!ts.isObjectLiteralExpression(options))return ["DYNAMIC"];
 const prop=options.properties.find(x=>ts.isPropertyAssignment(x)
  &&(x.name.getText()==="method"||x.name.getText()==='"method"'));
 return prop?methodVariants(prop.initializer):["GET"];
}

export function inspectUiFile(sourceText,file){
 const source=ts.createSourceFile(file,sourceText,ts.ScriptTarget.Latest,true,
  /\.(?:jsx|tsx)$/.test(file)?ts.ScriptKind.TSX:ts.ScriptKind.TS);
 if(source.parseDiagnostics.length)
  fail(file+" syntax: "+source.parseDiagnostics[0].messageText);
 const controls=[],requests=[];
 function visit(node){
  if(ts.isJsxOpeningElement(node)||ts.isJsxSelfClosingElement(node)){
   const tag=node.tagName.getText(source);
   if(["button","form","input","select","textarea","a"].includes(tag)){
    const attributes=node.attributes.properties;
    const handlers=attributes.filter(x=>ts.isJsxAttribute(x)
     &&/^on(?:Click|Submit|Change|KeyDown)$/.test(x.name.text))
     .map(x=>x.name.text);
    if(tag==="button"||tag==="form"||handlers.length){
     const label=ts.isJsxOpeningElement(node)&&ts.isJsxElement(node.parent)
      ?node.parent.children.filter(ts.isJsxText).map(x=>x.getText(source).trim())
        .filter(Boolean).join(" ").slice(0,90):"";
     controls.push({file,...position(source,node),element:tag,handlers,label});
    }
   }
  }
  if(ts.isCallExpression(node)&&ts.isIdentifier(node.expression)
   &&["load","api"].includes(node.expression.text)){
   if(!node.arguments.length)fail(file+" has an API call without a path");
   const path=node.arguments[0];
   requests.push({file,...position(source,node),caller:node.expression.text,
    path_expression:path.getText(source),static_path:literal(path),
    static_prefix:staticPrefix(path),methods:callMethods(node.arguments[1]),
    transport:"legacy-rest"});
  }
  ts.forEachChild(node,visit);
 }
 visit(source);
 return {file,controls,requests};
}

function descriptorMethods(path){
 const set=fromBinary(FileDescriptorSetSchema,readFileSync(path));
 const methods=[];
 for(const file of set.file)for(const service of file.service)
  for(const method of service.method)
   methods.push({path:"/"+file.package+"."+service.name+"/"+method.name,
    input:(method.inputType||"").replace(/^\./,""),
    output:(method.outputType||"").replace(/^\./,""),file:file.name});
 return methods.sort((a,b)=>a.path.localeCompare(b.path));
}

export function deriveMatrix({files,modules,descriptors,phase,sha}){
 const actual=new Map(descriptors.map(x=>[x.path,x]));
 if(actual.size!==descriptors.length)fail("duplicate compiled Protobuf methods");
 const bindings=new Map();
 for(const mod of modules.modules){
  for(const rpc of mod.rpc_bindings||[]){
   if(bindings.has(rpc.path))fail("ambiguous canonical RPC owner "+rpc.path);
   bindings.set(rpc.path,{...rpc,module_id:mod.module_id,linked:mod.linked});
  }
 }
 const declared=[...bindings].sort(([a],[b])=>a.localeCompare(b))
  .map(([path,binding])=>({path,module_id:binding.module_id,
   linked:binding.linked,request:binding.request,response:binding.response,
   descriptor_present:actual.has(path),
   active:modules.active_rpc_paths.includes(path)}));
 const unmatched=declared.filter(x=>!x.descriptor_present);
 const active=modules.active_rpc_paths.map(path=>{
  const binding=bindings.get(path),compiled=actual.get(path);
  if(!binding||!binding.linked||!compiled||
     compiled.input!==binding.request||compiled.output!==binding.response)
   fail("active RPC not backed by a linked owner and matching compiled types "+path);
  return {...compiled,module_id:binding.module_id};
 });
 // A UI interaction is NOT credited with a working RPC just because its
 // enclosing file mentions a service. Only a traced browser call may prove it.
 const controls=files.flatMap(x=>x.controls);
 const requests=files.flatMap(x=>x.requests);
 const dynamic=requests.filter(x=>x.static_path===null);
 return {schema_version:1,source_sha:sha,phase,
  evidence:"React TypeScript AST + canonical module.yaml/Cargo report + compiled Buf FileDescriptorSet",
  ui:{files:files.map(x=>({file:x.file,controls:x.controls.length,
   legacy_requests:x.requests.length})),controls,legacy_requests:requests,
   dynamic_legacy_requests:dynamic,verified_button_to_rpc_links:[],
   note:"Not linked: UI click -> RPC -> Rust owner -> D1 must be established by executable acceptance tests"},
  protobuf:{compiled_method_count:descriptors.length,canonical_bindings:declared,
   bindings_missing_descriptors:unmatched,active_methods:active},
  summary:{ui_controls:controls.length,legacy_ui_calls:requests.length,
   dynamic_legacy_calls:dynamic.length,declared_rpc_bindings:declared.length,
   active_rpc_methods:active.length,verified_button_to_rpc_links:0}};
}
export function run(){
 const outIndex=process.argv.indexOf("--output");
 if(outIndex<0||!process.argv[outIndex+1])fail("--output path is required");
 const descriptor=process.env.CRM_PROTO_DESCRIPTOR_FILE;
 const source=process.env.CRM_MODULE_COVERAGE_REPORT;
 if(!descriptor||!source)fail("compiled Buf descriptor and canonical module report required");
 const modules=JSON.parse(readFileSync(source,"utf8"));
 const policy=JSON.parse(fileText("apps/cloudflare-crm/architecture-policy.json"));
 if(policy.phase!==modules.phase)fail("module report phase mismatch");
 const sha=execFileSync("git",["rev-parse","HEAD"],{cwd:root,encoding:"utf8"}).trim();
 if(modules.source_sha!==sha)fail("stale canonical module report");
 const files=walkFiles(webRoot).map(path=>inspectUiFile(readFileSync(path,"utf8"),
  relative(root,path).replaceAll("\\","/")));
 const report=deriveMatrix({files,modules,descriptors:descriptorMethods(descriptor),
  phase:policy.phase,sha});
 if(policy.phase==="cutover"&&report.summary.legacy_ui_calls)
  fail("final typed UI still contains legacy REST calls");
 writeFileSync(process.argv[outIndex+1],JSON.stringify(report,null,2)+"\n");
 console.log(JSON.stringify({event:"crm.functional-matrix",result:"PASS",
  ...report.summary,report:process.argv[outIndex+1]}));
}
if(process.argv[1]&&resolve(process.argv[1])===fileURLToPath(import.meta.url))run();
