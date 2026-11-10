// Enforce the *reachable* CRM frontend module graph through the TypeScript AST.
// No regex-only source matching, no legacy barrel import, no alternative browser transport.
// Backend Rust and generated code have their own independent compile/runtime gates.
import ts from "typescript";
import {readFileSync,existsSync,readdirSync} from "node:fs";
import {resolve,dirname,extname,relative,sep} from "node:path";
import {fileURLToPath} from "node:url";

const root=resolve(dirname(fileURLToPath(import.meta.url)),"..");
const policy=JSON.parse(readFileSync(resolve(root,"apps/cloudflare-crm/architecture-policy.json"),"utf8"));
const fail=(file,message)=>{throw Error("CRM_AST_VIOLATION "+file+": "+message);};
const forbiddenSource=/(^|\/)(gateway|transport)(\.[cm]?[jt]sx?)?$|\/gen\/crm\/gateway\/|^@connectrpc\/connect-node(?:\/|$)/;
const forbiddenSymbols=new Set(["createGrpcWebTransport","createGrpcTransport","ApplicationGatewayService","GovernedClient","GovernedCrmClient"]);
const directHttp=new Set(["fetch","XMLHttpRequest","WebSocket","EventSource"]);
const forbiddenHttpPath=/^\/api\/(?!health(?:$|\/)|doctor(?:$|\/)|session(?:$|\/)|version(?:$|\/))/;
const exts=["",".ts",".tsx",".js",".jsx",".mjs","/index.ts","/index.tsx","/index.js"];
const insideRepo=path=>{
 const rel=relative(root,path);
 return rel && rel!==".." && !rel.startsWith(".."+sep) && !rel.startsWith("/") ? rel.replaceAll("\\","/") : null;
};
const read=path=>readFileSync(resolve(root,path),"utf8");
const activeTyped=policy.phase==="cutover";
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
   const names=node.importClause.namedBindings.elements.map(x=>x.propertyName?.text||x.name.text);
   if(!names.length||names.some(x=>x!=="createCloudflareCrmClient"))
    fail(file,"only the generated Connect CRM client is allowed");
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
if(policy.phase==="contract-only")entries.push("apps/cloudflare-crm/web/contract-only.tsx");
if(activeTyped)entries.push(...collect("apps/cloudflare-crm/web/src"));
const visited=new Set();
function resolveInternal(spec,parent){
 if(/\.(?:css|scss|sass|svg|png|jpe?g|webp|woff2?|ico)(?:\?.*)?$/.test(spec))return null;
 if(spec==="@ultimate-crm/client/cloudflare")return "packages/client/src/cloudflare.ts";
 if(spec==="@ultimate-crm/ui")return "packages/ui/src/index.ts";
 if(!spec.startsWith(".")){
   if(parent.startsWith("apps/cloudflare-crm/web/") &&
      spec.startsWith("@ultimate-crm/"))fail(parent,"unapproved workspace import "+spec);
   return null;
 }
 const from=resolve(root,parent);
 const settings={module:ts.ModuleKind.ESNext,moduleResolution:ts.ModuleResolutionKind.Bundler,
                 allowJs:true,resolveJsonModule:true};
 const resolved=ts.resolveModuleName(spec,from,settings,ts.sys).resolvedModule?.resolvedFileName;
 const abs=resolve(root,dirname(parent),spec);
 const result=resolved || exts.map(s=>abs+s).find(x=>existsSync(x));
 if(!result)fail(parent,"unresolved internal import "+spec);
 const rel=insideRepo(result);
 if(!rel)fail(parent,"import escapes repository "+spec);
 return rel;
}
function follow(file){
 if(visited.has(file))return;
 visited.add(file);
 if(!existsSync(resolve(root,file)))fail(file,"required active source missing");
 if(activeTyped&&file.startsWith("apps/cloudflare-crm/web/src/")&&
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
console.log(JSON.stringify({event:"crm.ts-ast-architecture",result:"PASS",
 phase:policy.phase,inspected:[...visited].sort()}));
export {inspect};
