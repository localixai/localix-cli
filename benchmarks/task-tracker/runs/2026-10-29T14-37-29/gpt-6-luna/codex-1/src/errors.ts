export class ApiError extends Error {
  constructor(public status: number, public code: string, message: string, public field?: string) {
    super(message);
  }
}
export function fail(status: number, code: string, message: string, field?: string): never {
  throw new ApiError(status, code, message, field);
}
export function response(body: unknown, status = 200, headers: Record<string, string> = {}): Response {
  return new Response(status === 204 ? null : JSON.stringify(body), {
    status, headers: { ...headers, ...(status === 204 ? {} : { "Content-Type": "application/json" }) },
  });
}
export const errorResponse = (e: unknown) => {
  const err = e instanceof ApiError ? e : new ApiError(500, "internal", "Internal error");
  const detail: Record<string, string> = { code: err.code, message: err.message };
  if (err.field) detail.field = err.field;
  return response({ error: detail }, err.status);
};
export async function bodyOf(request: Request): Promise<Record<string, any>> {
  let value: unknown;
  try { value = await request.json(); } catch { fail(400, "validation", "Body must be JSON"); }
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    fail(400, "validation", "Body must be an object");
  }
  return value as Record<string, any>;
}
export function invalid(field: string): never {
  return fail(400, "validation", `Invalid ${field}`, field);
}
