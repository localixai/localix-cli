import type { Store, User, Project, Task } from "./model";
import { fail } from "./http";
export type Ctx = {
  s: Store;
  now: () => number;
  user: User;
  body: Record<string, any>;
  req: Request;
  params: string[];
  url: URL;
};
export const iso = (c: Ctx) => new Date(c.now()).toISOString();
export const me = (c: Ctx): User => c.user;
export function project(c: Ctx, id: string): Project {
  const p = c.s.projects.find((x) => x.id === Number(id));
  if (!p) return fail(404, "not_found", "Project not found");
  if (!p.members.has(c.user.id)) fail(404, "not_found", "Project not found");
  return p;
}
export function task(c: Ctx, key: string): Task {
  const t = c.s.tasks.find((x) => x.key === key && !x.deleted);
  if (!t) return fail(404, "not_found", "Task not found");
  project(c, String(t.projectId));
  return t;
}
export const role = (p: Project, id: number) => p.members.get(id);
export function write(p: Project, id: number) {
  const r = role(p, id);
  if (!r) fail(404, "not_found", "Project not found");
  if (r === "viewer") fail(403, "forbidden", "Forbidden");
}
export const userDto = (u: User) => ({
  id: u.id,
  email: u.email,
  name: u.name,
});
export const projectDto = (p: Project) => ({
  id: p.id,
  name: p.name,
  key: p.key,
  ownerId: p.ownerId,
  createdAt: p.createdAt,
});
export const taskDto = ({ deps, history, comments, deleted, ...t }: Task) => t;
