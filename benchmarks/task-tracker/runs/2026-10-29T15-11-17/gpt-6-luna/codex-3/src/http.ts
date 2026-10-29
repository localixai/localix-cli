import type { State } from "./model";

export class Fault extends Error {
  constructor(public status: number, public code: string, message: string, public field?: string) {
    super(message);
  }
}
export function fail(status: number, code: string, message: string, field?: string): never {
  throw new Fault(status, code, message, field);
}
export const bad = (field: string): never => fail(400, "validation", `Invalid ${field}`, field);
export function json(body: unknown, status = 200, headers: Record<string, string> = {}) {
  return Response.json(body, { status, headers });
}
export const empty = () => new Response(null, { status: 204 });
export function errorResponse(error: unknown) {
  const e = error instanceof Fault ? error : new Fault(500, "internal_error", "Internal error");
  const detail: Record<string, string> = { code: e.code, message: e.message };
  if (e.field) detail.field = e.field;
  return json({ error: detail }, e.status);
}
export async function body(request: Request): Promise<Record<string, any>> {
  let value: unknown;
  try { value = await request.json(); } catch { fail(400, "validation", "Body must be valid JSON"); }
  if (!value || typeof value !== "object" || Array.isArray(value)) bad("body");
  return value as Record<string, any>;
}
export function auth(request: Request, s: State) {
  const value = request.headers.get("Authorization") ?? "";
  const match = /^Bearer (.+)$/.exec(value);
  const id = match ? s.tokens.get(match[1]!) : undefined;
  if (!id) fail(401, "unauthorized", "Authentication required");
  return id;
}
export function allow(method: string, methods: string[]) {
  return new Response(JSON.stringify({ error: { code: "method_not_allowed", message: "Method not allowed" } }), {
    status: 405, headers: { "Content-Type": "application/json", Allow: methods.join(", ") },
  });
}
export function rate(request: Request, s: State) {
  const bearer = /^Bearer (.+)$/.exec(request.headers.get("Authorization") ?? "");
  if (!bearer || !s.tokens.has(bearer[1]!)) return {};
  const token = bearer[1]!;
  const minute = Math.floor(s.now() / 60000);
  let data = s.limits.get(token);
  if (!data || data.minute !== minute) data = { minute, count: 0 };
  data.count++;
  s.limits.set(token, data);
  const remaining = Math.max(0, 100 - data.count);
  const headers: Record<string, string> = {
    "X-RateLimit-Limit": "100", "X-RateLimit-Remaining": String(remaining),
  };
  if (data.count > 100) {
    const wait = Math.ceil(((minute + 1) * 60000 - s.now()) / 1000);
    const response = json({ error: { code: "rate_limited", message: "Rate limit exceeded" } }, 429,
      { ...headers, "Retry-After": String(wait) });
    throw response;
  }
  return headers;
}
