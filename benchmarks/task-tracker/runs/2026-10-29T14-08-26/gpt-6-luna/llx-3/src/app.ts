import { newState, publicProject, publicTask, publicUser, type State, type Task } from "./model";
import { bodyOf, errorResponse, fail, response, validate } from "./http";
import { route } from "./routes";

export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };
export function createApp(options: Options = {}): App {
  const s = newState();
  const now = options.now || Date.now;
  return { fetch: async (req) => {
    try {
      const url = new URL(req.url);
      const token = req.headers.get("Authorization")?.match(/^Bearer (.+)$/)?.[1];
      const userId = token ? s.sessions.get(token) : undefined;
      const reply = await route({ req, url, s, now, token, userId });
      const headers = new Headers(reply.headers);
      if (token && userId) {
        const minute = Math.floor(now() / 60000);
        let rate = s.rates.get(token);
        if (!rate || rate.minute !== minute) rate = { minute, count: 0 };
        if (rate.count >= 100) {
          headers.set("Retry-After", String(Math.max(1, Math.ceil((60000 - now() % 60000) / 1000))));
          headers.set("X-RateLimit-Limit", "100");
          headers.set("X-RateLimit-Remaining", "0");
          return response({ error: { code: "rate_limited", message: "Rate limit exceeded" } }, 429,
            Object.fromEntries(headers));
        }
        rate.count++;
        s.rates.set(token, rate);
        headers.set("X-RateLimit-Limit", "100");
        headers.set("X-RateLimit-Remaining", String(100 - rate.count));
      }
      return new Response(reply.body, { status: reply.status, headers });
    } catch (e) { return errorResponse(e); }
  }};
}
export { publicProject, publicTask, publicUser, bodyOf, fail, validate, response };
