export class Fault extends Error {
  constructor(public status: number, public code: string, message: string, public field?: string) {
    super(message);
  }
}
export function fail(status: number, code: string, message: string, field?: string): never {
  throw new Fault(status, code, message, field);
}
export const bad = (field: string, message = `Invalid ${field}`): never =>
  fail(400, "validation", message, field);
export const obj = async (request: Request): Promise<Record<string, any>> => {
  try {
    const value = await request.json();
    if (!value || typeof value !== "object" || Array.isArray(value)) bad("");
    return value as Record<string, any>;
  } catch (error) {
    if (error instanceof Fault) throw error;
    return bad("", "Request body must be a JSON object");
  }
};
export const reply = (body: unknown, status = 200, headers: Record<string, string> = {}) =>
  new Response(body === null ? null : JSON.stringify(body), {
    status, headers: body === null ? headers : { "Content-Type": "application/json", ...headers },
  });
export const errorReply = (error: unknown) => {
  const e = error instanceof Fault ? error : new Fault(500, "internal", "Internal error");
  const body: Record<string, unknown> = { code: e.code, message: e.message };
  if (e.field) body.field = e.field;
  return reply({ error: body }, e.status);
};
export const text = (x: unknown) => typeof x === "string" ? x : "";
