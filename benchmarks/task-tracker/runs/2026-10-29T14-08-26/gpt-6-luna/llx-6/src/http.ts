export class Fault extends Error {
  constructor(public status: number, public code: string, message: string, public field?: string) {
    super(message);
  }
}
export const fail = (status: number, code: string, message = code, field?: string): never => {
  throw new Fault(status, code, message, field);
};
export const check = (ok: unknown, field: string, message = `invalid ${field}`): void => {
  if (!ok) fail(400, "validation", message, field);
};
export function response(body: unknown, status = 200, headers: Record<string, string> = {}) {
  return body === null ? new Response(null, { status, headers }) :
    Response.json(body, { status, headers });
}
export function errorResponse(error: unknown) {
  const e = error instanceof Fault ? error : new Fault(500, "internal", "internal error");
  const detail: Record<string, string> = { code: e.code, message: e.message };
  if (e.field) detail.field = e.field;
  return response({ error: detail }, e.status);
}
export async function readBody(req: Request): Promise<Record<string, any>> {
  let value: unknown;
  try { value = await req.json(); } catch { fail(400, "validation", "invalid JSON"); }
  check(value !== null && typeof value === "object" && !Array.isArray(value), "body");
  return value as Record<string, any>;
}
