export class Fault extends Error {
  constructor(public status: number, public code: string, message: string, public field?: string,
    public allow?: string) {
    super(message);
  }
}
export function fail(status: number, code: string, message: string, field?: string): never {
  throw new Fault(status, code, message, field);
}
export const json = (body: unknown, status = 200, headers: Record<string, string> = {}) =>
  Response.json(body, { status, headers });
export const err = (e: unknown) => {
  const f = e instanceof Fault ? e : new Fault(500, "internal", "Internal error");
  const error: Record<string, string> = { code: f.code, message: f.message };
  if (f.field !== undefined) error.field = f.field;
  return json({ error }, f.status, f.allow ? { Allow: f.allow } : {});
};
export const object = (x: unknown): Record<string, any> => {
  if (!x || typeof x !== "object" || Array.isArray(x)) fail(400, "validation", "Expected an object");
  return x as Record<string, any>;
};
