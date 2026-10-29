import { Fault, errorResponse, fail, readBody, response } from "./http";
import { createState, memberRole, pair, type Project, type State } from "./model";
import { cleanTask, createTask, historyChange, memberCanRead, taskFor } from "./tasks";
import { taskItemRoutes } from "./task-items";

export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };
type Context = { s: State; now: () => number; req: Request; url: URL; user?: number };
const stamp = (c: Context) => new Date(c.now()).toISOString();
const projectOf = (c: Context, id: string) => {
  const p = c.s.projects.find(x => x.id === Number(id));
  if (!p || !memberCanRead(c.s, p, c.user!)) fail(404, "not_found", "not found");
  return p;
};
const auth = (c: Context) => {
  const token = /^Bearer (.+)$/.exec(c.req.headers.get("Authorization") ?? "")?.[1];
  c.user = token ? c.s.sessions.get(token) : undefined;
  if (!c.user) fail(401, "unauthorized", "unauthorized");
  if (token) {
    const minute = Math.floor(c.now() / 60000), item = c.s.limits.get(token);
    if (!item || item.minute !== minute) c.s.limits.set(token, { minute, count: 0 });
    const current = c.s.limits.get(token)!;
    if (current.count >= 100) {
      const retry = Math.ceil(((minute + 1) * 60000 - c.now()) / 1000);
      throw Object.assign(new Fault(429, "rate_limited", "rate limited"), { retry });
    }
    current.count++;
  }
};
const ownerOrMember = (c: Context, p: Project) => {
  const role = memberRole(c.s, p, c.user!);
  if (role === "viewer") fail(403, "forbidden", "forbidden");
  if (!role) fail(404, "not_found", "not found");
};
const routeMethods: Record<string, string> = {
  "POST /users":"POST", "POST /sessions":"POST", "GET /me":"GET", "DELETE /sessions":"DELETE",
    "GET /projects":"GET, POST", "POST /projects":"GET, POST", "PUT /users":"POST", "PUT /me":"GET",
};
function userRoutes(c: Context, path: string, method: string, body: any) {
  if (path === "/users" && method === "POST") {
    const email = typeof body.email === "string" ? body.email.trim().toLowerCase() : "";
    const name = typeof body.name === "string" ? body.name.trim() : "";
    const password = body.password;
    if (!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(email)) fail(400,"validation","invalid email","email");
    if (typeof password !== "string" || password.length < 8) fail(400,"validation","invalid password","password");
    if (!name || name.length > 100) fail(400,"validation","invalid name","name");
    if (c.s.users.some(u=>u.email===email)) fail(409,"conflict","email already exists","email");
    const u={id:c.s.users.length+1,email,name,password}; c.s.users.push(u);
    return response({id:u.id,email,name},201);
  }
  if (path === "/sessions" && method === "POST") {
    const u=c.s.users.find(x=>x.email===String(body.email).toLowerCase()&&x.password===body.password);
    if (!u) fail(401,"unauthorized","unauthorized");
    const token=`session-${crypto.randomUUID()}`; c.s.sessions.set(token,u!.id); return response({token},201);
  }
  if (path === "/sessions" && method === "DELETE") {
    const token=/^Bearer (.+)$/.exec(c.req.headers.get("Authorization")??"")?.[1];
    if (!token||!c.s.sessions.has(token)) fail(401,"unauthorized","unauthorized");
    c.s.sessions.delete(token!); return response(null,204);
  }
  if (path === "/me" && method === "GET") {
    auth(c); const u=c.s.users.find(x=>x.id===c.user)!; return response({id:u.id,email:u.email,name:u.name});
  }
}
function projectRoutes(c: Context, path: string, method: string, body: any) {
  if(path==="/projects"&&method==="POST"){
    auth(c); const name=typeof body.name==="string"?body.name.trim():"";
    if(!name||name.length>100)fail(400,"validation","invalid name","name");
    if(typeof body.key!=="string"||!/^[A-Z]{2,6}$/.test(body.key))fail(400,"validation","invalid key","key");
    if(c.s.projects.some(p=>p.key===body.key))fail(409,"conflict","key already exists","key");
    const p={id:c.s.projects.length+1,name,key:body.key,ownerId:c.user!,createdAt:stamp(c)};
    c.s.projects.push(p); return response(p,201);
  }
  if(path==="/projects"&&method==="GET"){
    auth(c); return response(c.s.projects.filter(p=>memberCanRead(c.s,p,c.user!)));
  }
  const match=/^\/projects\/([^/]+)(?:\/(members|tasks|stats)(?:\/bulk|\/\d+)?)?$/.exec(path);
  if(!match)return;
  auth(c); const [,id,resource]=match;
  if(!resource&&method==="GET")return response(projectOf(c,id!));
  const p=c.s.projects.find(x=>x.id===Number(id));
  if(!p)fail(404,"not_found","not found");
  if(resource==="members")return members(c,p!,method,path,body);
  if(resource==="tasks")return taskRoutes(c,p!,method,path,body);
  if(resource==="stats"&&method==="GET"){projectOf(c,id!);return stats(c,p!);}
}
function members(c:Context,p:Project,method:string,path:string,body:any){
  if(method==="GET"&&path.endsWith("/members")){
    projectOf(c,String(p.id)); return response([{userId:p.ownerId,role:"owner"},...Array.from(c.s.members)
      .filter(([k])=>k.startsWith(`${p.id}:`)).map(([k,role])=>({userId:Number(k.split(":")[1]),role}))]);
  }
  if(method==="POST"&&path.endsWith("/members")){
    auth(c); if(c.user!==p.ownerId)fail(403,"forbidden","forbidden");
    if(!["member","viewer"].includes(body.role))fail(400,"validation","invalid role","role");
    if(!c.s.users.some(u=>u.id===body.userId))fail(400,"validation","invalid userId","userId");
    const k=pair(p.id,body.userId); if(body.userId===p.ownerId||c.s.members.has(k))fail(409,"conflict","already a member");
    c.s.members.set(k,body.role); return response({userId:body.userId,role:body.role},201);
  }
  const m=/\/members\/(\d+)$/.exec(path);
  if(method==="DELETE"&&m){if(c.user!==p.ownerId)fail(403,"forbidden","forbidden");
    const uid=Number(m[1]),k=pair(p.id,uid); if(!c.s.members.delete(k))fail(404,"not_found","not found");
    c.s.tasks.filter(t=>t.projectId===p.id&&t.assigneeId===uid).forEach(t=>t.assigneeId=null);
    return response(null,204);}
}
function stats(c:Context,p:Project){
  const tasks=c.s.tasks.filter(t=>t.projectId===p.id);
  const buckets=(field:string,values:string[])=>Object.fromEntries(values.map(v=>[v,tasks.filter(t=>(t as any)[field]===v).length]));
  const counts=new Map<number,number>(); tasks.forEach(t=>{if(t.assigneeId!==null)counts.set(t.assigneeId,(counts.get(t.assigneeId)||0)+1);});
  return response({total:tasks.length,byStatus:buckets("status",["open","in_progress","done"]),
    byPriority:buckets("priority",["high","normal","low"]),byAssignee:[...counts].map(([userId,count])=>({userId,count}))
      .sort((a,b)=>b.count-a.count||a.userId-b.userId)});
}
function taskRoutes(c:Context,p:Project,method:string,path:string,body:any){
  auth(c); ownerOrMember(c,p);
  if(method==="POST"&&(path.endsWith("/tasks")||path.endsWith("/tasks/bulk"))&&
    !path.endsWith("/bulk"))return response(createTask(c.s,p,cleanTask(body,c.user!,p,c.s),stamp(c)),201);
  if(method==="POST"&&path.endsWith("/tasks/bulk")){
    if(!Array.isArray(body.tasks)||body.tasks.length<1||body.tasks.length>50)fail(400,"validation","invalid tasks","tasks");
    const prepared=body.tasks.map((x:any,i:number)=>{try{return cleanTask(x,c.user!,p,c.s)}catch(e){
      if(e instanceof Fault&&e.field)e.field=`tasks[${i}].${e.field}`;throw e;}});
    return response({items:prepared.map((x:any)=>createTask(c.s,p,x,stamp(c)))},201);
  }
  return listTasks(c,p,path);
}
function listTasks(c:Context,p:Project,path:string){
  const url=c.url; let tasks=c.s.tasks.filter(t=>t.projectId===p.id);
  const status=url.searchParams.get("status"), label=url.searchParams.get("label")?.toLowerCase();
  const assignee=url.searchParams.get("assignee"), q=url.searchParams.get("q")?.toLowerCase();
  if(status){if(!["open","in_progress","done"].includes(status))fail(400,"validation","invalid status","status");tasks=tasks.filter(t=>t.status===status);}
  if(label)tasks=tasks.filter(t=>t.labels.includes(label));
  if(assignee){if(assignee==="me")tasks=tasks.filter(t=>t.assigneeId===c.user);else if(assignee==="none")tasks=tasks.filter(t=>t.assigneeId===null);
    else if(/^\d+$/.test(assignee))tasks=tasks.filter(t=>t.assigneeId===Number(assignee));else fail(400,"validation","invalid assignee","assignee");}
  if(q)tasks=tasks.filter(t=>(t.title+" "+t.description).toLowerCase().includes(q));
  const sort=url.searchParams.get("sort")??"created";
  if(!["created","-created","priority"].includes(sort))fail(400,"validation","invalid sort","sort");
  const rank={high:0,normal:1,low:2};
  tasks.sort((a,b)=>sort==="priority"?(rank[a.priority as keyof typeof rank]-rank[b.priority as keyof typeof rank]||a.number-b.number):
    sort==="-created"?b.number-a.number:a.number-b.number);
  const limitRaw=url.searchParams.get("limit")??"20",limit=Number(limitRaw);
  if(!/^\d+$/.test(limitRaw)||limit<1||limit>100)fail(400,"validation","invalid limit","limit");
  const cursor=url.searchParams.get("cursor"); let offset=0;
  if(cursor){try{offset=Number(atob(cursor));if(!Number.isInteger(offset)||offset<0)throw Error();}catch{fail(400,"validation","invalid cursor","cursor");}}
  const items=tasks.slice(offset,offset+limit),next=offset+limit<tasks.length?btoa(String(offset+limit)):null;
  return response({items,nextCursor:next});
}
export function createApp(options:Options={}):App{
  const s=createState(),now=options.now??Date.now;
  return {async fetch(req){
    const url=new URL(req.url),path=url.pathname,method=req.method;
    const c:Context={s,now,req,url};
    try{
      let body:any={};
      if(["POST","PATCH"].includes(method)&&req.body)body=await readBody(req);
      const out=userRoutes(c,path,method,body)??projectRoutes(c,path,method,body)??
        (path.startsWith("/tasks/") ? (auth(c),taskItemRoutes(s,c.user,path,method,body,stamp(c),req.headers)) : undefined);
      if(out){
        const token=/^Bearer (.+)$/.exec(req.headers.get("Authorization")??"")?.[1];
        if(token&&s.sessions.has(token)){
          const n=s.limits.get(token)?.count??0;
          out.headers.set("X-RateLimit-Limit","100");
          out.headers.set("X-RateLimit-Remaining",String(Math.max(0,100-n)));
        }
        return out;
      }
      const allow=routeMethods[`${method} ${path}`];
      if(allow) return response({error:{code:"method_not_allowed",message:"method not allowed"}},405,{Allow:allow});
      return response({error:{code:"not_found",message:"not found"}},404);
    }catch(e){const r=errorResponse(e);const retry=(e as any).retry;if(retry)r.headers.set("Retry-After",String(retry));return r;}
  }};
}
