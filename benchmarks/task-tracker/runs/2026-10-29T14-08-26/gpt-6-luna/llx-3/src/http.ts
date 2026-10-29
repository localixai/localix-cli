export type Err = { status: number; code: string; message: string; field?: string };
export const fail = (status: number, code: string, message = code, field?: string): never => {
  throw { status, code, message, field } satisfies Err;
};
export const validate = (ok: unknown, field: string, message = `Invalid ${field}`): void => {
  if (!ok) fail(400, "validation", message, field);
};
export const obj = (v: any): Record<string, any> => {
  validate(v !== null && typeof v === "object" && !Array.isArray(v), "body");
  return v;
};
export const response = (value: any, status = 200, headers: Record<string, string> = {}) =>
  new Response(status === 204 ? null : JSON.stringify(value), {
    status, headers: { ...(status === 204 ? {} : { "Content-Type": "application/json" }), ...headers },
  });
export const errorResponse = (e: any) => response({
  error: { code: e.code || "internal_error", message: e.message || "Internal error",
    ...(e.field ? { field: e.field } : {}) },
}, e.status || 500, e.headers || {});
export const bodyOf = async (r: Request) => {
  try { return obj(await r.json()); } catch (e: any) {
    if (e?.status) throw e;
    fail(400, "validation", "Invalid JSON");
  }
};
