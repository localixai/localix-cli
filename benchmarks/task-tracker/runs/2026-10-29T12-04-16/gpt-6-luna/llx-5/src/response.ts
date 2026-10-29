import { err, type Ctx } from "./model";
export function json(body: unknown, status = 200, headers: Record<string, string> = {}) {
  return Response.json(body, { status, headers });
}
export function failure(status: number, code: string, message?: string, field?: string) {
  const e = err(status, code, message, field);
  return json(e.body, status);
}
export function readBody(req: Request) {
  return req.text().then(text => {
    if (!text) return {};
    try {
      const value = JSON.parse(text);
      if (!value || typeof value !== "object" || Array.isArray(value))
        throw Object.assign(Error("Invalid JSON body"), { status: 400, code: "validation" });
      return value;
    } catch (e) {
      if ((e as any).status) throw e;
      throw Object.assign(Error("Invalid JSON body"), { status: 400, code: "validation" });
    }
  });
}
export function auth(ctx: Ctx) {
  const token = ctx.req.headers.get("Authorization")?.match(/^Bearer (.+)$/)?.[1];
  const id = token ? ctx.s.tokens.get(token) : undefined;
  if (!id) throw Object.assign(Error("Unauthorized"), { status: 401, code: "unauthorized" });
  ctx.user = ctx.s.users.find(u => u.id === id)!;
  const minute = Math.floor(ctx.s.now() / 60000);
  let bucket = ctx.s.limits.get(token!);
  if (!bucket || bucket.minute !== minute) bucket = { minute, count: 0 };
  bucket.count++; ctx.s.limits.set(token!, bucket);
  if (bucket.count > 100) throw Object.assign(Error("Rate limited"), {
    status: 429, code: "rate_limited", retry: Math.ceil((60000 - ctx.s.now() % 60000) / 1000),
  });
  return token!;
}
export function failureFor(e: any) {
  return failure(e.status || (e.field ? 400 : 500), e.code || (e.field ? "validation" : "internal"),
    e.message || "Internal error", e.field);
}
