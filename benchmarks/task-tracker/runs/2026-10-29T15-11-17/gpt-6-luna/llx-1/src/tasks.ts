import type { RequestCtx } from "./http";
import { bad, body, conflict, empty, err, missing, ok } from "./http";
import { iso } from "./model";
import { access, etag, normalizeLabels, versionCheck } from "./task-common";
import { createTasks } from "./task-create";
import { listTasks } from "./task-list";

export async function tasks(method: string, path: string, c: RequestCtx) {
  if (method === "POST") {
    const creation = await createTasks(path,c);
    if (creation) return creation;
  }
  if (method==="GET") { const listing=listTasks(path,c); if (listing) return listing; }
  const m=path.match(/^\/tasks\/([^/]+)(?:\/(.*))?$/); if (!m) return undefined;
  const key=m[1]!, sub=m[2]??"", write=method!=="GET";
  const a=access(c,key,write && !["comments"].includes(sub)); if (a.result) return a.result;
  const t=a.task!, s=c.state;
  if (sub==="" && method==="GET") return ok(t,200,etag(t.version));
  if (sub==="" && method==="DELETE") {
    if (a.role==="member" && t.createdBy!==c.userId) return err(403,"forbidden","Forbidden");
    s.tasks=s.tasks.filter(x=>x!==t);
    for(const set of s.deps.values()) set.delete(t.id);
    s.deps.delete(t.id); return empty();
  }
  if (sub==="history" && method==="GET") return ok(s.history.get(key)??[]);
  if (sub==="comments" && method==="GET") return ok(s.comments.get(key)??[]);
  if (sub==="comments" && method==="POST") {
    const b=await body(c.req); if(!b)return bad("body");
    if(typeof b.body!=="string"||!b.body.trim()||b.body.length>2000)return bad("body");
    const arr=s.comments.get(key)??[];
    const comment={id:++s.ids.comment,authorId:c.userId,body:b.body.trim(),createdAt:iso(c)};
    arr.push(comment);s.comments.set(key,arr);return ok(comment,201);
  }
  if (sub==="transitions" && method==="POST") return transition(c,t);
  if (sub==="" && method==="PATCH") return patch(c,t);
  if (sub==="dependencies" && method==="GET") {
    const depends=[...(s.deps.get(t.id)??[])].map(id=>s.tasks.find(x=>x.id===id)).filter(Boolean);
    const blocks=[...s.deps].filter(([,set])=>set.has(t.id)).map(([id])=>s.tasks.find(x=>x.id===id)).filter(Boolean);
    return ok({key,dependsOn:depends.sort((x,y)=>x!.number-y!.number).map(x=>x!.key),
      blocks:blocks.sort((x,y)=>x!.number-y!.number).map(x=>x!.key)});
  }
  if (sub==="dependencies" && method==="POST") return addDependency(c,t);
  const d=sub.match(/^dependencies\/(.+)$/);
  if(d&&method==="DELETE") {
    const other=s.tasks.find(x=>x.key===d[1]&&x.projectId===t.projectId), set=s.deps.get(t.id);
    if(!other||!set?.has(other.id))return missing();
    set.delete(other.id);return empty();
  }
  return undefined;
}
function record(c:RequestCtx,t:any,field:string,from:any,to:any) {
  const k=t.key, h=c.state.history.get(k)??[];
  h.push({at:iso(c),by:c.userId,field,from,to});c.state.history.set(k,h);
}
async function patch(c:RequestCtx,t:any) {
  const b=await body(c.req);if(!b)return bad("body");
  const allowed=["title","description","priority","labels","assigneeId"];
  for(const k of Object.keys(b))if(!allowed.includes(k))return bad(k);
  if(versionCheck(c,t.version))return err(412,"precondition_failed","Version changed");
  const changes:any={};
  if ("title" in b) {
    if (typeof b.title !== "string" || !b.title.trim() || b.title.trim().length > 200) return bad("title");
    changes.title = b.title.trim();
  }
  if ("description" in b) {
    if (typeof b.description !== "string") return bad("description");
    changes.description = b.description;
  }
  if ("priority" in b) {
    if (!["low", "normal", "high"].includes(b.priority)) return bad("priority");
    changes.priority = b.priority;
  }
  if ("labels" in b) {
    const x = normalizeLabels(b.labels);
    if (!x) return bad("labels");
    changes.labels = x;
  }
  if ("assigneeId" in b) {
    const assigned = c.state.members.has(`${t.projectId}:${b.assigneeId}`);
    const owner = c.state.projects.find(p => p.id === t.projectId)?.ownerId === b.assigneeId;
    if (b.assigneeId !== null && !assigned && !owner) return bad("assigneeId");
    changes.assigneeId = b.assigneeId;
  }
  let changed=false;
  for(const k of Object.keys(changes).sort()) {
    const from=t[k], to=changes[k];
    if(JSON.stringify(from)!==JSON.stringify(to)){record(c,t,k,from,to);t[k]=to;changed=true;}
  }
  if(changed){t.version++;t.updatedAt=iso(c);}
  return ok(t,200,etag(t.version));
}
async function transition(c:RequestCtx,t:any) {
  const b=await body(c.req);if(!b)return bad("body");
  if(!["open","in_progress","done"].includes(b.to))return bad("to");
  if(versionCheck(c,t.version))return err(412,"precondition_failed","Version changed");
  const valid=(t.status==="open"&&b.to==="in_progress")||
    (t.status==="in_progress"&&(b.to==="open"||b.to==="done"))||(t.status==="done"&&b.to==="open");
  if(!valid)return err(409,"invalid_transition","Invalid transition");
  if(b.to==="done"&&[...(c.state.deps.get(t.id)??[])].some(id=>c.state.tasks.some(x=>x.id===id&&x.status!=="done")))
    return err(409,"blocked","Blocked");
  const from=t.status;t.status=b.to;t.version++;t.updatedAt=iso(c);record(c,t,"status",from,b.to);
  return ok(t,200,etag(t.version));
}
async function addDependency(c:RequestCtx,t:any) {
  const b=await body(c.req);if(!b)return bad("body");
  const other=typeof b.dependsOn==="string"?c.state.tasks.find(x=>x.key===b.dependsOn):undefined;
  if(!other||other.projectId!==t.projectId||other.id===t.id)return bad("dependsOn");
  const set=c.state.deps.get(t.id)??new Set<number>();
  if(set.has(other.id))return conflict();
  const reaches=(id:number,seen=new Set<number>()):boolean=>{
    if(id===t.id)return true;if(seen.has(id))return false;seen.add(id);
    return [...(c.state.deps.get(id)??[])].some(next => reaches(next,seen));
  };
  if(reaches(other.id))return err(409,"dependency_cycle","Dependency cycle");
  set.add(other.id);c.state.deps.set(t.id,set);
  const depends=[...set].map(id=>c.state.tasks.find(x=>x.id===id)!).sort((x,y)=>x.number-y.number);
  return ok({key:t.key,dependsOn:depends.map(x=>x.key),blocks:[]},201);
}
