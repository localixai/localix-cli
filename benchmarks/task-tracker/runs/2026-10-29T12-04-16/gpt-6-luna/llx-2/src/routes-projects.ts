import { Ctx, Result, auth, member, validation } from "./routes";
import { fail, Fault } from "./http";
export async function projects(c: Ctx): Promise<Result> {
  const { db, path, method, body: b } = c, uid = auth(c);
  if (path === "/projects") {
    if (method === "GET") return { status: 200, body: db.projects.filter(p =>
      p.ownerId === uid || db.role(p.id, uid)).sort((a, z) => a.id-z.id) };
    if (method !== "POST") return methodFail("GET, POST");
    if (typeof b.name !== "string" || !b.name.trim() || b.name.trim().length > 100) validation("name");
    if (typeof b.key !== "string" || !/^[A-Z]{2,6}$/.test(b.key)) validation("key");
    if (db.projects.some(p => p.key === b.key)) fail(409, "conflict", "Key exists", "key");
    const p = { id: db.projects.length+1, name: b.name.trim(), key: b.key, ownerId: uid,
      createdAt: db.iso() };
    db.projects.push(p); return { status: 201, body: p };
  }
  const m = path.match(/^\/projects\/(\d+)(?:\/(members)(?:\/(\d+))?|\/(tasks)(?:\/(bulk))?|\/(stats))?$/);
  if (!m) fail(404, "not_found", "Not found");
  const id = Number(m[1]), sub = m[2] ? "members" : m[4] ? "tasks" : m[6] ? "stats" : undefined;
  const p = db.projects.find(x => x.id === id);
  if (sub === "members") {
    member(c, id);
    if (method === "GET") return { status: 200, body: [{ userId: p!.ownerId, role: "owner" },
      ...[...db.members].filter(([k]) => k.startsWith(`${id}:`)).map(([k, role]) =>
        ({ userId: Number(k.split(":")[1]), role })).sort((a,b) => a.userId-b.userId)] };
    if (method === "DELETE") {
      const target = Number(m[3] ?? c.query.get("userId")), key = `${id}:${target}`;
      if (!db.members.has(key)) fail(404, "not_found", "Member not found");
      db.members.delete(key);
      for (const t of db.tasks.filter(x => x.projectId === id && x.assigneeId === target)) {
        t.assigneeId = null;
      }
      return { status: 204 };
    }
    member(c, id, true);
    if (db.role(id, uid) !== "owner") fail(403, "forbidden", "Owner required");
    if (!["member", "viewer"].includes(b.role)) validation("role");
    if (!db.users.some(u => u.id === b.userId)) validation("userId");
    const key = `${id}:${b.userId}`;
    if (db.role(id, b.userId)) fail(409, "conflict", "Already a member");
    db.members.set(key, b.role); return { status: 201, body: { userId: b.userId, role: b.role } };
  }
  member(c, id);
  if (sub === "stats") return stats(db, id);
  if (sub === "tasks") {
    const mod = await import("./routes-tasks");
    return mod.projectTasks(c, id, !!m[5]);
  }
  if (!p) fail(404, "not_found", "Project not found");
  if (method !== "GET") return methodFail("GET");
  return { status: 200, body: p };
}
function stats(db: Ctx["db"], id: number): Result {
  const ts = db.tasks.filter(t => t.projectId === id);
  const count = (key: string, vals: string[]) => Object.fromEntries(vals.map(v =>
    [v, ts.filter(t => (t as any)[key] === v).length]));
  const assignees = new Map<number, number>();
  ts.forEach(t => { if (t.assigneeId) assignees.set(t.assigneeId, (assignees.get(t.assigneeId) ?? 0)+1); });
  return { status: 200, body: { total: ts.length, byStatus: count("status", ["open", "in_progress", "done"]),
    byPriority: count("priority", ["high", "normal", "low"]),
    byAssignee: [...assignees].map(([userId, count]) => ({ userId, count }))
      .sort((a,b) => b.count-a.count || a.userId-b.userId) } };
}
function methodFail(allow: string): never {
  throw new Fault(405, "method_not_allowed", "Method not allowed", undefined, allow);
}
