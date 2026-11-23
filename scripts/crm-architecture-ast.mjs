// Enforce the *reachable* CRM frontend module graph through the TypeScript AST.
// No regex-only source matching, no legacy barrel import, no alternative browser transport.
// Backend Rust and generated code have their own independent compile/runtime gates.
import ts from "@typescript/typescript6";
import {isActiveProtoFile} from "./crm-rpc-descriptors.mjs";
import {readFileSync,existsSync,readdirSync} from "node:fs";
import {resolve,dirname,extname,relative,sep} from "node:path";
import {fileURLToPath} from "node:url";

const root=resolve(dirname(fileURLToPath(import.meta.url)),"..");
const policy=JSON.parse(readFileSync(resolve(root,"apps/cloudflare-crm/architecture-policy.json"),"utf8"));
const fail=(file,message)=>{throw Error("CRM_AST_VIOLATION "+file+": "+message);};
const forbiddenSource=/(^|\/)(gateway|transport)(\.[cm]?[jt]sx?)?$|\/gen\/crm\/gateway\/|^@connectrpc\/connect-node(?:\/|$)/;
const forbiddenSymbols=new Set(["createGrpcWebTransport","createGrpcTransport","ApplicationGatewayService","GovernedClient","GovernedCrmClient"]);
const directHttp=new Set(["fetch","XMLHttpRequest","WebSocket","EventSource"]);
const forbiddenHttpPath=/^\/api\/(?!health(?:$|\/)|doctor(?:$|\/)|session(?:$|\/)|version(?:$|\/)|customer-data-operations\/parties\.csv$)/;
// Resolve tsconfig inheritance with the compiler before inspecting the module graph.
function compilerConfig(project){
 const path=resolve(root,project);
 const config=ts.getParsedCommandLineOfConfigFile(path,{},{
  ...ts.sys,onUnRecoverableConfigFileDiagnostic: diagnostic=>{
   throw Error(ts.flattenDiagnosticMessageText(diagnostic.messageText,"\n"));
  }
 });
 if(!config||config.errors.length)fail(project,"invalid TypeScript configuration");
 return config;
}
export function assertStrictOptions(project,options){
 for(const key of ["strict","noUncheckedIndexedAccess","exactOptionalPropertyTypes"]){
  if(options[key]!==true)fail(project,"required TypeScript option "+key);
 }
 for(const key of ["noCheck","allowJs","checkJs"]){
  if(options[key])fail(project,"unexpected TypeScript option "+key);
 }
 // TypeScript allows individual strict-family flags to override strict:true.
 for(const key of ["noImplicitAny","noImplicitThis","strictNullChecks","strictFunctionTypes",
  "strictBindCallApply","strictPropertyInitialization","strictBuiltinIteratorReturn","useUnknownInCatchVariables"]){
  if(options[key]===false)fail(project,"disabled TypeScript option "+key);
 }
}
const config=compilerConfig("apps/cloudflare-crm/tsconfig.json");
const allowedBrowserPackages=new Set(["react","react-dom","@ultimate-crm/ui","@ultimate-crm/client"]);
const insideRepo=path=>{
 const rel=relative(root,path);
 return rel && rel!==".." && !rel.startsWith(".."+sep) && !rel.startsWith("/") ? rel.replaceAll("\\","/") : null;
};
// Generated enum re-exports belong to the public client. Resolve their actual
// declarations so new domain enums need no second permission list in CI.
function publicClientValues(){
 const entry=resolve(root,"packages/client/src/cloudflare.ts");
 const program=ts.createProgram([entry],config.options);
 const checker=program.getTypeChecker();
 const client=checker.getSymbolAtLocation(program.getSourceFile(entry));
 const names=new Set(["createCloudflareCrmClient"]);
 for(const symbol of checker.getExportsOfModule(client)){
  if(!(symbol.flags & ts.SymbolFlags.Alias))continue;
  const target=checker.getAliasedSymbol(symbol);
  for(const decl of target.declarations??[]){
   if(!ts.isEnumDeclaration(decl))continue;
   const source=insideRepo(decl.getSourceFile().fileName);
   const prefix="packages/client/gen/";
   if(source?.startsWith(prefix) && source.endsWith("_pb.ts") &&
      isActiveProtoFile(source.slice(prefix.length,-"_pb.ts".length)+".proto")){
    names.add(symbol.name);
   }
  }
 }
 return names;
}
const allowedClientValues=publicClientValues();
const read=path=>readFileSync(resolve(root,path),"utf8");
function collect(directory){
 const files=[];
 function visit(dir){
  if(!existsSync(resolve(root,dir)))return;
  for(const item of readdirSync(resolve(root,dir),{withFileTypes:true})){
   const next=dir+"/"+item.name;
   if(item.isDirectory())visit(next);
   else if(item.isFile()&&/\.(?:ts|tsx|js|jsx|mjs)$/.test(item.name))files.push(next);
  }
 }
 visit(directory);
 return files;
}
function inspect(file,source,strict=true){
 const kind=/\.(?:tsx|jsx)$/.test(file)?ts.ScriptKind.TSX:ts.ScriptKind.TS;
 const tree=ts.createSourceFile(file,source,ts.ScriptTarget.Latest,true,kind);
 if(tree.parseDiagnostics.length)fail(file,"parse error "+tree.parseDiagnostics[0].messageText);
 if(strict && /@ts-(?:ignore|nocheck|expect-error)/.test(source))
  fail(file,"TypeScript diagnostics suppression is forbidden in active CRM");
 const imports=[];
 const inBrowser=file.startsWith("apps/cloudflare-crm/web/");
 function importedName(spec,node){
  if(forbiddenSource.test(spec))fail(file,"legacy transport import "+spec);
  if(spec.startsWith("@connectrpc/")&&file!=="packages/client/src/cloudflare.ts")
   fail(file,"Connect transport belongs only to packages/client/src/cloudflare.ts");
  if(!inBrowser)return;
  if(spec.startsWith("@connectrpc/")||spec.startsWith("@bufbuild/"))
   fail(file,"UI must import generated client, not protocol libraries: "+spec);
  if(spec==="@ultimate-crm/client"){
   fail(file,"legacy client barrel forbidden; import @ultimate-crm/client/cloudflare");
  }
  if(spec.startsWith("@ultimate-crm/client/")&&spec!=="@ultimate-crm/client/cloudflare")
   fail(file,"unapproved client entrypoint "+spec);
  if(spec==="@ultimate-crm/client/cloudflare"){
   if(!ts.isImportDeclaration(node)||!node.importClause||
      !ts.isNamedImports(node.importClause.namedBindings))
    fail(file,"CRM client imports must be explicit named imports");
   const names=node.importClause.namedBindings.elements
    .filter(x=>!node.importClause.isTypeOnly&&!x.isTypeOnly)
    .map(x=>x.propertyName?.text||x.name.text);
   if(names.some(x=>!allowedClientValues.has(x)))
    fail(file,"only the generated Connect CRM client and contract enums are allowed");
  }
 }
 const walk=node=>{
  if(ts.isImportDeclaration(node)||ts.isExportDeclaration(node)){
   if(node.moduleSpecifier&&ts.isStringLiteral(node.moduleSpecifier)){
    const spec=node.moduleSpecifier.text;
    imports.push(spec);
    importedName(spec,node);
   }
  }
  if(ts.isImportEqualsDeclaration(node))fail(file,"CommonJS import alias forbidden");
  if(ts.isCallExpression(node)){
   const e=node.expression;
   if(e.kind===ts.SyntaxKind.ImportKeyword){
    const spec=node.arguments[0];
    if(!spec||!ts.isStringLiteral(spec))
     fail(file,"computed dynamic import forbidden");
    importedName(spec.text,node);
    imports.push(spec.text);
   }
   if(ts.isIdentifier(e)){
    if(["require","eval","Function"].includes(e.text))
     fail(file,"dynamic loading/eval forbidden: "+e.text);
    if(directHttp.has(e.text))fail(file,"direct network call forbidden: "+e.text);
   }
  }
  if(ts.isNewExpression(node)&&ts.isIdentifier(node.expression)
      &&directHttp.has(node.expression.text))
   fail(file,"alternate browser network transport: "+node.expression.text);
  if(ts.isPropertyAccessExpression(node)&&
      directHttp.has(node.name.text)&&
      (inBrowser||node.expression.getText(tree).match(/^(?:window|globalThis|self)$/)))
   fail(file,"indirect HTTP/network call forbidden: "+node.getText(tree));
  if(ts.isElementAccessExpression(node)&&node.argumentExpression&&
      ts.isStringLiteral(node.argumentExpression)&&
      directHttp.has(node.argumentExpression.text))
   fail(file,"computed browser network access forbidden");
  if(ts.isIdentifier(node)&&forbiddenSymbols.has(node.text))
   fail(file,"legacy API symbol "+node.text);
  // Check identifiers as well as call expressions: aliasing fetch or require must
  // not bypass the architecture boundary (e.g. fetch.bind(window)).
  if(inBrowser&&ts.isIdentifier(node)&&directHttp.has(node.text))
   fail(file,"direct browser transport identifier "+node.text);
  if(inBrowser&&ts.isIdentifier(node)&&["eval","Function","require"].includes(node.text))
   fail(file,"untyped browser execution "+node.text);
  if(inBrowser&&ts.isPropertyAccessExpression(node)&&
     node.name.text==="sendBeacon")
   fail(file,"alternate browser transport sendBeacon");
  if(strict&&(node.kind===ts.SyntaxKind.AnyKeyword||
      ts.isAsExpression(node)&&node.type.kind===ts.SyntaxKind.AnyKeyword||
      ts.isTypeAssertionExpression(node)&&node.type.kind===ts.SyntaxKind.AnyKeyword))
   fail(file,"unsafe any is forbidden");
  if(strict&&ts.isAsExpression(node)&&ts.isAsExpression(node.expression)
      &&node.expression.type.kind===ts.SyntaxKind.UnknownKeyword)
   fail(file,"double type assertions cannot bypass the generated contract");
  if(ts.isStringLiteral(node)&&forbiddenHttpPath.test(node.text))
   fail(file,"manual business REST URL "+node.text);
  ts.forEachChild(node,walk);
 };
 walk(tree);
 return imports;
}
const entries=["packages/client/src/cloudflare.ts"];
entries.push("apps/cloudflare-crm/web/src/app.tsx",...collect("apps/cloudflare-crm/web/src"));
function resolveInternal(spec,parent){
 if(/\.(?:css|scss|sass|svg|png|jpe?g|webp|woff2?|ico)(?:\?.*)?$/.test(spec))return null;
 const from=resolve(root,parent);
 const resolved=ts.resolveModuleName(spec,from,config.options,ts.sys).resolvedModule;
 if(!resolved)fail(parent,"unresolved import "+spec);
 const actual=ts.sys.realpath ? ts.sys.realpath(resolved.resolvedFileName) : resolved.resolvedFileName;
 const rel=insideRepo(actual);
 if(parent.startsWith("apps/cloudflare-crm/web/")&&spec.startsWith(".")&&
    !rel?.startsWith("apps/cloudflare-crm/web/src/"))
  fail(parent,"relative import bypasses public workspace exports: "+spec);
 // External declarations are checked by tsc. Runtime package boundaries are
 // explicit; workspace exports are resolved by TypeScript, never a path table.
 const workspace=rel&&!rel.split("/").includes("node_modules");
 if(!spec.startsWith(".")&&parent.startsWith("apps/cloudflare-crm/web/")){
  const packageName=spec.startsWith("@")?spec.split("/").slice(0,2).join("/"):spec.split("/")[0];
  if(!allowedBrowserPackages.has(packageName))
   fail(parent,"unapproved browser dependency "+spec);
 }
 if(!workspace){
  if(spec.startsWith("."))fail(parent,"internal import escapes repository "+spec);
  return null;
 }
 return rel;
}
export function inspectFrontend(){
 for(const [project,parsed] of [
  ["apps/cloudflare-crm",config],
  ["packages/client",compilerConfig("packages/client/tsconfig.json")],
  ["packages/ui",compilerConfig("packages/ui/tsconfig.json")],
 ])assertStrictOptions(project,parsed.options);
 const visited=new Set();
 function follow(file){
  if(visited.has(file))return;
  visited.add(file);
  if(!existsSync(resolve(root,file)))fail(file,"required active source missing");
  if(file.startsWith("apps/cloudflare-crm/web/src/")&&
     !/\.(ts|tsx)$/.test(file))
    fail(file,"legacy JS/JSX reachable after cutover");
  // Generated protobuf source is fully checked by the pinned generation/typecheck.
  if(file.startsWith("packages/client/gen/"))return;
  const source=read(file);
  for(const imp of inspect(file,source,true)){
   const next=resolveInternal(imp,file);
   if(next)follow(next);
  }
 }
 entries.forEach(follow);
 return [...visited].sort();
}
if(import.meta.main){
 console.log(JSON.stringify({event:"crm.ts-ast-architecture",result:"PASS",
  policy_schema:policy.schema_version,inspected:inspectFrontend()}));
}
export {inspect,resolveInternal};
