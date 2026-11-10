// TypeScript Compiler API, not regex: inspect actual source nodes and import edges.
// Used before every CRM Worker build, even while old JSX stays frozen and unused.
import ts from "typescript";
import {readFileSync,existsSync,readdirSync} from "node:fs";
import {resolve,dirname,extname} from "node:path";
import {fileURLToPath} from "node:url";
const root=resolve(dirname(fileURLToPath(import.meta.url)),"..");
const rd=p=>readFileSync(resolve(root,p),"utf8");
const policy=JSON.parse(rd("apps/cloudflare-crm/architecture-policy.json"));
const forbiddenSource=/^(?:.*\/)?(?:transport|gateway)(?:\.[cm]?[jt]sx?)?$|\/gen\/crm\/gateway\/|^@connectrpc\/connect-node$/;
const legacySymbols=new Set(["createGrpcWebTransport","ApplicationGatewayService","GovernedClient","createGrpcTransport"]);
const badHttp=/^\/api\/(?!health(?:$|\/)|doctor(?:$|\/)|session(?:$|\/)|version(?:$|\/))/;
const fail=(file,msg)=>{throw Error("CRM_AST_VIOLATION "+file+": "+msg);};
const tsExt=/\.(ts|tsx|js|jsx|mjs)$/;
function list(dir){
  const result=[];
  function visit(p){if(!existsSync(resolve(root,p)))return;for(const e of readdirSync(resolve(root,p),{withFileTypes:true})){const f=p+"/"+e.name;if(e.isDirectory())visit(f);else if(e.isFile()&&tsExt.test(f))result.push(f);}}
  visit(dir);return result;
}
function inspect(file,source,strict=true){
 const kind=file.endsWith(".tsx")||file.endsWith(".jsx")?ts.ScriptKind.TSX:ts.ScriptKind.TS;
 const tree=ts.createSourceFile(file,source,ts.ScriptTarget.Latest,true,kind);
 if(tree.parseDiagnostics.length)fail(file,"parse error "+tree.parseDiagnostics[0].messageText);
 const imports=[];
 const walk=node=>{
   if(ts.isImportDeclaration(node)||ts.isExportDeclaration(node)){
     if(node.moduleSpecifier&&ts.isStringLiteral(node.moduleSpecifier)){
       const name=node.moduleSpecifier.text;imports.push(name);
       if(forbiddenSource.test(name))fail(file,"legacy gateway import "+name);
       if(file.includes("/web/") && name.startsWith("@connectrpc/"))
         fail(file,"web must import the generated client package, not raw transports");
     }
   }
   if(ts.isImportEqualsDeclaration(node))fail(file,"CommonJS import alias not allowed");
   if(ts.isCallExpression(node)){
     const e=node.expression;
     if(e.kind===ts.SyntaxKind.ImportKeyword){
       if(!node.arguments.length||!ts.isStringLiteral(node.arguments[0]))
         fail(file,"dynamic imports must use a literal specifier");
       if(forbiddenSource.test(node.arguments[0].text))fail(file,"legacy dynamic import");
     }
     if(ts.isIdentifier(e)){
       if(e.text==="require"||e.text==="eval"||e.text==="Function")
         fail(file,"untyped dynamic loading not allowed: "+e.text);
       if(e.text==="fetch"||e.text==="XMLHttpRequest")
         fail(file,"direct HTTP bypass not allowed: "+e.text);
       if(legacySymbols.has(e.text))fail(file,"legacy transport call "+e.text);
     }
     if(ts.isPropertyAccessExpression(e)&&["fetch","request"].includes(e.name.text)
        && file.includes("/web/"))fail(file,"indirect browser HTTP request not allowed");
   }
   if(ts.isNewExpression(node)&&ts.isIdentifier(node.expression)
     &&["XMLHttpRequest","WebSocket","EventSource"].includes(node.expression.text))
     fail(file,"alternate transport not allowed "+node.expression.text);
   if(ts.isIdentifier(node)&&legacySymbols.has(node.text))
     fail(file,"legacy transport identifier "+node.text);
   if(strict&&(node.kind===ts.SyntaxKind.AnyKeyword||ts.isAsExpression(node)&&
       node.type.kind===ts.SyntaxKind.AnyKeyword))
     fail(file,"unsafe any is not allowed in active typed CRM");
   if(strict&&ts.isTypeAssertionExpression(node)&&node.type.kind===ts.SyntaxKind.AnyKeyword)
     fail(file,"unsafe type assertion");
   if(ts.isStringLiteral(node)&&badHttp.test(node.text))
     fail(file,"manual business REST path "+node.text);
   ts.forEachChild(node,walk);
 };
 walk(tree);
 return imports;
}
const target=["packages/client/src/cloudflare.ts"];
if(policy.phase==="contract-only")target.push("apps/cloudflare-crm/web/contract-only.tsx");
if(policy.phase==="cutover")target.push(...list("apps/cloudflare-crm/web/src"));
const visited=new Set();
const follow=file=>{
 if(visited.has(file))return;visited.add(file);
 if(!existsSync(resolve(root,file)))fail(file,"required active source missing");
 for(const imp of inspect(file,rd(file))){
   if(!imp.startsWith("."))continue;
   const dir=dirname(resolve(root,file));const abs=resolve(dir,imp);
   const candidate=[abs,abs+".ts",abs+".tsx",abs+".js",abs+".jsx",abs+"/index.ts",abs+"/index.tsx"];
   const next=candidate.find(existsSync);
   if(!next)fail(file,"unresolved relative import "+imp);
   const normalized=next.slice(root.length+1).replaceAll("\\","/");
   if(/\.(jsx|js|mjs)$/.test(normalized)&&policy.phase!=="migration-freeze")
     fail(file,"active code imports legacy JavaScript "+normalized);
   if(normalized.startsWith("apps/cloudflare-crm/web/")||normalized==="packages/client/src/cloudflare.ts")
     follow(normalized);
 }
};
target.forEach(follow);
console.log(JSON.stringify({event:"crm.ts-ast-architecture",result:"PASS",phase:policy.phase,inspected:[...visited]}));
export {inspect};
