import type { State, Task, Project } from "./state";
export class Fault extends Error {
  constructor(
    public status: number,
    public code: string,
    message: string,
    public field?: string,
  ) {
    super(message);
  }
}
export const fail = (
  status: number,
  code: string,
  message = code,
  field?: string,
): never => {
  throw new Fault(status, code, message, field);
};
export function answer(
  body: any,
  status = 200,
  headers: Record<string, string> = {},
) {
  return new Response(body === null ? null : JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json; charset=utf-8", ...headers },
  });
}
export function bodyObj(raw: any) {
  if (!raw || typeof raw !== "object" || Array.isArray(raw))
    fail(400, "validation", "Body must be an object");
  return raw;
}
export function auth(s: State, r: Request) {
  const t = (r.headers.get("Authorization") || "").match(/^Bearer (.+)$/)?.[1];
  const id = t ? s.sessions.get(t) : undefined;
  if (!id) fail(401, "unauthorized", "Unauthorized");
  return s.users.find((u) => u.id === id)!;
}
export function project(
  s: State,
  id: string,
  user: number,
  hidden = true,
): Project {
  const p = /^\d+$/.test(id)
    ? s.projects.find((x) => x.id === Number(id))
    : undefined;
  if (!p || !s.members.get(p.id)?.has(user)) {
    if (hidden) fail(404, "not_found", "Not found");
    fail(404, "not_found", "Not found");
  }
  return p!;
}
export function task(s: State, key: string, user: number): Task {
  const t = s.tasks.find((x) => x.key === key);
  if (!t) fail(404, "not_found", "Not found");
  if (!s.members.get(t!.projectId)?.has(user))
    fail(404, "not_found", "Not found");
  return t!;
}
export function writeRole(s: State, p: Project, id: number) {
  const role = s.members.get(p.id)?.get(id);
  if (!role) fail(404, "not_found", "Not found");
  if (role === "viewer") fail(403, "forbidden", "Forbidden");
}
export function validateTask(b: any, existing = false) {
  const allowed = ["title", "description", "priority", "labels", "assigneeId"];
  for (const k of Object.keys(b))
    if (!allowed.includes(k)) fail(400, "validation", "Unknown field", k);
  if (!existing || "title" in b) {
    if (
      typeof b.title !== "string" ||
      !b.title.trim() ||
      b.title.trim().length > 200
    )
      fail(400, "validation", "Invalid title", "title");
  }
  if ("description" in b && typeof b.description !== "string")
    fail(400, "validation", "Invalid description", "description");
  if ("priority" in b && !["low", "normal", "high"].includes(b.priority))
    fail(400, "validation", "Invalid priority", "priority");
  if (
    "labels" in b &&
    (!Array.isArray(b.labels) ||
      b.labels.some(
        (x: any) =>
          typeof x !== "string" ||
          !x.trim() ||
          x.trim().length > 30 ||
          /\s/.test(x.trim()),
      ))
  )
    fail(400, "validation", "Invalid labels", "labels");
}
export const labels = (xs: string[]) =>
  [...new Set(xs.map((x) => x.trim().toLowerCase()))].sort();
export function taskView(t: Task) {
  const { deps, ...out } = t;
  return out;
}
export function etag(t: Task) {
  return { ETag: `"${t.version}"` };
}
export function precondition(r: Request, t: Task) {
  const m = r.headers.get("If-Match");
  if (m && m !== `"${t.version}"`)
    fail(412, "precondition_failed", "Task has changed");
}
