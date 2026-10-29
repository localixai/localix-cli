import type { State } from './model';
import { accountRoute, projectRoute } from './accounts';
import { taskRoute } from './tasks';
import { fail, missing } from './http';
export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };
const methods: Record<string, string> = {
  '/users': 'POST', '/sessions': 'POST, DELETE', '/me': 'GET', '/projects': 'GET, POST',
};
export function createApp(options: Options = {}): App {
  const s: State = { users: [], projects: [], members: new Map(), tasks: [], histories: new Map(),
    comments: new Map(), deps: new Map(), tokens: new Map(), idempotency: new Map(), limits: new Map(),
    ids: { user: 0, project: 0, task: 0, comment: 0 }, now: options.now ?? Date.now };
  return { async fetch(request) {
    const url = new URL(request.url), path = url.pathname, method = request.method;
    const allow = methods[path] ?? (path.match(/^\/projects\/[^/]+$/) ? 'GET' : undefined);
    if (allow && !allow.split(', ').includes(method)) {
      return response(fail(405, 'method_not_allowed', 'Method not allowed'), { Allow: allow });
    }
    const auth = request.headers.get('Authorization') ?? '';
    const token = auth.startsWith('Bearer ') ? auth.slice(7) : undefined;
    let user = token ? s.tokens.get(token) : undefined;
    if (token && user !== undefined) {
      const minute = Math.floor(s.now() / 60000), current = s.limits.get(token);
      const limit = current?.minute === minute ? current : { minute, count: 0 };
      if (limit.count >= 100) {
        const retry = Math.ceil(((minute + 1) * 60000 - s.now()) / 1000);
        return response(fail(429, 'rate_limited', 'Rate limit exceeded'), {
          'X-RateLimit-Limit': '100', 'X-RateLimit-Remaining': '0', 'Retry-After': String(retry) });
      }
      limit.count++; s.limits.set(token, limit);
    }
    if (token && user === undefined && path !== '/sessions') {
      return response(fail(401, 'unauthorized', 'Sign in required'));
    }
    if (method === 'DELETE' && path === '/sessions') {
      if (!token || user === undefined) return response(fail(401, 'unauthorized', 'Sign in required'));
      s.tokens.delete(token); return new Response(null, { status: 204 });
    }
    let body: any = {};
    if (!['GET', 'HEAD', 'DELETE'].includes(method) && request.body) {
      try { body = await request.json(); } catch { return response(fail(400, 'validation', 'Invalid JSON')); }
      if (!body || typeof body !== 'object' || Array.isArray(body)) {
        return response(fail(400, 'validation', 'Body must be an object'));
      }
    }
    const result = accountRoute(s, method, path, user, body, token) ??
      projectRoute(s, method, path, user, body) ?? taskRoute(s, method, path, url, user, body, request.headers);
    if (!result) return response(missing());
    const rate: Record<string, string> = token && user !== undefined ? {
      'X-RateLimit-Limit': '100',
      'X-RateLimit-Remaining': String(100 - (s.limits.get(token)?.count ?? 0)) } : {};
    return response(result, rate);
  } };
}
function response(result: any, headers: Record<string, string> = {}) {
  const all = new Headers({ ...headers, ...(result.headers ?? {}) });
  if (result.status === 204) return new Response(null, { status: 204, headers: all });
  return Response.json(result.body ?? null, { status: result.status, headers: all });
}
