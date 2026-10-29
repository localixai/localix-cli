import { dispatch } from "./routes";
import { Store } from "./store";
import { err, fail, json, object } from "./http";

export async function handle(req: Request, db: Store): Promise<Response> {
  try {
    const url = new URL(req.url), path = url.pathname, method = req.method;
    const body = ["POST", "PATCH", "PUT"].includes(method) ? object(await readBody(req)) : {};
    const tokenHeader = req.headers.get("Authorization") ?? "";
    const token = tokenHeader.startsWith("Bearer ") ? tokenHeader.slice(7) : "";
    const uid = token ? db.tokens.get(token) : undefined;
    let rate: Record<string, string> = {};
    if (uid !== undefined) {
      const min = Math.floor(db.now() / 60000), old = db.limits.get(token);
      const count = old?.minute === min ? old.count : 0;
      rate = { "X-RateLimit-Limit": "100", "X-RateLimit-Remaining": String(Math.max(0, 99-count)) };
      if (count >= 100) {
        rate["Retry-After"] = String(Math.ceil((60000-db.now()%60000)/1000));
        return json({ error: { code: "rate_limited", message: "Rate limit exceeded" } }, 429, rate);
      }
      db.limits.set(token, { minute: min, count: count+1 });
    }
    const result = await dispatch({ db, path, method, query: url.searchParams, body,
      uid, token, headers: req.headers });
    const headers = { ...rate, ...(result.headers ?? {}) };
    return result.status === 204 ? new Response(null, { status: 204, headers }) :
      json(result.body, result.status, headers);
  } catch (e) { return err(e); }
}
async function readBody(req: Request) {
  const text = await req.text();
  if (!text) return {};
  try { return JSON.parse(text); } catch { fail(400, "validation", "Invalid JSON"); }
}
