import type { State } from "./types";
import { fail, valid, owner, member, projectFor } from "./util";
import { projectTasks } from "./task-list";
const json = (x: unknown, status = 200) => Response.json(x, { status });
const noContent = () => new Response(null, { status: 204 });
export function projects(s: State, method: string, path: string, b: any, uid: number, url: URL,
  headers: Headers): Response {
  if (path === "/projects" && method === "POST") {
    const name = typeof b.name === "string" ? b.name.trim() : "";
    valid(name.length > 0 && name.length <= 100, "name");
    valid(typeof b.key === "string" && /^[A-Z]{2,6}$/.test(b.key), "key");
    if (s.projects.some((p) => p.key === b.key)) fail(409, "conflict", "Key taken", "key");
    const p = { id: ++s.ids.project, name, key: b.key, ownerId: uid,
      createdAt: new Date(s.now()).toISOString(), members: new Map<number, string>(), next: 1 };
    s.projects.push(p);
    return json({ id: p.id, name, key: p.key, ownerId: uid, createdAt: p.createdAt }, 201);
  }
  if (path === "/projects" && method === "GET") return json(s.projects.filter((p) =>
    member(p, uid)).map(strip).sort((a, b) => a.id - b.id));
  const m = path.match(/^\/projects\/(\d+)(.*)$/);
  if (!m) return fail(404, "not_found");
  const p = projectFor(s, Number(m[1]!), uid), sub = m[2] ?? "";
  if (!sub && method === "GET") return json(strip(p));
  if (sub === "/members" && method === "GET")
    return json([{ userId: p.ownerId, role: "owner" }, ...[...p.members]
      .sort(([a], [b]) => a - b).map(([userId, role]) => ({ userId, role }))]);
  if (sub === "/members" && method === "POST") {
    owner(p, uid);
    valid(Number.isInteger(b.userId) && s.users.some((u) => u.id === b.userId), "userId");
    valid(["member", "viewer"].includes(b.role), "role");
    if (member(p, b.userId)) fail(409, "conflict");
    p.members.set(b.userId, b.role);
    return json({ userId: b.userId, role: b.role }, 201);
  }
  const rm = sub.match(/^\/members\/(\d+)$/);
  if (rm && method === "DELETE") {
    owner(p, uid);
    const id = Number(rm[1]);
    if (!p.members.delete(id)) fail(404, "not_found");
    for (const t of s.tasks) if (t.projectId === p.id && t.assigneeId === id) t.assigneeId = null;
    return noContent();
  }
  if (sub === "/stats" && method === "GET") {
    const ts = s.tasks.filter((t) => t.projectId === p.id);
    const count = (field: string, values: string[]) => Object.fromEntries(values.map((x) =>
      [x, ts.filter((t) => (t as any)[field] === x).length]));
    const assignees = new Map<number, number>();
    for (const t of ts) if (t.assigneeId !== null) assignees.set(t.assigneeId, (assignees.get(t.assigneeId) ?? 0) + 1);
    return json({ total: ts.length, byStatus: count("status", ["open", "in_progress", "done"]),
      byPriority: count("priority", ["high", "normal", "low"]),
      byAssignee: [...assignees].map(([userId, count]) => ({ userId, count }))
        .sort((a, b) => b.count - a.count || a.userId - b.userId) });
  }
  if ((sub === "/tasks" || sub === "/tasks/bulk") && ["GET", "POST"].includes(method))
    return projectTasks(s, method, sub, p, uid, b, url, headers);
  return fail(404, "not_found");
}
function strip(p: State["projects"][number]) {
  return { id: p.id, name: p.name, key: p.key, ownerId: p.ownerId, createdAt: p.createdAt };
}
