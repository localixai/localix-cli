import { fail, json, projectAccess, role, type State } from "./model";
import { authenticate, userRoute } from "./users";
import { projectRoute } from "./projects";
import { createTask } from "./tasks-create";
import { readTask } from "./task-read";
import { changeTask } from "./task-change";
import { extra } from "./task-extra";
import { deleteOrStats } from "./task-admin";
import { methods } from "./routes-meta";
export function createRouter(s: State) {
  return async (req: Request): Promise<Response> => {
    const url = new URL(req.url),
      method = req.method,
      path = url.pathname;
    let body: any = {};
    const allowed = methods(path);
    if (allowed && !allowed.includes(method)) {
      const out = fail(405, "method_not_allowed", "Method not allowed");
      out.headers.set("Allow", allowed.join(", "));
      return out;
    }
    if (!allowed) return fail(404, "not_found", "Not found");
    if (req.headers.has("Content-Type")) {
      try {
        body = await req.json();
        if (!body || typeof body !== "object" || Array.isArray(body))
          return fail(400, "validation", "Body must be an object");
      } catch {
        return fail(400, "validation", "Invalid JSON");
      }
    }
    const auth = authenticate(s, req);
    const publicRoute =
      (path === "/users" && method === "POST") ||
      (path === "/sessions" && method === "POST");
    if (!auth && !publicRoute)
      return fail(401, "unauthorized", "Authentication required");
    if (path === "/sessions" && method === "DELETE") {
      const token = req.headers.get("Authorization")?.slice(7);
      if (token) s.sessions.delete(token);
      return new Response(null, { status: 204 });
    }
    if (auth) {
      const win = Math.floor(s.now() / 60000) * 60000;
      let l = s.limits.get(auth.token);
      if (!l || l.start !== win) {
        l = { start: win, count: 0 };
        s.limits.set(auth.token, l);
      }
      if (l.count >= 100) {
        const wait = Math.ceil((win + 60000 - s.now()) / 1000);
        return new Response(
          JSON.stringify({
            error: { code: "rate_limited", message: "Rate limited" },
          }),
          {
            status: 429,
            headers: {
              "Content-Type": "application/json",
              "Retry-After": String(wait),
              "X-RateLimit-Limit": "100",
              "X-RateLimit-Remaining": "0",
            },
          },
        );
      }
      l.count++;
    }
    let response = await userRoute(s, method, path, body);
    if (path === "/me" && method === "GET")
      response = json(safeUser(s, auth!.id));
    if (!response && auth)
      response = projectRoute(s, method, path, auth.id, body);
    const create = path.match(/^\/projects\/(\d+)\/tasks$/);
    if (!response && create && method === "POST" && auth) {
      const key = req.headers.get("Idempotency-Key");
      const id = key ? `${auth.id}:${key}` : "";
      const prior = id ? s.idempotency.get(id) : undefined;
      const fingerprint = JSON.stringify(body);
      if (prior && s.now() - prior.at < 86400000) {
        response =
          prior.fingerprint === fingerprint
            ? json(prior.body, prior.status, {
                "Idempotent-Replayed": "true",
                ETag: `"${prior.body.version}"`,
              })
            : fail(422, "idempotency_mismatch", "Idempotency key reused");
      } else {
        response = createTask(s, auth.id, Number(create[1]), body);
        if (id && response.status === 201) {
          const result = await response.clone().json();
          s.idempotency.set(id, {
            at: s.now(),
            fingerprint,
            body: result,
            status: 201,
          });
        }
      }
    }
    const bulk = path.match(/^\/projects\/(\d+)\/tasks\/bulk$/);
    if (!response && bulk && method === "POST" && auth) {
      if (
        !Array.isArray(body.tasks) ||
        body.tasks.length < 1 ||
        body.tasks.length > 50
      )
        response = fieldErr("tasks");
      else {
        const p = projectAccess(s, auth.id, Number(bulk[1]));
        if (!p) response = fail(404, "not_found", "Project not found");
        else if (!["owner", "member"].includes(role(s, p, auth.id)!))
          response = fail(403, "forbidden", "Cannot create tasks");
        else {
          const old = s.tasks.length;
          const oldId = s.ids.task;
          const answers: any[] = [];
          for (let i = 0; i < body.tasks.length; i++) {
            const r = createTask(s, auth.id, p.id, body.tasks[i], false);
            const b: any = await r.json();
            if (r.status !== 201) {
              s.tasks.splice(old);
              s.ids.task = oldId;
              response = fieldErr(`tasks[${i}].${b.error.field}`);
              break;
            }
            answers.push(b);
          }
          if (!response) response = json({ items: answers }, 201);
        }
      }
    }
    if (!response && auth) response = readTask(s, method, path, auth.id, url);
    if (!response && auth)
      response = changeTask(
        s,
        method,
        path,
        auth.id,
        body,
        req.headers.get("If-Match"),
      );
    if (!response && auth) response = extra(s, method, path, auth.id, body);
    if (!response && auth) response = deleteOrStats(s, method, path, auth.id);
    response ??= fail(404, "not_found", "Not found");
    if (auth) {
      response.headers.set("X-RateLimit-Limit", "100");
      const l = s.limits.get(auth.token)!;
      response.headers.set(
        "X-RateLimit-Remaining",
        String(Math.max(0, 100 - l.count)),
      );
    }
    return response;
  };
}
function safeUser(s: State, id: number) {
  const u = s.users.find((x) => x.id === id);
  return u ? { id: u.id, email: u.email, name: u.name } : null;
}
function fieldErr(field: string) {
  return fail(400, "validation", "Invalid field", field);
}
