import type { State } from "./model";

export type Ctx = {
  s: State;
  req: Request;
  user: number | null;
  url: URL;
  body: any;
};
export type Result = {
  status: number;
  body?: unknown;
  headers?: Record<string, string>;
};
export const ok = (
  body: unknown,
  status = 200,
  headers: Record<string, string> = {},
): Result => ({ status, body, headers });
export const empty = (): Result => ({ status: 204 });
export function err(
  status: number,
  code: string,
  message = code,
  field?: string,
): Result {
  const e: Record<string, string> = { code, message };
  if (field !== undefined) e.field = field;
  return { status, body: { error: e } };
}
export const bad = (field?: string) =>
  err(400, "validation", field ? `Invalid ${field}` : "Invalid request", field);
export const denied = () => err(403, "forbidden", "Forbidden");
export const missing = () => err(404, "not_found", "Not found");
export const conflict = (field?: string) =>
  err(409, "conflict", "Conflict", field);
export function object(v: any): v is Record<string, any> {
  return !!v && typeof v === "object" && !Array.isArray(v);
}
export function readBody(raw: string): any {
  try {
    const x = JSON.parse(raw);
    return object(x) ? x : null;
  } catch {
    return null;
  }
}
export function response(result: Result, extra: Record<string, string> = {}) {
  const headers = new Headers(result.headers);
  for (const [k, v] of Object.entries(extra)) headers.set(k, v);
  if (result.status === 204)
    return new Response(null, { status: 204, headers });
  headers.set("Content-Type", "application/json; charset=utf-8");
  return new Response(JSON.stringify(result.body), {
    status: result.status,
    headers,
  });
}
export function member(ctx: Ctx, pid: number) {
  const p = ctx.s.projects.find((x) => x.id === pid);
  return p &&
    ctx.user !== null &&
    (ctx.user === p.ownerId || ctx.s.members.get(pid)?.has(ctx.user))
    ? p
    : undefined;
}
export function canWrite(ctx: Ctx, pid: number) {
  const p = member(ctx, pid);
  return p &&
    (p.ownerId === ctx.user ||
      ctx.s.members.get(pid)?.get(ctx.user!) === "member")
    ? p
    : undefined;
}
export function taskView(t: any) {
  const { history, comments, deps, ...v } = t;
  return v;
}
