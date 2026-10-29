import type { State, User } from "./state";

export const json = (body: unknown, status = 200, headers: Record<string, string> = {}) =>
  Response.json(body, { status, headers });
export function error(code: string, message: string, status: number, field?: string) {
  const item: Record<string, string> = { code, message };
  if (field !== undefined) item.field = field;
  return json({ error: item }, status);
}
export const invalid = (field: string, message = `Invalid ${field}`) =>
  error("validation", message, 400, field);
export async function body(request: Request): Promise<any> {
  try {
    const value = await request.json();
    return value && typeof value === "object" && !Array.isArray(value) ? value : null;
  } catch { return null; }
}
export function auth(request: Request, state: State): User | Response {
  const header = request.headers.get("Authorization") ?? "";
  const token = header.startsWith("Bearer ") ? header.slice(7) : "";
  const id = state.sessions.get(token);
  return id ? state.users.find((user) => user.id === id)! : error("unauthorized", "Authentication required", 401);
}
export function isError(value: User | Response): value is Response { return value instanceof Response; }
export function role(state: State, project: number, user: number): string | undefined {
  return state.members.get(project)?.get(user);
}
export const member = (state: State, project: number, user: number) =>
  role(state, project, user) !== undefined;
export function projectAccess(state: State, id: number, user: number) {
  const p = state.projects.find((item) => item.id === id);
  return p && member(state, id, user) ? p : undefined;
}
export function taskFor(state: State, key: string) {
  return state.tasks.find((task) => task.key === key && !task.deleted);
}
export function taskAccess(state: State, task: NonNullable<ReturnType<typeof taskFor>>, user: number) {
  return member(state, task.projectId, user);
}
export function validTitle(value: any) {
  return typeof value === "string" && value.trim().length > 0 && value.trim().length <= 200;
}
export function taskFields(input: any, user: number, state: State, project: number) {
  if (!validTitle(input.title)) return "title";
  if (input.description !== undefined && typeof input.description !== "string") return "description";
  if (input.priority !== undefined && !["low", "normal", "high"].includes(input.priority)) return "priority";
  if (input.labels !== undefined && (!Array.isArray(input.labels) || input.labels.some((x: any) =>
    typeof x !== "string" || !x.trim() || x.trim().length > 30 || /\s/.test(x.trim())))) return "labels";
  if (input.assigneeId !== undefined && input.assigneeId !== null &&
    !member(state, project, input.assigneeId)) return "assigneeId";
  return undefined;
}
export function normalizedLabels(labels: string[] = []) {
  return [...new Set(labels.map((label) => label.trim().toLowerCase()))].sort();
}
