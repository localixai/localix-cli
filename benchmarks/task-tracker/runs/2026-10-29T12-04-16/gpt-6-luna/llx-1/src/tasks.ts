import {
  bad, conflict, error, etag, iso, json, member, record, taskByKey,
  type Context, type Project, type Task,
} from "./shared";
export async function tasksRoute(c: Context, req: Request, p: Project | undefined,
  uid: number, path: string[], url: URL): Promise<Response> {
  const method = req.method;
  const b: any = method === "GET" || method === "DELETE" ? {} : await req.json().catch(() => null);
  if (b === null) return bad("body");
  if (p && path[0] === "stats") {
    const ts = c.tasks.filter(t => t.projectId === p.id), statuses: any = { open: 0, in_progress: 0, done: 0 };
    const priorities: any = { high: 0, normal: 0, low: 0 }, assignees = new Map<number, number>();
    ts.forEach(t => {
      statuses[t.status]++;
      priorities[t.priority]++;
      if (t.assigneeId) assignees.set(t.assigneeId, (assignees.get(t.assigneeId) ?? 0) + 1);
    });
    return json({ total: ts.length, byStatus: statuses, byPriority: priorities,
      byAssignee: [...assignees].sort((a,b) => b[1]-a[1] || a[0]-b[0]).map(([userId,count]) => ({ userId,count })) });
  }
  if (p && path.length === 2 && path[0] === "tasks" && path[1] === "bulk" && method === "POST") {
    if (!["owner", "member"].includes(member(c, p, uid)!)) return error(403, "forbidden", "Forbidden");
    if (!Array.isArray(b.tasks) || b.tasks.length < 1 || b.tasks.length > 50) return bad("tasks");
    const vals = b.tasks.map((x: any) => validate(x));
    const index = vals.findIndex((x: any) => typeof x === "string");
    if (index >= 0) return bad(`tasks[${index}].${vals[index]}`);
    if (vals.some((x: any) => x.assigneeId !== null && !member(c, p, x.assigneeId)))
      return bad("assigneeId");
    const items: Task[] = [];
    for (const value of b.tasks) {
      const item = await (await create(c, p, uid, value)).json() as Task;
      items.push(item);
    }
    return json({ items }, 201);
  }
  if (p && path.length === 1 && path[0] === "tasks" && method === "POST") {
    if (!["owner", "member"].includes(member(c, p, uid)!)) return error(403, "forbidden", "Forbidden");
    const header = req.headers.get("Idempotency-Key");
    const key = header ? `${uid}:${header}` : "";
    const saved = key ? c.idem.get(key) : undefined;
    const fingerprint = JSON.stringify(b);
    if (saved && c.now() - saved.time < 86400000) {
      if (saved.body !== fingerprint) return error(422, "idempotency_mismatch", "Key reused");
      return json(saved.result, 201, {
        ETag: `"${saved.result.version}"`, "Idempotent-Replayed": "true",
      });
    }
    if (saved) c.idem.delete(key);
    const reply = await create(c, p, uid, b);
    if (key && reply.status === 201)
      c.idem.set(key, {
        body: fingerprint, time: c.now(), result: await reply.clone().json() as Task,
      });
    return reply;
  }
  if (p && path.length === 1 && path[0] === "tasks" && method === "GET") {
    let ts = c.tasks.filter(t => t.projectId === p.id);
    const q = url.searchParams;
    if (q.has("status")) ts = ts.filter(t => t.status === q.get("status"));
    if (q.has("assignee")) {
      const a = q.get("assignee");
      ts = ts.filter(t => a === "none" ? !t.assigneeId :
        t.assigneeId === (a === "me" ? uid : Number(a)));
    }
    if (q.has("label")) ts=ts.filter(t=>t.labels.includes(q.get("label")!.toLowerCase()));
    if (q.has("q")) ts=ts.filter(t=>(t.title+" "+t.description).toLowerCase().includes(q.get("q")!.toLowerCase()));
    if (q.get("sort")==="-created") ts.reverse();
    if (q.get("sort") === "priority") ts.sort((a,b) =>
      ["high","normal","low"].indexOf(a.priority) -
      ["high","normal","low"].indexOf(b.priority) || a.number-b.number);
    const limit=Number(q.get("limit")??20), cursor=Number(q.get("cursor")??0);
    if (q.has("cursor") && (!/^\d+$/.test(q.get("cursor")!) || cursor < 0)) return bad("cursor");
    if (q.has("limit") && (!Number.isInteger(limit)||limit<1||limit>100)) return bad("limit");
    if (q.has("status")&&!["open","in_progress","done"].includes(q.get("status")!)) return bad("status");
    if (q.has("sort")&&!["-created","priority"].includes(q.get("sort")!)) return bad("sort");
    if (q.has("assignee") && !["me","none"].includes(q.get("assignee")!) &&
      !/^\d+$/.test(q.get("assignee")!)) return bad("assignee");
    const page=ts.slice(cursor,cursor+limit), next=cursor+limit<ts.length?String(cursor+limit):null;
    return json({ items: page, nextCursor: next });
  }
  const key = path[0]!, t = taskByKey(c,key);
  if (!t) return error(404,"not_found","Not found");
  const project=c.projects.find(x=>x.id===t.projectId)!;
  if (!member(c,project,uid)) return error(404,"not_found","Not found");
  if (path[1] === "comments") {
    if (method==="GET") return json(c.comments.filter(x=>x.taskId===t.id)
      .sort((a,b)=>a.id-b.id).map(({taskId,...x})=>x));
    const text=typeof b.body==="string"?b.body.trim():"";
    if (!text||text.length>2000) return bad("body");
    const item={id:++c.ids.comment,authorId:uid,body:text,createdAt:iso(c),taskId:t.id}; c.comments.push(item);
    const {taskId,...out}=item; return json(out,201);
  }
  if (path[1] === "history") return json(c.history.get(t.id) ?? []);
  if (path[1] === "dependencies") {
    const deps=c.dependencies.get(t.id)??new Set<number>();
    const names = (set: Set<number>) => [...set].map(id => c.tasks.find(x => x.id === id))
      .filter(Boolean).sort((a,b) => a!.number-b!.number).map(x => x!.key);
    if (method === "GET") {
      const blockers = new Set([...c.dependencies]
        .filter(([,s]) => s.has(t.id)).map(([id]) => id));
      return json({ key, dependsOn: names(deps), blocks: names(blockers) });
    }
    if (path[2] && method==="DELETE") {
      const dep=taskByKey(c,path[2]); if (!dep||!deps.delete(dep.id)) return error(404,"not_found","Not found");
      return new Response(null,{status:204});
    }
    if (method==="POST") {
      if (!["owner","member"].includes(member(c,project,uid)!)) return error(403,"forbidden","Forbidden");
      const d=taskByKey(c,b.dependsOn);
      if (!d||d.id===t.id||d.projectId!==t.projectId) return bad("dependsOn");
      if (deps.has(d.id)) return conflict("dependsOn");
      if (reaches(c,d.id,t.id)) return error(409,"dependency_cycle","Dependency cycle");
      deps.add(d.id); c.dependencies.set(t.id,deps); return json({key,dependsOn:names(deps),blocks:[]},201);
    }
  }
  if (method==="GET") return etag(t,200,t.version);
  if (!["owner","member"].includes(member(c,project,uid)!)) return error(403,"forbidden","Forbidden");
  if (method==="DELETE") {
    if (project.ownerId!==uid&&t.createdBy!==uid) return error(403,"forbidden","Forbidden");
    c.tasks=c.tasks.filter(x=>x!==t); c.dependencies.delete(t.id);
    c.dependencies.forEach(s=>s.delete(t.id)); return new Response(null,{status:204});
  }
  if (req.headers.has("If-Match") &&
    req.headers.get("If-Match") !== `"${t.version}"`)
    return error(412,"precondition_failed","Changed");
  if (path[1]==="transitions") return transition(c,t,uid,b);
  if (method==="PATCH") return update(c,t,uid,b);
  return error(405,"method_not_allowed","Method not allowed");
}
function reaches(c:Context,from:number,to:number,seen=new Set<number>()):boolean{
  if(from===to)return true;if(seen.has(from))return false;seen.add(from);
  return [...(c.dependencies.get(from)??[])].some(x=>reaches(c,x,to,seen));
}
async function create(c:Context,p:Project,uid:number,b:any):Promise<Response>{
  const v=validate(b);if(typeof v==="string")return bad(v);
  if(v.assigneeId!==null&&!member(c,p,v.assigneeId))return bad("assigneeId");
  const n=(c.ids.number.get(p.id) ?? 0)+1;
  c.ids.number.set(p.id,n);
  const t:Task={id:++c.ids.task,key:`${p.key}-${n}`,number:n,projectId:p.id,...v,status:"open",createdBy:uid,
    createdAt:iso(c),updatedAt:iso(c),version:1};
  c.tasks.push(t);return etag(t,201,1);
}
function validate(b:any):any{
  if(typeof b?.title!=="string"||!b.title.trim()||b.title.trim().length>200)return "title";
  if(b.description!==undefined&&typeof b.description!=="string")return "description";
  if(b.priority!==undefined&&!["low","normal","high"].includes(b.priority))return "priority";
  if (b.labels !== undefined && (!Array.isArray(b.labels) ||
    b.labels.some((x: any) => typeof x !== "string" || !x.trim() ||
      x.trim().length > 30 || /\s/.test(x.trim())))) return "labels";
  if(b.assigneeId!==undefined&&b.assigneeId!==null&&!Number.isInteger(b.assigneeId))return "assigneeId";
  return {title:b.title.trim(),description:b.description??"",priority:b.priority??"normal",
    labels: b.labels ? [...new Set(b.labels.map((x: string) =>
      x.trim().toLowerCase()))].sort() : [], assigneeId: b.assigneeId ?? null };
}
function update(c:Context,t:Task,uid:number,b:any):Response{
  const fields=["title","description","priority","labels","assigneeId"];
  for(const k of Object.keys(b))if(!fields.includes(k))return bad(k);
  const merged=validate({...t,...b});if(typeof merged==="string")return bad(merged);
  if (merged.assigneeId !== null &&
    !member(c, c.projects.find(p => p.id === t.projectId)!, merged.assigneeId))
    return bad("assigneeId");
  for (const k of fields.sort()) {
    const from = (t as any)[k], to = merged[k];
    if (JSON.stringify(from) !== JSON.stringify(to)) {
      record(c,t,uid,k,from,to);
      (t as any)[k] = to;
    }
  }
  if(c.history.get(t.id)?.length){t.version++;t.updatedAt=iso(c);}
  return etag(t,200,t.version);
}
function transition(c:Context,t:Task,uid:number,b:any):Response{
  const to=b.to;if(!["open","in_progress","done"].includes(to))return bad("to");
  const valid = (t.status === "open" && to === "in_progress") ||
    (t.status === "in_progress" && (to === "open" || to === "done")) ||
    (t.status === "done" && to === "open");
  if(!valid)return error(409,"invalid_transition","Invalid transition");
  if (to === "done" && [...(c.dependencies.get(t.id) ?? [])]
    .some(id => c.tasks.some(x => x.id === id && x.status !== "done")))
    return error(409,"blocked","Blocked");
  record(c,t,uid,"status",t.status,to);t.status=to;t.version++;t.updatedAt=iso(c);return etag(t,200,t.version);
}
