import type { State } from "./model";
import { pair, projectView } from "./model";
import { check, fail, memberRole } from "./util";

export function projects(
  s: State,
  method: string,
  path: string,
  body: Record<string, any>,
  uid: number | undefined,
  now: number,
) {
  if (path === "/projects" && method === "POST") {
    check(typeof body.name === "string" && body.name.trim().length > 0 && body.name.trim().length <= 100, "name");
    check(typeof body.key === "string" && /^[A-Z]{2,6}$/.test(body.key), "key");
    if (s.projects.some((p) => p.key === body.key)) fail(409, "conflict", "key already exists", "key");
    const p = {
      id: s.projects.length + 1,
      name: body.name.trim(),
      key: body.key,
      ownerId: uid!,
      createdAt: new Date(now).toISOString(),
      next: 1,
    };
    s.projects.push(p);
    return { status: 201, body: projectView(p) };
  }
  if (path === "/projects" && method === "GET") {
    return {
      status: 200,
      body: s.projects.filter((p) => memberRole(s, p, uid!) !== undefined).map(projectView),
    };
  }
  const m = /^\/projects\/(\d+)(?:\/(members(?:\/\d+)?))?$/.exec(path);
  if (!m) return;
  const p = s.projects.find((x) => x.id === Number(m[1]));
  if (!p || !memberRole(s, p, uid!)) fail(404, "not_found", "project not found");
  if (!m[2] && method === "GET") return { status: 200, body: projectView(p) };
  if (m[2] === "members" && method === "GET") {
    const entries = [
      { userId: p.ownerId, role: "owner" },
      ...[...s.members]
        .filter(([k]) => k.startsWith(`${p.id}:`))
        .map(([k, role]) => ({ userId: Number(k.split(":")[1]), role }))
        .sort((a, b) => a.userId - b.userId),
    ];
    return { status: 200, body: entries };
  }
  if (m[2] === "members" && method === "POST") {
    if (uid !== p.ownerId) fail(403, "forbidden", "only the owner can add members");
    check(Number.isInteger(body.userId) && s.users.some((u) => u.id === body.userId), "userId");
    check(body.role === "member" || body.role === "viewer", "role");
    if (memberRole(s, p, body.userId)) fail(409, "conflict", "user is already a member");
    s.members.set(pair(p.id, body.userId), body.role);
    return { status: 201, body: { userId: body.userId, role: body.role } };
  }
  const del = /^members\/(\d+)$/.exec(m[2] || "");
  if (del && method === "DELETE") {
    if (uid !== p.ownerId) fail(403, "forbidden", "only the owner can remove members");
    const id = Number(del[1]);
    if (!s.members.delete(pair(p.id, id))) fail(404, "not_found", "member not found");
    for (const t of s.tasks.filter((x) => x.projectId === p.id && x.assigneeId === id)) t.assigneeId = null;
    return { status: 204, body: null };
  }
}
