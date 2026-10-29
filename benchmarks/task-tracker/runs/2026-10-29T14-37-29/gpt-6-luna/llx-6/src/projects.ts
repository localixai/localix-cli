import { error, ok, time, type State } from "./model";
export function projects(s: State, method: string, path: string, body: any, uid: number) {
  if (path === "/projects" && method === "POST") {
    const name = typeof body?.name === "string" ? body.name.trim() : "";
    const key = body?.key;
    if (!name || name.length > 100) return error("validation", "Invalid name", 400, "name");
    if (typeof key !== "string" || !/^[A-Z]{2,6}$/.test(key)) return error("validation", "Invalid key", 400, "key");
    if (s.projects.some(p => p.key === key)) return error("conflict", "Key exists", 409, "key");
    const p = {
      id: ++s.ids.project, name, key, ownerId: uid, createdAt: time(s),
      members: new Map<number, string>(),
    };
    p.members.set(uid, "owner"); s.projects.push(p);
    return ok({ id:p.id,name,key,ownerId:uid,createdAt:p.createdAt },201);
  }
  if (path === "/projects" && method === "GET") {
    const owned=s.projects.filter(p=>p.members.has(uid));
    return ok(owned.map(p=>({
      id:p.id,name:p.name,key:p.key,ownerId:p.ownerId,createdAt:p.createdAt,
    })));
  }
  const del = path.match(/^\/projects\/(\d+)\/members\/(\d+)$/);
  const m = path.match(/^\/projects\/(\d+)(?:\/(members|stats))?$/);
  if (!m && !del) return null;
  const p = s.projects.find(x=>x.id===Number(m?.[1]||del?.[1]));
  if (!p || !p.members.has(uid)) return error("not_found","Not found",404);
  if (!m?.[2] && method === "GET" && !del)
    return ok({id:p.id,name:p.name,key:p.key,ownerId:p.ownerId,createdAt:p.createdAt});
  if (m?.[2] === "members") {
    if (method === "GET") return ok([...p.members].sort(([a],[b])=>a-b).map(([userId,role])=>({userId,role})));
    if (method === "POST") {
      if (uid !== p.ownerId) return error("forbidden","Forbidden",403);
      const user = s.users.find(x=>x.id===body?.userId);
      if (!user) return error("validation","Unknown user",400,"userId");
      if (!["member","viewer"].includes(body?.role)) return error("validation","Invalid role",400,"role");
      if (p.members.has(user.id)) return error("conflict","Already a member",409);
      p.members.set(user.id,body.role); return ok({userId:user.id,role:body.role},201);
    }
  }
  if(m?.[2]==="stats"&&method==="GET") {
    const ts=s.tasks.filter(t=>t.projectId===p.id);
    const count=(field:string,values:string[])=>Object.fromEntries(
      values.map(v=>[v,ts.filter(t=>(t as any)[field]===v).length]),
    );
    const assignees=new Map<number,number>();
    ts.forEach(t=>{if(t.assigneeId!==null)assignees.set(t.assigneeId,(assignees.get(t.assigneeId)||0)+1);});
    const byAssignee=[...assignees].sort((a,b)=>b[1]-a[1]||a[0]-b[0])
      .map(([userId,count])=>({userId,count}));
    return ok({
      total:ts.length,byStatus:count("status",["open","in_progress","done"]),
      byPriority:count("priority",["high","normal","low"]),byAssignee,
    });
  }
  if (del && method === "DELETE") {
    if (uid !== p.ownerId) return error("forbidden","Forbidden",403);
    const id=Number(del[2]); if (!p.members.has(id) || id===p.ownerId) return error("not_found","Not found",404);
    p.members.delete(id); s.tasks.filter(t=>t.projectId===p.id && t.assigneeId===id).forEach(t=>t.assigneeId=null);
    return ok(null,204);
  }
  return null;
}
