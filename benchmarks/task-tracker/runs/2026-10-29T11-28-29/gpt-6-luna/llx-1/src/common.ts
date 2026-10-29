import type { State } from "./model";
export class Fault extends Error {
  constructor(public status: number, public code: string, message = code, public field?: string) {
    super(message);
  }
}
export const fail = (status: number, code: string, field?: string): never => {
  throw new Fault(status, code, code.replaceAll("_", " "), field);
};
export function obj(value: unknown): Record<string, any> {
  if (!value || typeof value !== "object" || Array.isArray(value)) fail(400, "validation");
  return value as Record<string, any>;
}
export function valid(cond: unknown, field: string): asserts cond {
  if (!cond) fail(400, "validation", field);
}
export function auth(s: State, req: Request): number {
  const token = /^Bearer (.+)$/.exec(req.headers.get("Authorization") ?? "")?.[1];
  if (!token) fail(401, "unauthorized");
  const id = s.tokens.get(token!);
  if (id === undefined) fail(401, "unauthorized");
  return id;
}
export const memberRole = (s: State, project: number, user: number) =>
  s.members.get(project)?.get(user);
export function projectAccess(s: State, id: number, user: number) {
  const p = s.projects.find(x => x.id === id);
  if (!p || !memberRole(s, id, user)) fail(404, "not_found");
  return p;
}
export function taskAccess(s: State, key: string, user: number) {
  const t = s.tasks.find(x => x.key === key);
  if (!t || !memberRole(s, t.projectId, user)) fail(404, "not_found");
  return t;
}
export function writeRole(s: State, t: { projectId: number }, user: number) {
  const role = memberRole(s, t.projectId, user);
  if (!role) fail(404, "not_found");
  if (role === "viewer") fail(403, "forbidden");
}
