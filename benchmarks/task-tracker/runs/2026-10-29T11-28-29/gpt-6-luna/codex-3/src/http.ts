import type { State } from "./model";
export class Fault extends Error {
  constructor(public status: number, public code: string, message: string, public field?: string) {
    super(message);
  }
}
export const fail = (status: number, code: string, message: string, field?: string): never => {
  throw new Fault(status, code, message, field);
};
export const bad = (field: string, message = `Invalid ${field}`): never =>
  fail(400, "validation", message, field);
export const missing = (): never => fail(404, "not_found", "Not found");
export const forbidden = (): never => fail(403, "forbidden", "Forbidden");
export const conflict = (code = "conflict", field?: string): never =>
  fail(409, code, code === "conflict" ? "Conflict" : code, field);
export function json(value: unknown, status = 200, headers: Record<string, string> = {}): Response {
  return Response.json(value, { status, headers });
}
export function errorResponse(error: unknown): Response {
  const fault = error instanceof Fault ? error : new Fault(500, "internal", "Internal error");
  const detail: Record<string, string> = { code: fault.code, message: fault.message };
  if (fault.field !== undefined) detail.field = fault.field;
  return json({ error: detail }, fault.status);
}
export async function bodyOf(request: Request): Promise<Record<string, any>> {
  let value: unknown;
  try { value = await request.json(); } catch { return fail(400, "validation", "Body must be valid JSON"); }
  if (!value || Array.isArray(value) || typeof value !== "object") return bad("body");
  return value as Record<string, any>;
}
export function auth(state: State, request: Request, limited = true): number {
  const header = request.headers.get("Authorization") ?? "";
  const token = header.startsWith("Bearer ") ? header.slice(7) : "";
  const userId = state.tokens.get(token);
  if (!userId) return fail(401, "unauthorized", "Unauthorized");
  if (limited) {
    const now = state.now();
    const start = Math.floor(now / 60000) * 60000;
    const hits = (state.hits.get(token) ?? []).filter((time) => time >= start);
    if (hits.length >= 100) {
      const wait = Math.ceil((start + 60000 - now) / 1000);
      const response = json({ error: { code: "rate_limited", message: "Rate limited" } }, 429,
        { "Retry-After": String(wait), "X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "0" });
      throw response;
    }
    hits.push(now);
    state.hits.set(token, hits);
    request.headers.set("X-Internal-Remaining", String(100 - hits.length));
  }
  return userId;
}
export function visibleRole(state: State, projectId: number, userId: number): string | undefined {
  return state.memberships.get(projectId)?.get(userId);
}
export function projectFor(state: State, id: number, userId: number) {
  const project = state.projects.find((item) => item.id === id);
  if (!project || !visibleRole(state, id, userId)) return missing();
  return project;
}
export function taskFor(state: State, key: string, userId: number) {
  const task = state.tasks.find((item) => item.key === key);
  if (!task || !visibleRole(state, task.projectId, userId)) return missing();
  return task;
}
export function member(state: State, projectId: number, userId: number) {
  const role = visibleRole(state, projectId, userId);
  if (!role) return missing();
  if (role === "viewer") forbidden();
  return role;
}
export const iso = (state: State) => new Date(state.now()).toISOString();
export const publicTask = (task: any) => {
  const { dependsOn, comments, history, ...value } = task;
  return value;
};
