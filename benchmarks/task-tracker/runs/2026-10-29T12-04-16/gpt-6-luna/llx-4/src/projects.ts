import type { State } from "./model";
import { auth } from "./auth";
import { fail, membership, need, obj } from "./util";

export function projectRoutes(s: State, method: string, path: string, body: any, token?: string, now = "") {
  if (path === "/projects" && method === "GET") {
    const uid = auth(s, token);
    return [200, s.projects.filter(p => membership(s, p.id, uid)).sort((a,b)=>a.id-b.id)];
  }
  if (path === "/projects" && method === "POST") {
    const uid = auth(s, token), b = obj(body);
    const name = typeof b.name === "string" ? b.name.trim() : "";
    need(name.length > 0 && name.length <= 100, "name");
    need(typeof b.key === "string" && /^[A-Z]{2,6}$/.test(b.key), "key");
    if (s.projects.some(p => p.key === b.key)) fail(409, "conflict", "Key already exists", "key");
    const p = { id: ++s.ids.project, name, key: b.key, ownerId: uid, createdAt: now };
    s.projects.push(p); s.members.set(p.id, new Map());
    return [201, p];
  }
  const m = path.match(/^\/projects\/(\d+)(?:\/(members)(?:\/(\d+))?|\/(stats))?$/);
  if (!m) return null;
  const id = Number(m[1]), p = s.projects.find(x => x.id === id);
  const uid = auth(s, token);
  if (!p || !membership(s, id, uid)) fail(404, "not_found", "Project not found");
  if (!m[2] && !m[4] && method === "GET") return [200, p];
  if (m[4] && method === "GET") {
    const tasks = s.tasks.filter(t => t.projectId === id);
    const byStatus = Object.fromEntries(["open","in_progress","done"].map(x=>[x,tasks.filter(t=>t.status===x).length]));
    const byPriority = Object.fromEntries(["high","normal","low"].map(x=>[x,tasks.filter(t=>t.priority===x).length]));
    const counts = new Map<number,number>();
    for (const t of tasks) if (t.assigneeId !== null) counts.set(t.assigneeId,(counts.get(t.assigneeId)??0)+1);
    const byAssignee = [...counts].sort((a,b)=>b[1]-a[1]||a[0]-b[0]).map(([userId,count])=>({userId,count}));
    return [200,{total:tasks.length,byStatus,byPriority,byAssignee}];
  }
  if (m[2] === "members") {
    const members = s.members.get(id)!;
    if (!m[3] && method === "GET") return [200,[{userId:p.ownerId,role:"owner"},
      ...([...members].sort((a,b)=>a[0]-b[0]).map(([userId,role])=>({userId,role})))]] ;
    if (!m[3] && method === "POST") {
      if (uid !== p.ownerId) fail(403,"forbidden","Owner required");
      const b = obj(body);
      need(Number.isInteger(b.userId) && s.users.some(u=>u.id===b.userId),"userId");
      need(["member","viewer"].includes(b.role),"role");
      if (b.userId === p.ownerId || members.has(b.userId)) fail(409,"conflict","Already a member");
      members.set(b.userId,b.role);
      return [201,{userId:b.userId,role:b.role}];
    }
    if (m[3] && method === "DELETE") {
      if (uid !== p.ownerId) fail(403,"forbidden","Owner required");
      const member = Number(m[3]);
      if (!members.delete(member)) fail(404,"not_found","Member not found");
      for (const t of s.tasks) if (t.projectId===id && t.assigneeId===member) t.assigneeId=null;
      return [204,null];
    }
  }
  return null;
}
