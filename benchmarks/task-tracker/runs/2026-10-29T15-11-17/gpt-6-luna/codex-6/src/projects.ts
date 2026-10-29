import { fail, stamp, validation } from "./http";
import type { Context, Project, User } from "./model";
export function memberRole(c: Context, project: Project, userId: number) {
  return c.s.members.get(project.id)?.get(userId);
}
export function visibleProject(c: Context, id: number, userId: number) {
  const p = c.s.projects.find((x) => x.id === id);
  if (!p || !memberRole(c, p, userId)) fail(404, "not_found", "Project not found");
  return p;
}
export function writable(c: Context, p: Project, userId: number) {
  const role = memberRole(c, p, userId);
  if (!role) fail(404, "not_found", "Project not found");
  if (role === "viewer") fail(403, "forbidden", "You cannot change this project");
}
export function createProject(c: Context, user: User, body: Record<string, any>) {
  const name = typeof body.name === "string" ? body.name.trim() : "";
  const key = body.key;
  if (!name || name.length > 100) validation("name");
  if (typeof key !== "string" || !/^[A-Z]{2,6}$/.test(key)) validation("key");
  if (c.s.projects.some((p) => p.key === key)) fail(409, "conflict", "Project key is in use", "key");
  const p: Project = { id: ++c.s.ids.project, name, key, ownerId: user.id,
    createdAt: stamp(c.now), next: 1 };
  c.s.projects.push(p);
  c.s.members.set(p.id, new Map([[user.id, "owner"]]));
  return projectView(p);
}
export function projectView(p: Project) {
  return { id: p.id, name: p.name, key: p.key, ownerId: p.ownerId, createdAt: p.createdAt };
}
export function addMember(c: Context, p: Project, body: Record<string, any>) {
  if (!Number.isInteger(body.userId) || !c.s.users.some((u) => u.id === body.userId)) validation("userId");
  if (body.role !== "member" && body.role !== "viewer") validation("role");
  const members = c.s.members.get(p.id)!;
  if (members.has(body.userId)) fail(409, "conflict", "User is already a member");
  members.set(body.userId, body.role);
  return { userId: body.userId, role: body.role };
}
export function removeMember(c: Context, p: Project, userId: number) {
  const members = c.s.members.get(p.id)!;
  if (!members.has(userId) || userId === p.ownerId) fail(404, "not_found", "Member not found");
  members.delete(userId);
  for (const t of c.s.tasks) if (t.projectId === p.id && t.assigneeId === userId) t.assigneeId = null;
}
