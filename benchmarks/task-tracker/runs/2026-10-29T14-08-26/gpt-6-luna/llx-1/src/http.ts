export const json = (body: any, status = 200, headers: Record<string, string> = {}) =>
  new Response(body === null ? null : JSON.stringify(body), {
    status, headers: { "Content-Type": "application/json", ...headers },
  });
export const fail = (status: number, code: string, message = code, field?: string) =>
  json({ error: { code, message, ...(field ? { field } : {}) } }, status);
export const bad = (field: string) => fail(400, "validation", `Invalid ${field}`, field);
export const notFound = () => fail(404, "not_found", "Not found");
export const forbidden = () => fail(403, "forbidden", "Forbidden");
export function bodyObject(value: any): value is Record<string, any> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}
export const stamp = (db: any) => new Date(db.now()).toISOString();
