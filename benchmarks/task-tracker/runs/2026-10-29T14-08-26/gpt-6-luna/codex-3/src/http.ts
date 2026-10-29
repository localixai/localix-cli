import type { State, Project, Task, Member } from "./model";

export class Fault extends Error {
  constructor(public status: number, public code: string, message: string, public field?: string) {
    super(message);
  }
}
export function fail(status: number, code: string, message: string, field?: string): never {
  throw new Fault(status, code, message, field);
}
export const validation = (field: string): never => fail(400, "validation", `Invalid ${field}`, field);
export const obj = (v: unknown): Record<string, any> => {
  if (!v || typeof v !== "object" || Array.isArray(v)) validation("body");
  return v as Record<string, any>;
};
export const auth = (req: Request, s: State): number => {
  const h = req.headers.get("Authorization") || "";
  const m = /^Bearer (.+)$/.exec(h);
  const id = m && s.sessions.get(m[1]!);
  if (!id) fail(401, "unauthorized", "Authentication required");
  return id;
};
export const projectOf = (s: State, id: number, user: number): Project => {
  const p = s.projects.find((x) => x.id === id);
  if (!p || !role(s, p, user)) fail(404, "not_found", "Project not found");
  return p;
};
export const role = (s: State, p: Project, user: number): string | undefined =>
  s.members.get(p.id)?.find((m) => m.userId === user)?.role;
export const writable = (s: State, p: Project, user: number): void => {
  const r = role(s, p, user);
  if (r === "viewer") fail(403, "forbidden", "You may not change this project");
};
export const taskOf = (s: State, key: string, user: number): Task => {
  const t = s.tasks.find((x) => x.key === key);
  if (!t) fail(404, "not_found", "Task not found");
  const p = s.projects.find((x) => x.id === t.projectId)!;
  if (!role(s, p, user)) fail(404, "not_found", "Task not found");
  return t;
};
export const checkVersion = (req: Request, t: Task): void => {
  const match = req.headers.get("If-Match");
  if (match && match !== `"${t.version}"`) fail(412, "precondition_failed", "Task has changed");
};
export const response = (body: unknown, status = 200, headers: Record<string, string> | Headers = {}): Response =>
  new Response(body === null ? null : JSON.stringify(body), {
    status, headers: { "Content-Type": "application/json; charset=utf-8", ...Object.fromEntries(new Headers(headers)) },
  });
export const errorResponse = (e: unknown): Response => {
  if (e instanceof Fault) {
    const error: Record<string, string> = { code: e.code, message: e.message };
    if (e.field) error.field = e.field;
    return response({ error }, e.status);
  }
  return response({ error: { code: "internal", message: "Internal error" } }, 500);
};
export const empty = (status = 204): Response => new Response(null, { status });
export const time = (now: () => number): string => new Date(now()).toISOString();
export const memberList = (s: State, p: Project): Member[] => s.members.get(p.id) || [];
