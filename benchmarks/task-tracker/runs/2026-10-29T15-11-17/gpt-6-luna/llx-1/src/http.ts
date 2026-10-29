import type { Ctx } from "./model";

export type Result = { status: number; body?: unknown; headers?: Record<string, string> };
export const ok = (body: unknown, status = 200, headers: Record<string, string> = {}): Result =>
  ({ status, body, headers });
export const empty = (status = 204): Result => ({ status });
export const err = (status: number, code: string, message = code, field?: string): Result =>
  ({ status, body: { error: { code, message, ...(field ? { field } : {}) } } });
export const bad = (field: string, message = `Invalid ${field}`) => err(400, "validation", message, field);
export const missing = () => err(404, "not_found", "Not found");
export const denied = () => err(403, "forbidden", "Forbidden");
export const conflict = (field?: string) => err(409, "conflict", "Conflict", field);
export const object = (x: unknown): x is Record<string, any> =>
  !!x && typeof x === "object" && !Array.isArray(x);
export async function body(req: Request): Promise<Record<string, any> | null> {
  try { const v: unknown = await req.json(); return object(v) ? v : null; }
  catch { return null; }
}
export type RequestCtx = Ctx & { req: Request; userId: number; url: URL };
export function auth(ctx: Ctx, req: Request): number | null {
  const h = req.headers.get("Authorization") ?? "";
  if (!h.startsWith("Bearer ")) return null;
  return ctx.state.tokens.get(h.slice(7)) ?? null;
}
