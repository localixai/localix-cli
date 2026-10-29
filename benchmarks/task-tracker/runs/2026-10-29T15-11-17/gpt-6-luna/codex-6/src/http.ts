export class Fault extends Error {
  constructor(public status: number, public code: string, message: string,
    public field?: string) { super(message); }
}
export const fail = (status: number, code: string, message: string, field?: string): never => {
  throw new Fault(status, code, message, field);
};
export function ok(body: unknown, status = 200, headers: Record<string, string> = {}) {
  return Response.json(body, { status, headers });
}
export function empty(status = 204, headers: Record<string, string> = {}) {
  return new Response(null, { status, headers });
}
export async function read(request: Request): Promise<Record<string, any>> {
  try {
    const value = await request.json();
    if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error();
    return value as Record<string, any>;
  } catch {
    return fail(400, "validation", "Body must be a JSON object");
  }
}
export function validation(field: string, message = `Invalid ${field}`): never {
  return fail(400, "validation", message, field);
}
export function errorResponse(error: unknown): Response {
  const e = error instanceof Fault ? error : new Fault(500, "internal", "Internal server error");
  const detail: Record<string, string> = { code: e.code, message: e.message };
  if (e.field !== undefined) detail.field = e.field;
  return ok({ error: detail }, e.status);
}
export function stamp(now: () => number) { return new Date(now()).toISOString(); }
export function plainTask(task: any) {
  return { ...task };
}
