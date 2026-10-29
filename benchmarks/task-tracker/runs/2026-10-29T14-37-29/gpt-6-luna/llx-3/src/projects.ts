import type { State } from "./state";
import { bad, body, err, iso, json, member } from "./util";
import { taskList } from "./task-list";

export async function projectsRoute(req: Request, s: State, u: any, url: URL) {
  const path = url.pathname, p = path.match(/^\/projects\/(\d+)(?:\/(.*))?$/);
  if (path === "/projects") {
    if (req.method === "GET") return json(s.projects.filter(x => member(x, u.id)).map(({ members, ...x }) => x));
    if (req.method !== "POST") return null;
    const b = await body(req); if (!b) return err(400, "validation", "Invalid JSON");
    const name = typeof b.name === "string" ? b.name.trim() : "";
    if (!name || name.length > 100) return bad("name");
    if (typeof b.key !== "string" || !/^[A-Z]{2,6}$/.test(b.key)) return bad("key");
    if (s.projects.some(x => x.key === b.key)) return err(409, "conflict", "Key taken", "key");
    const project = { id: s.projects.length + 1, name, key: b.key, ownerId: u.id,
      createdAt: iso(s), members: [] };
    s.projects.push(project);
    const { members: _, ...view } = project;
    return json(view, 201);
  }
  if (!p) return null;
  const project = s.projects.find(x => x.id === Number(p[1]));
  const sub = p[2] ?? "";
  if (sub === "") {
    if (!member(project, u.id)) return err(404, "not_found", "Project not found");
    const { members: _, ...view } = project;
    return req.method === "GET" ? json(view) : null;
  }
  if (!project || !member(project, u.id)) return err(404, "not_found", "Project not found");
  if (sub === "members") {
    if (req.method === "GET") return json([{ userId: project.ownerId, role: "owner" },
      ...[...project.members].sort((a: any, b: any) => a.userId - b.userId)]);
    if (req.method !== "POST") return null;
    if (project.ownerId !== u.id) return err(403, "forbidden", "Owner required");
    const b = await body(req); if (!b) return err(400, "validation", "Invalid JSON");
    if (!["member", "viewer"].includes(b.role)) return bad("role");
    if (!s.users.some(x => x.id === b.userId)) return bad("userId");
    if (b.userId === project.ownerId || project.members.some((x: any) => x.userId === b.userId))
      return err(409, "conflict", "Already a member");
    project.members.push({ userId: b.userId, role: b.role });
    return json({ userId: b.userId, role: b.role }, 201);
  }
  const remove = sub.match(/^members\/(\d+)$/);
  if (remove && req.method === "DELETE") {
    if (project.ownerId !== u.id) return err(403, "forbidden", "Owner required");
    const id = Number(remove[1]), index = project.members.findIndex((x: any) => x.userId === id);
    if (index < 0) return err(404, "not_found", "Member not found");
    project.members.splice(index, 1);
    s.tasks.filter(t => t.projectId === project.id && t.assigneeId === id).forEach(t => t.assigneeId = null);
    return new Response(null, { status: 204 });
  }
  if (sub === "tasks" && req.method === "GET") return taskList(req, s, project, u, url);
  if (sub === "stats" && req.method === "GET") {
    const tasks = s.tasks.filter(t => t.projectId === project.id && !t.deleted);
    const count = (field: string, values: string[]) => Object.fromEntries(values.map(v =>
      [v, tasks.filter(t => t[field] === v).length]));
    const assignees = new Map<number, number>();
    tasks.forEach(t => { if (t.assigneeId) assignees.set(t.assigneeId, (assignees.get(t.assigneeId) ?? 0) + 1); });
    return json({ total: tasks.length, byStatus: count("status", ["open", "in_progress", "done"]),
      byPriority: count("priority", ["high", "normal", "low"]),
      byAssignee: [...assignees].map(([userId, count]) => ({ userId, count }))
        .sort((a, b) => b.count - a.count || a.userId - b.userId) });
  }
  return null;
}
