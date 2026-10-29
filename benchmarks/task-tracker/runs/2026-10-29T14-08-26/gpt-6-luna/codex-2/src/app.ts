import { accounts } from "./accounts";
import type { Ctx } from "./context";
import { auth } from "./context";
import { errorResponse, fail, json, object } from "./http";
import { store } from "./model";
import { projects } from "./projects";
import { taskRoutes } from "./taskdata";
export type Options={now?:()=>number};
export type App={fetch(request:Request):Response|Promise<Response>};
const methods:Record<string,string>={"/users":"POST","/sessions":"POST, DELETE","/me":"GET"};
function allowed(path:string):string|undefined {
  if(methods[path])return methods[path];
  if(path==="/projects")return "GET, POST";
  if(/^\/projects\/[^/]+$/.test(path))return "GET";
  if(/^\/projects\/\d+\/members$/.test(path))return "GET, POST";
  if(/^\/projects\/\d+\/members\/\d+$/.test(path))return "DELETE";
  if(/^\/projects\/\d+\/(tasks|stats)$/.test(path))return "GET, POST";
  if(/^\/projects\/\d+\/tasks\/bulk$/.test(path))return "POST";
  if(/^\/tasks\/[^/]+$/.test(path))return "GET, PATCH, DELETE";
  if(/^\/tasks\/[^/]+\/(history|dependencies|comments)$/.test(path))return "GET, POST";
  if(/^\/tasks\/[^/]+\/transitions$/.test(path))return "POST";
  if(/^\/tasks\/[^/]+\/dependencies\/[^/]+$/.test(path))return "DELETE";
}
export function createApp(options:Options={}):App {
  const s=store(),now=options.now||Date.now;
  return {async fetch(request:Request):Promise<Response>{
    const url=new URL(request.url),path=url.pathname,method=request.method.toUpperCase();
    const allow=allowed(path);
    if(!allow)return json({error:{code:"not_found",message:"Not found"}},404);
    if(!allow.split(", ").includes(method)){
      return json({error:{code:"method_not_allowed",message:"Method not allowed"}},405,
        {Allow:allow});
    }
    const ctx:Ctx={s,now,request,url};
    let response:Response;
    try{
      let body:any={};
      if(["POST","PATCH","PUT"].includes(method)){
        const raw=await request.text();
        if(raw){
          try{body=object(JSON.parse(raw));}
          catch(e){
            if(e instanceof SyntaxError)fail(400,"validation","Invalid JSON");
            throw e;
          }
        }
      }
      response=accounts(ctx,method,path,body)||projects(ctx,method,path,body)||taskRoutes(ctx,method,path,body)||
        json({error:{code:"not_found",message:"Not found"}},404);
    }catch(e){response=errorResponse(e);}
    const header=request.headers.get("Authorization")||"";
    const token=header.startsWith("Bearer ")?header.slice(7):"";
    const user=s.sessions.get(token);
    if(user===undefined)return response;
    const minute=Math.floor(now()/60000), key=token;
    let rate=s.rates.get(key);
    if(!rate||rate.minute!==minute)rate={minute,count:0};
    rate.count++;s.rates.set(key,rate);
    const headers=new Headers(response.headers);
    headers.set("X-RateLimit-Limit","100");
    headers.set("X-RateLimit-Remaining",String(Math.max(0,100-rate.count)));
    if(rate.count>100){
      const retry=Math.ceil(((minute+1)*60000-now())/1000);
      headers.set("Retry-After",String(retry));
      return Response.json({error:{code:"rate_limited",message:"Rate limit exceeded"}},
        {status:429,headers});
    }
    return new Response(response.body,{status:response.status,statusText:response.statusText,headers});
  }};
}
