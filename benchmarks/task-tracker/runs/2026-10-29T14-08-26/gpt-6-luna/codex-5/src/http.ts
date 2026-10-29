export class HttpError extends Error {
  constructor(
    public status: number,
    public code: string,
    message: string,
    public field?: string,
  ) {
    super(message);
  }
}
export const fail = (
  status: number,
  code: string,
  message: string,
  field?: string,
): never => {
  throw new HttpError(status, code, message, field);
};
export const bad = (field: string): never =>
  fail(400, "validation", `Invalid ${field}`, field);
export const obj = (v: unknown): Record<string, any> => {
  if (!v || typeof v !== "object" || Array.isArray(v)) bad("body");
  return v as Record<string, any>;
};
export const validText = (v: any, field: string, max: number): string => {
  if (typeof v !== "string" || !v.trim() || v.trim().length > max) bad(field);
  return v.trim();
};
export const errorResponse = (e: unknown): Response => {
  const x =
    e instanceof HttpError
      ? e
      : new HttpError(500, "internal", "Internal error");
  const error: Record<string, string> = { code: x.code, message: x.message };
  if (x.field) error.field = x.field;
  return Response.json({ error }, { status: x.status });
};
export const json = (
  v: unknown,
  status = 200,
  headers: Record<string, string> = {},
) => Response.json(v, { status, headers });
export const empty = () => new Response(null, { status: 204 });
