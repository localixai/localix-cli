import type { Project, State } from "./model";
import { iso, memberRole, visibleProject } from "./model";
import { auth, bad, body, empty, fail, json } from "./http";

const idOf = (text: string) => /^\d+$/.test(text) ? Number(text) : NaN;
function getProject(s: State, id: number, user: number) {
  const p = s.projects.find((x) => x.id === id);
  if (!p || !visibleProject(s, p, user)) fail(404, "not_found", "Project not found");
  return p;
}
export async function projects(req: Request, path: string[], s: State): Promise<Response | undefined> {
  const m = req.method;
  if (path[0] !== "projects") return undefined;
  if (path.length === 1) {
    if (m === "GET") {
      const u = auth(req, s);
      return json(s.projects.filter((p) => visibleProject(s, p, u)).map(({ next, ...p }) => p));
    }
    if (m === "POST") {
      const u = auth(req, s); const b = await body(req);
      if (typeof b.name !== "string" || !b.name.trim() || b.name.trim().length > 100) bad("name");
      if (typeof b.key !== "string" || !/^[A-Z]{2,6}$/.test(b.key)) bad("key");
      if (s.projects.some((p) => p.key === b.key)) fail(409, "conflict", "Project key already exists", "key");
      const p: Project = {
        id: ++s.ids.project, name: b.name.trim(), key: b.key, ownerId: u,
        createdAt: iso(s), next: 1,
      };
      s.projects.push(p); s.members.set(p.id, new Map());
      return json({ id: p.id, name: p.name, key: p.key, ownerId: p.ownerId, createdAt: p.createdAt }, 201);
    }
    return undefined;
  }
  const p = getProject(s, idOf(path[1]!), auth(req, s));
  if (path.length === 2) {
    if (m === "GET") { const { next, ...out } = p; return json(out); }
    return undefined;
  }
  if (path[2] !== "members") return undefined;
  if (path.length === 3 && m === "GET") {
    const members = [{ userId: p.ownerId, role: "owner" }, ...[...(s.members.get(p.id) ?? [])]
      .map(([userId, role]) => ({ userId, role })).sort((a, b) => a.userId - b.userId)];
    return json(members);
  }
  if (path.length === 3 && m === "POST") {
    const u = auth(req, s);
    if (u !== p.ownerId) fail(403, "forbidden", "Only the owner can add members");
    const b = await body(req);
    if (!Number.isInteger(b.userId) || !s.users.some((x) => x.id === b.userId)) bad("userId");
    if (b.role !== "member" && b.role !== "viewer") bad("role");
    const members = s.members.get(p.id)!;
    if (b.userId === p.ownerId || members.has(b.userId)) fail(409, "conflict", "Already a member");
    members.set(b.userId, b.role);
    return json({ userId: b.userId, role: b.role }, 201);
  }
  if (path.length === 4 && m === "DELETE") {
    const u = auth(req, s);
    if (u !== p.ownerId) fail(403, "forbidden", "Only the owner can remove members");
    const who = idOf(path[3]!); const members = s.members.get(p.id)!;
    if (!members.delete(who)) fail(404, "not_found", "Member not found");
    for (const t of s.tasks) if (t.projectId === p.id && t.assigneeId === who) t.assigneeId = null;
    return empty();
  }
  return undefined;
}
