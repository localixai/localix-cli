import { error, type Options, type State } from "./model";
import { users } from "./users";
import { projects } from "./projects";
import { tasks } from "./tasks";
export type App = { fetch(request: Request): Response | Promise<Response> };
export function createApp(options: Options = {}): App {
  const s:State={
    now:options.now||Date.now,users:[],projects:[],tasks:[],sessions:new Map(),
    limits:new Map(),ids:{user:0,project:0,task:0,comment:0},
    idem:new Map(),sequences:new Map(),
  };
  return {async fetch(req:Request) {
    const url=new URL(req.url), method=req.method, path=url.pathname;
    const token=(req.headers.get("authorization")||"")
      .match(/^Bearer (.+)$/)?.[1]||null;
    let body:any=null;
    if(req.body)try {
      body=await req.json();
      if(!body||typeof body!=="object"||Array.isArray(body))throw 0;
    } catch { return error("validation","Invalid JSON",400); }
    const noAuth=["/users","/sessions"].includes(path);
    const known=path==="/users"||path==="/sessions"||path==="/me"||
      path==="/projects"||/^\/projects\/\d+/.test(path)||/^\/tasks\//.test(path);
    if(!known)return error("not_found","Not found",404);
    const allow:Record<string,string>={"/users":"POST","/sessions":"POST, DELETE","/me":"GET","/projects":"GET, POST"};
    if(allow[path]&&!allow[path].split(", ").includes(method))
      return new Response(JSON.stringify({error:{code:"method_not_allowed",message:"Method not allowed"}}),{
        status:405,headers:{"Content-Type":"application/json","Allow":allow[path]},
      });
    const uid=token?s.sessions.get(token):undefined;
    if(!noAuth&&!uid)return error("unauthorized","Unauthorized",401);
    if(token&&uid){
      const l=s.limits.get(token),start=Math.floor(s.now()/60000)*60000;
      const cur=!l||l.start!==start?{start,count:0}:l;
      cur.count++;s.limits.set(token,cur);
      const h={"X-RateLimit-Limit":"100","X-RateLimit-Remaining":String(Math.max(0,100-cur.count))};
      if(cur.count>100)return Response.json({error:{code:"rate_limited",message:"Rate limited"}},{
        status:429,headers:{...h,"Retry-After":String(Math.ceil((start+60000-s.now())/1000))},
      });
    }
    const response=users(s,method,path,body,token)||
      projects(s,method,path,body,uid||0)||
      tasks(s,method,url.pathname+url.search,body,uid||0,req.headers)||
      error("not_found","Not found",404);
    if(uid&&path==="/me") {
      response.headers.set("X-RateLimit-Limit","100");
      response.headers.set("X-RateLimit-Remaining",String(Math.max(0,100-(s.limits.get(token!)?.count||1))));
    }
    return response;
  }};
}
