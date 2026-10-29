import type { State } from "./model";
import { iso, projectOf } from "./model";
import {
  ApiError,
  auth,
  body,
  member,
  owner,
  projectAccess,
  respond,
} from "./http";
export async function projects(
  req: Request,
  url: URL,
  s: State,
): Promise<Response | null> {
  const path = url.pathname,
    method = req.method;
  if (path === "/projects" && method === "GET") {
    const uid = auth(req, s);
    return respond(s.projects.filter((p) => member(s, p, uid)));
  }
  if (path === "/projects" && method === "POST") {
    const uid = auth(req, s),
      b = await body(req);
    const name = typeof b.name === "string" ? b.name.trim() : "";
    const key = typeof b.key === "string" ? b.key : "";
    if (!name || name.length > 100)
      throw new ApiError(400, "validation", "name is invalid", "name");
    if (!/^[A-Z]{2,6}$/.test(key))
      throw new ApiError(400, "validation", "key is invalid", "key");
    if (s.projects.some((p) => p.key === key))
      throw new ApiError(409, "conflict", "key already used", "key");
    const p = {
      id: s.projects.length + 1,
      name,
      key,
      ownerId: uid,
      createdAt: iso(s),
    };
    s.projects.push(p);
    s.members.set(p.id, new Map());
    return respond(p, 201);
  }
  const match =
    /^\/projects\/(\d+)(?:\/(members)(?:\/(\d+))?|\/(stats)|\/tasks(?:\/(bulk))?)?$/.exec(
      path,
    );
  if (!match) return null;
  const id = Number(match[1]),
    uid = auth(req, s),
    p = projectAccess(s, id, uid);
  if (!match[2] && !match[4] && !path.endsWith("/tasks")) {
    if (method === "GET") return respond(p);
    return null;
  }
  if (match[2] === "members") {
    if (!match[3] && method === "GET") {
      const entries = [...(s.members.get(id) || [])]
        .sort(([a], [b]) => a - b)
        .map(([userId, role]) => ({ userId, role }));
      return respond([{ userId: p.ownerId, role: "owner" }, ...entries]);
    }
    if (!match[3] && method === "POST") {
      owner(s, p, uid);
      const b = await body(req);
      if (!["member", "viewer"].includes(b.role))
        throw new ApiError(400, "validation", "role is invalid", "role");
      if (!s.users.some((u) => u.id === b.userId))
        throw new ApiError(400, "validation", "userId is invalid", "userId");
      if (b.userId === p.ownerId || s.members.get(id)?.has(b.userId))
        throw new ApiError(409, "conflict", "already a member");
      s.members.get(id)!.set(b.userId, b.role);
      return respond({ userId: b.userId, role: b.role }, 201);
    }
    if (match[3] && method === "DELETE") {
      owner(s, p, uid);
      const target = Number(match[3]);
      if (!s.members.get(id)?.delete(target))
        throw new ApiError(404, "not_found", "member not found");
      for (const t of s.tasks.filter(
        (x) => x.projectId === id && x.assigneeId === target,
      ))
        t.assigneeId = null;
      return respond(null, 204);
    }
  }
  if (match[4] && method === "GET") {
    const tasks = s.tasks.filter((t) => t.projectId === id);
    const byStatus = { open: 0, in_progress: 0, done: 0 },
      byPriority = { high: 0, normal: 0, low: 0 };
    const assignees = new Map<number, number>();
    for (const t of tasks) {
      byStatus[t.status as keyof typeof byStatus]++;
      byPriority[t.priority as keyof typeof byPriority]++;
      if (t.assigneeId !== null)
        assignees.set(t.assigneeId, (assignees.get(t.assigneeId) || 0) + 1);
    }
    return respond({
      total: tasks.length,
      byStatus,
      byPriority,
      byAssignee: [...assignees]
        .map(([userId, count]) => ({ userId, count }))
        .sort((a, b) => b.count - a.count || a.userId - b.userId),
    });
  }
  return null;
}
