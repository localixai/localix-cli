import type { Store } from "./model";

export class Fault extends Error {
  constructor(public status: number, public code: string, message: string, public field?: string) {
    super(message);
  }
}
export function fail(status: number, code: string, message = code, field?: string): never {
  throw new Fault(status, code, message, field);
}
export const ok = (body: any, status = 200, headers?: Record<string, string>) =>
  new Response(status === 204 ? null : JSON.stringify(body), {
    status, headers: { "Content-Type": "application/json", ...headers },
  });
export function errorResponse(e: unknown) {
  const f = e instanceof Fault ? e : new Fault(500, "internal_error", "internal error");
  const error: any = { code: f.code, message: f.message };
  if (f.field) error.field = f.field;
  return ok({ error }, f.status);
}
export function readBody(req: Request): Promise<any> {
  return req.text().then((text) => {
    let value: any;
    try { value = JSON.parse(text); } catch { fail(400, "validation", "body must be JSON"); }
    if (!value || typeof value !== "object" || Array.isArray(value)) {
      fail(400, "validation", "body must be an object");
    }
    return value;
  });
}
export function auth(req: Request, s: Store): { id: number; token: string } {
  const m = req.headers.get("Authorization")?.match(/^Bearer (.+)$/);
  const id = m && s.sessions.get(m[1]!);
  if (!id) fail(401, "unauthorized", "unauthorized");
  return { id, token: m![1]! };
}
export const required = (value: any, field: string, check: (v: any) => boolean): any => {
  if (!check(value)) fail(400, "validation", `invalid ${field}`, field);
  return value;
};
