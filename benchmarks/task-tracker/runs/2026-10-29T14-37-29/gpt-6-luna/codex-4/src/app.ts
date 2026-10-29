import { createState, type Ctx } from './model';
import { accountRoute } from './accounts';
import { projectRoute } from './projects';
import { taskRoute } from './tasks';
import { statsRoute } from './stats';
import { errorResponse, json } from './http';
export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };
const allowed: Record<string, string[]> = {
  '/users': ['POST'], '/sessions': ['POST', 'DELETE'], '/me': ['GET'], '/projects': ['GET', 'POST'],
};
function methods(path: string) {
  if (path.startsWith('/projects/')) {
    if (/\/tasks(?:\/bulk)?$/.test(path)) return ['GET', 'POST'];
    if (/\/members(?:\/\d+)?$/.test(path)) return ['GET', 'POST', 'DELETE'];
    if (/\/stats$/.test(path) || /^\/projects\/\d+$/.test(path)) return ['GET'];
  }
  if (/^\/tasks\/[^/]+$/.test(path)) return ['GET', 'PATCH', 'DELETE'];
  if (/^\/tasks\/[^/]+\/transitions$/.test(path)) return ['POST'];
  if (/^\/tasks\/[^/]+\/(history|comments|dependencies)$/.test(path)) return ['GET', 'POST'];
  if (/^\/tasks\/[^/]+\/dependencies\/[^/]+$/.test(path)) return ['DELETE'];
  if (/^\/tasks\/[^/]+\/dependencies\/[^/]+$/.test(path)) return ['DELETE'];
  return allowed[path] || [];
}
export function createApp(options: Options = {}): App {
  const c: Ctx = { s: createState(), now: options.now || Date.now };
  return { async fetch(request: Request) {
    const url = new URL(request.url), path = url.pathname, method = request.method;
    const allow = methods(path);
    if (!allow.length) return json({ error: { code: 'not_found', message: 'not found' } }, 404);
    if (!allow.includes(method)) {
      return json({ error: { code: 'method_not_allowed', message: 'method not allowed' } },
        405, { Allow: allow.join(', ') });
    }
    try {
      const token = request.headers.get('Authorization')?.match(/^Bearer (.+)$/)?.[1];
      let rateHeaders: Record<string, string> = {};
      if (token && c.s.sessions.has(token)) {
        const minute = Math.floor(c.now() / 60000), rate = c.s.rates.get(token);
        if (!rate || rate.minute !== minute) c.s.rates.set(token, { minute, count: 0 });
        const cur = c.s.rates.get(token)!;
        if (cur.count >= 100) return json({ error: { code: 'rate_limited', message: 'rate limit exceeded' } }, 429,
          { 'Retry-After': String(Math.ceil(((minute + 1) * 60000 - c.now()) / 1000)),
            'X-RateLimit-Limit': '100', 'X-RateLimit-Remaining': '0' });
        cur.count++; rateHeaders = { 'X-RateLimit-Limit': '100', 'X-RateLimit-Remaining': String(100 - cur.count) };
      }
      const result = await accountRoute(c, request, path) || await projectRoute(c, request, path) ||
        await taskRoute(c, request, path, url) || statsRoute(c, request, path);
      if (!result) return json({ error: { code: 'not_found', message: 'not found' } }, 404);
      for (const [k, v] of Object.entries(rateHeaders)) result.headers.set(k, v);
      return result;
    } catch (e) { return errorResponse(e); }
  } };
}
