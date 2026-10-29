import type { App, Options } from "./app-types";
import { err } from "./http";
import { projects } from "./projects";
import { stats } from "./stats";
import { tasks } from "./tasks";
import { users } from "./users";
import type { Ctx, State } from "./model";
import { auth, body, type Result } from "./http";

export type { App, Options } from "./app-types";

const methods: Record<string,string[]> = {
  "/users":["POST"],"/sessions":["POST","DELETE"],"/me":["GET"],
  "/projects":["GET","POST"],
};
function allowed(path:string,method:string) {
  if(path.startsWith("/projects/")) {
    if(/\/tasks(?:\/bulk)?$/.test(path))return ["GET","POST"];
    if(/\/members(?:\/\d+)?$/.test(path))return ["GET","POST","DELETE"];
    if(/\/stats$/.test(path))return ["GET"];
    return ["GET"];
  }
  if(path.startsWith("/tasks/")) {
    if(/\/comments$/.test(path))return ["GET","POST"];
    if(/\/dependencies(?:\/.+)?$/.test(path))return ["GET","POST","DELETE"];
    if(/\/transitions$/.test(path))return ["POST"];
    if(/\/history$/.test(path))return ["GET"];
    return ["GET","PATCH","DELETE"];
  }
  return methods[path];
}
export function createApp(options:Options={}):App {
  const state:State={users:[],projects:[],tasks:[],tokens:new Map(),members:new Map(),
    comments:new Map(),history:new Map(),deps:new Map(),idem:new Map(),hits:new Map(),
    sequences:new Map(),ids:{user:0,project:0,task:0,comment:0}};
  const base:Ctx={state,now:options.now??Date.now};
  return {async fetch(req:Request) {
    const url=new URL(req.url), path=url.pathname, method=req.method.toUpperCase();
    const accepts=allowed(path,method);
    if(!accepts)return response(err(404,"not_found","Not found"));
    if(!accepts.includes(method)) {
      const r=response(err(405,"method_not_allowed","Method not allowed"));
      r.headers.set("Allow",accepts.join(", "));return r;
    }
    const token=(req.headers.get("Authorization")??"").startsWith("Bearer ")
      ? req.headers.get("Authorization")!.slice(7) : "";
    let uid=auth(base,req);
    const needsAuth=!(path==="/users"&&method==="POST")&&!(path==="/sessions"&&method==="POST");
    if(needsAuth&&!uid)return response(err(401,"unauthorized","Unauthorized"));
    const rateHeaders:Record<string,string>={};
    if(uid&&token) {
      const now=base.now(), prior=state.hits.get(token);
      const start=Math.floor(now/60000)*60000;
      if(!prior||now-prior.start>=60000)state.hits.set(token,{start,count:0});
      const hit=state.hits.get(token)!;
      if(hit.count>=100) {
        const out=response(err(429,"rate_limited","Rate limited"));
        out.headers.set("Retry-After",String(Math.ceil((hit.start+60000-now)/1000)));
        out.headers.set("X-RateLimit-Limit","100");out.headers.set("X-RateLimit-Remaining","0");
        return out;
      }
      hit.count++;rateHeaders["X-RateLimit-Limit"]="100";
      rateHeaders["X-RateLimit-Remaining"]=String(100-hit.count);
    }
    const c={...base,req,userId:uid??0,url};
    const result:Result=(await users(method,path,c))??(await projects(method,path,c))??
      stats(path,c)??(await tasks(method,path,c))??err(404,"not_found","Not found");
    return response(result,rateHeaders);
  }};
}
function response(r:Result, extra:Record<string,string>={}) {
  const headers=new Headers(extra);
  for(const [k,v] of Object.entries(r.headers??{}))headers.set(k,v);
  if(r.status===204)return new Response(null,{status:204,headers});
  return Response.json(r.body??null,{status:r.status,headers});
}
