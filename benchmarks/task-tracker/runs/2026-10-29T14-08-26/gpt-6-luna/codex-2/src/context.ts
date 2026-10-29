import type { Project, Store, Task, User } from "./model";
import { fail } from "./http";
export type Ctx = { s:Store; now:()=>number; request:Request; url:URL; user?:User };
export const iso=(c:Ctx)=>new Date(c.now()).toISOString();
export const auth=(c:Ctx):User=>{
  const h=c.request.headers.get("Authorization")||"";
  const token=h.startsWith("Bearer ")?h.slice(7):"";
  const id=c.s.sessions.get(token), user=c.s.users.find(x=>x.id===id);
  if(!user) return fail(401,"unauthorized","Authentication required");
  c.user=user; return user;
};
export const project=(c:Ctx,id:number):Project=>{
  const p=c.s.projects.find(x=>x.id===id);
  if(!p) return fail(404,"not_found","Project not found");
  return p;
};
export const role=(c:Ctx,p:Project):string|undefined=>
  c.s.members.get(p.id)?.get(auth(c).id);
export function visibleProject(c:Ctx,id:number):Project {
  const p=project(c,id); if(!role(c,p)) fail(404,"not_found","Project not found"); return p;
}
export function writable(c:Ctx,p:Project):User {
  const u=auth(c), r=c.s.members.get(p.id)?.get(u.id);
  if(!r) fail(404,"not_found","Project not found");
  if(r==="viewer") fail(403,"forbidden","Forbidden"); return u;
}
export function task(c:Ctx,key:string):Task {
  const m=/^([A-Z]{2,6})-(\d+)$/.exec(key);
  const p=m&&c.s.projects.find(x=>x.key===m[1]);
  const t=p&&c.s.tasks.find(x=>x.projectId===p.id&&x.number===Number(m?.[2]));
  if(!t) return fail(404,"not_found","Task not found");
  visibleProject(c,t.projectId); t.key=`${p!.key}-${t.number}`; return t;
}
export function normalizedLabels(v:any,field="labels"):string[] {
  if(!Array.isArray(v)) fail(400,"validation","Invalid labels",field);
  const out:string[]=[];
  for(const raw of v){
    if(typeof raw!=="string") fail(400,"validation","Invalid labels",field);
    const x=raw.trim().toLowerCase();
    if(!x||x.length>30||/\s/.test(x)) fail(400,"validation","Invalid labels",field);
    if(!out.includes(x)) out.push(x);
  }
  return out.sort();
}
