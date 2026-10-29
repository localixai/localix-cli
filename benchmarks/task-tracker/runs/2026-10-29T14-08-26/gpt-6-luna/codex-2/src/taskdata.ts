import type { Ctx } from "./context";
import { iso, normalizedLabels, project, task, visibleProject, writable } from "./context";
import { bad, fail, json } from "./http";
import { detailRoutes } from "./taskdetail";
import type { Task } from "./model";
function validate(c:Ctx,p:any,b:any,fieldPrefix=""):any {
  const title=typeof b.title==="string"?b.title.trim():"";
  if(!title||title.length>200) bad(fieldPrefix+"title");
  const description=b.description===undefined?"":b.description;
  if(typeof description!=="string") bad(fieldPrefix+"description");
  const priority=b.priority??"normal";
  if(!["low","normal","high"].includes(priority)) bad(fieldPrefix+"priority");
  const labels=normalizedLabels(b.labels??[],fieldPrefix+"labels");
  const assigneeId=b.assigneeId??null;
  if(assigneeId!==null&&(!Number.isInteger(assigneeId)||
    !c.s.members.get(p.id)?.has(assigneeId))) bad(fieldPrefix+"assigneeId");
  return {title,description,priority,labels,assigneeId};
}
export function create(c:Ctx,pid:number,b:any,number?:number):Task {
  const p=project(c,pid), u=writable(c,p), fields=validate(c,p,b);
  const n=number??(c.s.taskNumbers.get(pid)||0)+1;
  c.s.taskNumbers.set(pid,Math.max(n,c.s.taskNumbers.get(pid)||0));
  const t:Task={id:c.s.nextTask++,number:n,projectId:pid,key:`${p.key}-${n}`,...fields,
    status:"open",createdBy:u.id,
    createdAt:iso(c),updatedAt:iso(c),version:1};
  c.s.tasks.push(t); c.s.histories.set(t.id,[]); c.s.deps.set(t.id,new Set()); return t;
}
export function taskRoutes(c:Ctx,method:string,path:string,b:any):Response|undefined {
  let m=/^\/projects\/(\d+)\/tasks(?:\/(bulk))?$/.exec(path);
  if(m){
    const p=visibleProject(c,Number(m[1]));
    if(method==="POST"&&!m[2]){
      const u=writable(c,p), key=c.request.headers.get("Idempotency-Key");
      const ikey=key?`${u.id}:${key}`:"", old=ikey&&c.s.idempotency.get(ikey);
      if(old){
        if(old.expires<=c.now()) c.s.idempotency.delete(ikey);
        else if(old.body===JSON.stringify(b)){
          return json(old.task,201,{
            "ETag":`"${old.task.version}"`,"Idempotent-Replayed":"true",
          });
        }
        else fail(422,"idempotency_mismatch","Idempotency key reused");
      }
      const t=create(c,p.id,b);
      if(ikey)c.s.idempotency.set(ikey,{
        body:JSON.stringify(b),task:t,expires:c.now()+86400000,
      });
      return json(t,201,{ETag:`"${t.version}"`});
    }
    if(method==="POST"&&m[2]){
      writable(c,p); if(!Array.isArray(b.tasks)||b.tasks.length<1||b.tasks.length>50) bad("tasks");
      for(let i=0;i<b.tasks.length;i++) validate(c,p,b.tasks[i],`tasks[${i}].`);
      const items=b.tasks.map((x:any)=>create(c,p.id,x));
      return json({items},201);
    }
    if(method==="GET"&&!m[2]) return listing(c,p.id);
  }
  return detailRoutes(c,method,path,b);
}
function listing(c:Ctx,pid:number):Response {
  const p=project(c,pid), q=c.url.searchParams, status=q.get("status"), sort=q.get("sort")||"created";
  if(status&&!['open','in_progress','done'].includes(status)) bad("status");
  if(!['created','-created','priority'].includes(sort)) bad("sort");
  const limitRaw=q.get("limit")||"20", limit=Number(limitRaw);
  if(!/^\d+$/.test(limitRaw)||limit<1||limit>100) bad("limit");
  let items=c.s.tasks.filter(t=>t.projectId===pid);
  if(status) items=items.filter(t=>t.status===status);
  const assignee=q.get("assignee");
  if(assignee){
    if(assignee==="none")items=items.filter(t=>t.assigneeId===null);
    else if(assignee==="me")items=items.filter(t=>t.assigneeId===c.user?.id);
    else if(/^\d+$/.test(assignee)){
      items=items.filter(t=>t.assigneeId===Number(assignee));
    }else bad("assignee");
  }
  const label=q.get("label");if(label)items=items.filter(t=>t.labels.includes(label.toLowerCase()));
  const term=q.get("q")?.toLowerCase();
  if(term)items=items.filter(t=>
    `${t.title} ${t.description}`.toLowerCase().includes(term));
  if(sort==="-created")items.reverse();
  if(sort==="priority")items.sort((a,b)=>
    ["high","normal","low"].indexOf(a.priority)-
    ["high","normal","low"].indexOf(b.priority)||a.number-b.number);
  let offset=0,cursor=q.get("cursor");
  if(cursor){try{offset=Number(atob(cursor));if(!Number.isInteger(offset)||offset<1)throw 0;}catch{bad("cursor");}}
  const page=items.slice(offset,offset+limit),next=offset+limit<items.length?btoa(String(offset+limit)):null;
  void p; return json({items:page,nextCursor:next});
}
