import type { Ctx } from "./context";
import { iso, task, writable } from "./context";
import { bad, fail, json } from "./http";
import type { Task } from "./model";
function etag(c:Ctx,t:Task):boolean {
  const match=c.request.headers.get("If-Match");
  if(match&&match!==`"${t.version}"`) fail(412,"precondition_failed","Task has changed");
  return !!match;
}
function record(c:Ctx,t:Task,field:string,from:any,to:any):void {
  c.s.histories.get(t.id)!.push({at:iso(c),by:c.user!.id,field,from,to});
}
export function detailRoutes(c:Ctx,method:string,path:string,b:any):Response|undefined {
  let m=/^\/tasks\/([^/]+)(?:\/(history|transitions|dependencies|comments)(?:\/([^/]+))?)?$/.exec(path);
  if(!m)return;
  const t=task(c,m[1]||""), part=m[2];
  if(!part&&method==="GET")return json(t,200,{ETag:`"${t.version}"`});
  if(part==="history"&&method==="GET")return json(c.s.histories.get(t.id)||[]);
  if(part==="dependencies")return dependency(c,t,method,m[3],b);
  if(part==="comments")return comments(c,t,method,b);
  if(method==="DELETE"&&!part){
    const p=c.s.projects.find(x=>x.id===t.projectId)!;
    const user=awaitAuth(c);
    if(user.id!==p.ownerId&&t.createdBy!==user.id) fail(403,"forbidden","Forbidden");
    c.s.tasks=c.s.tasks.filter(x=>x.id!==t.id);
    for(const set of c.s.deps.values()) set.delete(t.id); c.s.deps.delete(t.id);
    return new Response(null,{status:204});
  }
  if((method==="PATCH"&&!part)||(method==="POST"&&part==="transitions")){
    writable(c,c.s.projects.find(x=>x.id===t.projectId)!); etag(c,t);
    if(part==="transitions"){
      if(!["open","in_progress","done"].includes(b.to))bad("to");
      const allowed=(t.status==="open"&&b.to==="in_progress")||
        (t.status==="in_progress"&&["open","done"].includes(b.to))||
        (t.status==="done"&&b.to==="open");
      if(!allowed)fail(409,"invalid_transition","Invalid transition");
      if(b.to==="done"&&[...(c.s.deps.get(t.id)||[])].some(id=>c.s.tasks.some(x=>x.id===id&&x.status!=="done")))
        fail(409,"blocked","Dependencies are not done");
      const from=t.status;t.status=b.to;record(c,t,"status",from,b.to);touch(c,t);
    }else patchTask(c,t,b);
    return json(t,200,{ETag:`"${t.version}"`});
  }
}
function awaitAuth(c:Ctx){return c.user!;}
function touch(c:Ctx,t:Task){t.version++;t.updatedAt=iso(c);}
function patchTask(c:Ctx,t:Task,b:any){
  for(const k of Object.keys(b))if(!["title","description","priority","labels","assigneeId"].includes(k))bad(k);
  const order=["assigneeId","description","labels","priority","title"];
  for(const f of order){if(!(f in b))continue;let value:any=b[f];
    if(f==="title"){if(typeof value!=="string"||!(value=value.trim())||value.length>200)bad(f);}
    if(f==="description"&&typeof value!=="string")bad(f);
    if(f==="priority"&&!['low','normal','high'].includes(value))bad(f);
    if(f==="labels")value=(awaitLabels)(value);
    if(f==="assigneeId"&&value!==null&&(!Number.isInteger(value)||!c.s.members.get(t.projectId)?.has(value)))bad(f);
    const old=(t as any)[f];
    if(JSON.stringify(old)!==JSON.stringify(value)){
      record(c,t,f,old,value);(t as any)[f]=value;
    }
  }
  if((c.s.histories.get(t.id)||[]).some(h=>h.by===c.user!.id&&h.at===iso(c)))touch(c,t);
}
function awaitLabels(v:any){
  if(!Array.isArray(v))bad("labels");
  const arr=v.map((x:any)=>{
    if(typeof x!=="string")bad("labels");
    const y=x.trim().toLowerCase();
    if(!y||y.length>30||/\s/.test(y))bad("labels");
    return y;
  });
  return [...new Set(arr)].sort();
}
function dependency(c:Ctx,t:Task,method:string,other:string|undefined,b:any):Response {
  const set=c.s.deps.get(t.id)!;
  if(method==="GET"){
    const dependsOn=[...set].map(id=>c.s.tasks.find(x=>x.id===id))
      .filter(Boolean).sort((a,b)=>a!.number-b!.number).map(x=>x!.key);
    const blocks=[...c.s.deps].filter(([,v])=>v.has(t.id))
      .map(([id])=>c.s.tasks.find(x=>x.id===id)).filter(Boolean)
      .sort((a,b)=>a!.number-b!.number).map(x=>x!.key);
    return json({key:`${c.s.projects.find(p=>p.id===t.projectId)!.key}-${t.number}`,dependsOn,blocks});
  }
  if(method==="POST"){
    writable(c,c.s.projects.find(p=>p.id===t.projectId)!);
    if(typeof b.dependsOn!=="string")bad("dependsOn");
    const match=/^([A-Z]{2,6})-(\d+)$/.exec(b.dependsOn);
    const d=match&&c.s.tasks.find(x=>x.key===b.dependsOn);
    if(!d)return bad("dependsOn");
    if(d.id===t.id||d.projectId!==t.projectId)bad("dependsOn");
    if(set.has(d.id))fail(409,"conflict","Dependency exists");
    const reaches=(id:number,seen=new Set<number>()):boolean=>id===t.id?true:seen.has(id)?false:
      [...(c.s.deps.get(id)||[])].some(x=>reaches(x,seen.add(id)));
    if(reaches(d.id))fail(409,"dependency_cycle","Dependency cycle");set.add(d.id);
    const dependsOn=[...set].map(id=>c.s.tasks.find(x=>x.id===id)!)
      .sort((a,b)=>a.number-b.number)
      .map(x=>`${c.s.projects.find(p=>p.id===x.projectId)!.key}-${x.number}`);
    const key=`${c.s.projects.find(p=>p.id===t.projectId)!.key}-${t.number}`;
    return json({key,dependsOn,blocks:[]},201);
  }
  if(method==="DELETE"&&other){
    const key=c.s.projects.find(p=>p.id===t.projectId)!.key;
    const d=task(c,`${key}-${other.split("-").pop()}`);
    writable(c,c.s.projects.find(p=>p.id===t.projectId)!);
    if(!set.delete(d.id))fail(404,"not_found","Dependency not found");
    return new Response(null,{status:204});
  }
  return json({});
}
function comments(c:Ctx,t:Task,method:string,b:any):Response {
  const list=c.s.comments.get(t.id)||[];c.s.comments.set(t.id,list);
  if(method==="GET")return json(list);
  if(method==="POST"){
    const body=typeof b.body==="string"?b.body.trim():"";if(!body||body.length>2000)bad("body");
    const item={id:c.s.nextComment++,authorId:c.user!.id,body,createdAt:iso(c)};list.push(item);return json(item,201);
  }
  return json({});
}
