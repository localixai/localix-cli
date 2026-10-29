import type { State, Project, Task } from "./model";
import { projectOf, taskByKey } from "./model";
export class ApiError extends Error {
  constructor(
    public status: number,
    public code: string,
    message: string,
    public field?: string,
  ) {
    super(message);
  }
}
export const fail = (field: string, message = "is invalid"): never => {
  throw new ApiError(400, "validation", `${field} ${message}`, field);
};
export function respond(
  body: unknown,
  status = 200,
  headers: Record<string, string> = {},
) {
  return new Response(status === 204 ? null : JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json; charset=utf-8", ...headers },
  });
}
export function errorResponse(e: unknown) {
  const x =
    e instanceof ApiError
      ? e
      : new ApiError(500, "internal_error", "internal error");
  const error: Record<string, string> = { code: x.code, message: x.message };
  if (x.field) error.field = x.field;
  return respond({ error }, x.status);
}
export async function body(req: Request): Promise<Record<string, any>> {
  let v: unknown;
  try {
    v = await req.json();
  } catch {
    throw new ApiError(400, "validation", "body must be JSON");
  }
  if (!v || typeof v !== "object" || Array.isArray(v)) {
    throw new ApiError(400, "validation", "body must be an object");
  }
  return v as Record<string, any>;
}
export function auth(req: Request, s: State): number {
  const m = /^Bearer (.+)$/.exec(req.headers.get("Authorization") || "");
  const id = m ? s.sessions.get(m[1]!) : undefined;
  if (!id) throw new ApiError(401, "unauthorized", "authentication required");
  return id;
}
export function member(s: State, p: Project, uid: number): string | undefined {
  if (p.ownerId === uid) return "owner";
  return s.members.get(p.id)?.get(uid);
}
export function projectAccess(s: State, id: number, uid: number): Project {
  const p = projectOf(s, id);
  if (!p || !member(s, p, uid))
    throw new ApiError(404, "not_found", "project not found");
  return p;
}
export function taskAccess(s: State, key: string, uid: number): Task {
  const t = taskByKey(s, key);
  if (!t) throw new ApiError(404, "not_found", "task not found");
  projectAccess(s, t.projectId, uid);
  return t;
}
export function writer(s: State, t: Task, uid: number): void {
  const p = projectOf(s, t.projectId)!;
  if (!["owner", "member"].includes(member(s, p, uid) || "")) {
    throw new ApiError(403, "forbidden", "write access required");
  }
}
export function owner(s: State, p: Project, uid: number): void {
  if (p.ownerId !== uid)
    throw new ApiError(403, "forbidden", "owner access required");
}
export function etag(t: Task) {
  return { ETag: `"${t.version}"` };
}
export function checkVersion(req: Request, t: Task) {
  const tag = req.headers.get("If-Match");
  if (tag && tag !== `"${t.version}"`) {
    throw new ApiError(412, "precondition_failed", "task version changed");
  }
}
