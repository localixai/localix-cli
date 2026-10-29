import type { State } from "./model";
import { clean, err, makeTask, member, ok, task, validTask } from "./util";
export function taskRoutes(s: State, p: string[], m: string, b: any, uid: number,
  req: Request, now: string, out: (x: any, n?: number, e?: string) => Response): Response {
  const no=()=>err("not_found",404), find=(k:string)=>task(s,k);
  if (p[0] === "projects" && p[2] === "stats" && p[1] && m === "GET") {
    const pid = Number(p[1]);
    if (!s.projects.some(x => x.id === pid) || !member(s, pid, uid)) return no();
    const a = s.tasks.filter(t => t.projectId === pid);
    const count = (field: string, values: string[]) => Object.fromEntries(values.map(v =>
      [v, a.filter(t => (t as any)[field] === v).length]));
    const people = new Map<number, number>();
    a.forEach(t => { if (t.assigneeId !== null) people.set(t.assigneeId, (people.get(t.assigneeId) ?? 0) + 1); });
    return out({ total: a.length, byStatus: count("status", ["open", "in_progress", "done"]),
      byPriority: count("priority", ["high", "normal", "low"]),
      byAssignee: [...people].sort((x, y) => y[1] - x[1] || x[0] - y[0])
        .map(([userId, n]) => ({ userId, count: n })) });
  }
  if(p[0]==="projects"&&p[2]==="tasks"&&p[1]) {
    const id=Number(p[1]), pr=s.projects.find(x=>x.id===id), role=member(s,id,uid);
    if(!pr||!role)return no();
    if(p.length===3&&m==="GET") {
      let a=s.tasks.filter(t=>t.projectId===id);
      const q=new URLSearchParams(req.url.split("?")[1]??"");
      if (q.has("status")) {
        if (!["open", "in_progress", "done"].includes(q.get("status")!))
          return err("validation", 400, "status");
        a = a.filter(t => t.status === q.get("status"));
      }
      if (q.has("assignee")) {
        const x = q.get("assignee")!;
        if (x === "none") a = a.filter(t => t.assigneeId === null);
        else if (x === "me") a = a.filter(t => t.assigneeId === uid);
        else if (/^\d+$/.test(x)) a = a.filter(t => t.assigneeId === Number(x));
        else return err("validation", 400, "assignee");
      }
      if (q.has("label")) a = a.filter(t => t.labels.includes(q.get("label")!.toLowerCase()));
      if (q.has("q")) {
        a = a.filter(t => (t.title + " " + t.description).toLowerCase()
          .includes(q.get("q")!.toLowerCase()));
      }
      let sort=q.get("sort")??"created";
      if(!["created","-created","priority"].includes(sort))return err("validation",400,"sort");
      a.sort((x, y) => sort === "priority"
        ? ["high", "normal", "low"].indexOf(x.priority) -
          ["high", "normal", "low"].indexOf(y.priority) || x.number - y.number
        : sort === "-created" ? y.number - x.number : x.number - y.number);
      const limit = Number(q.get("limit") ?? 20);
      if (!Number.isInteger(limit) || limit < 1 || limit > 100)
        return err("validation", 400, "limit");
      let at = 0;
      if (q.has("cursor")) {
        try {
          at = Number(atob(q.get("cursor")!));
          if (!Number.isInteger(at) || at < 0) throw 0;
        } catch { return err("validation", 400, "cursor"); }
      }
      const items = a.slice(at, at + limit);
      return out({ items: items.map(clean),
        nextCursor: at + limit < a.length ? btoa(String(at + limit)) : null });
    }
    if((p.length===3||p[3]==="bulk")&&m==="POST") {
      if(role==="viewer")return err("forbidden",403);
      const bulk=p[3]==="bulk", entries=bulk?b.tasks:[b];
      if(bulk&&(!Array.isArray(entries)||entries.length<1||entries.length>50))return err("validation",400,"tasks");
      if(!Array.isArray(entries))return err("validation",400,"tasks");
      for (let i = 0; i < entries.length; i++) {
        const f = validTask(entries[i], s, id);
        if (f) return err("validation", 400, bulk ? `tasks[${i}].${f}` : f);
      }
      const key=req.headers.get("Idempotency-Key"), cache=key?s.idem.get(`${uid}:${key}`):undefined;
      if (cache && Date.parse(now) - cache.at < 86400000) {
        if (cache.body !== JSON.stringify(b)) return err("idempotency_mismatch", 422);
        return ok(cache.result, 201, { "Idempotent-Replayed": "true" });
      }
      const made=entries.map((x:any)=>clean(makeTask(s,pr,uid,x,now)));
      const result=bulk?{items:made}:made[0];
      if(key)s.idem.set(`${uid}:${key}`,{body:JSON.stringify(b),result,at:Date.parse(now)});
      return out(result,201,made[0]?.version);
    }
    if(p.length===4&&p[3]==="stats"&&m==="GET") {
      const a = s.tasks.filter(t => t.projectId === id);
      const by = (k: string, vals: string[]) => Object.fromEntries(vals.map(v =>
        [v, a.filter(t => (t as any)[k] === v).length]));
      const counts = new Map<number, number>();
      a.forEach(t => {
        if (t.assigneeId !== null)
          counts.set(t.assigneeId, (counts.get(t.assigneeId) ?? 0) + 1);
      });
      const byAssignee = [...counts].sort((x, y) => y[1] - x[1] || x[0] - y[0])
        .map(([userId, count]) => ({ userId, count }));
      return out({ total: a.length, byStatus: by("status", ["open", "in_progress", "done"]),
        byPriority: by("priority", ["high", "normal", "low"]), byAssignee });
    }
  }
  if(p[0]!=="tasks"||!p[1])return no();
  const t=find(p[1]);if(!t||!member(s,t.projectId,uid))return no();
  const role=member(s,t.projectId,uid), write=()=>role==="viewer"?err("forbidden",403):null;
  if(p.length===2&&m==="GET")return out(clean(t),200,String(t.version));
  if(p.length===2&&m==="PATCH") {
    const denied=write();if(denied)return denied;
    const allowed=["title","description","priority","labels","assigneeId"];
    for(const k of Object.keys(b))if(!allowed.includes(k))return err("validation",400,k);
    const f=validTask({...t,...b},s,t.projectId);if(f)return err("validation",400,f);
    const match=req.headers.get("If-Match");if(match&&match!==`"${t.version}"`)return err("precondition_failed",412);
    for (const k of Object.keys(b).sort()) {
      const from = (t as any)[k];
      const to = k === "labels"
        ? [...new Set(b[k].map((x: string) => x.trim().toLowerCase()))].sort()
        : k === "title" ? b[k].trim() : b[k];
      if (JSON.stringify(from) !== JSON.stringify(to)) {
        t.history.push({ at: now, by: uid, field: k, from, to });
        (t as any)[k] = to;
      }
    }
    if(t.history.at(-1)?.at===now){t.updatedAt=now;t.version++;}
    return out(clean(t),200,String(t.version));
  }
  if(p[2]==="transitions"&&m==="POST") {
    const denied=write();if(denied)return denied;
    if(!["open","in_progress","done"].includes(b.to))return err("validation",400,"to");
    const match=req.headers.get("If-Match");if(match&&match!==`"${t.version}"`)return err("precondition_failed",412);
    const allowed:any={open:["in_progress"],in_progress:["open","done"],done:["open"]};
    if(!allowed[t.status].includes(b.to))return err("invalid_transition",409);
    if(b.to==="done"&&[...t.depends].some(id=>s.tasks.find(x=>x.id===id)?.status!=="done"))return err("blocked",409);
    t.history.push({at:now,by:uid,field:"status",from:t.status,to:b.to});t.status=b.to;t.version++;t.updatedAt=now;
    return out(clean(t),200,String(t.version));
  }
  if(p[2]==="history"&&m==="GET")return out(t.history);
  if(p[2]==="comments") {
    if(m==="GET")return out(t.comments);
    if (m === "POST") {
      const body = typeof b.body === "string" ? b.body.trim() : "";
      if (!body || body.length > 2000) return err("validation", 400, "body");
      const c = { id: ++s.commentIds, authorId: uid, body, createdAt: now };
      t.comments.push(c);
      return out(c, 201);
    }
  }
  if(p[2]==="dependencies") {
    if (m === "GET") {
      const keys = (ids: Set<number>) => [...ids]
        .map(id => s.tasks.find(x => x.id === id)).filter(Boolean)
        .sort((a: any, b: any) => a.number - b.number).map((x: any) => x.key);
      const blocks = new Set(s.tasks.filter(x => x.depends.has(t.id)).map(x => x.id));
      return out({ key: t.key, dependsOn: keys(t.depends), blocks: keys(blocks) });
    }
    if(m==="POST"){const denied=write();if(denied)return denied;const d=find(b.dependsOn);
      if(!d||d.id===t.id||d.projectId!==t.projectId)return err("validation",400,"dependsOn");
      if(t.depends.has(d.id))return err("conflict",409);
      const reaches = (n: number, seen = new Set<number>()): boolean => {
        if (n === t.id) return true;
        if (seen.has(n)) return false;
        seen.add(n);
        const deps = s.tasks.find(x => x.id === n)?.depends ?? [];
        return [...deps].some(x => reaches(x, seen));
      };
      if(reaches(d.id))return err("dependency_cycle",409);t.depends.add(d.id);
      return out({key:t.key,dependsOn:[...t.depends].map(id=>s.tasks.find(x=>x.id===id)!.key).sort(),blocks:[]},201);
    }
    if (m === "DELETE" && p[3]) {
      const d = find(p[3]);
      if (!d || !t.depends.delete(d.id)) return no();
      return new Response(null, { status: 204 });
    }
  }
  if (p.length === 2 && m === "DELETE") {
    const denied = role === "owner" ? null
      : role === "member" && t.createdBy === uid ? null : err("forbidden", 403);
    if (denied) return denied;
    s.tasks = s.tasks.filter(x => x !== t);
    s.tasks.forEach(x => x.depends.delete(t.id));
    return new Response(null, { status: 204 });
  }
  return no();
}
