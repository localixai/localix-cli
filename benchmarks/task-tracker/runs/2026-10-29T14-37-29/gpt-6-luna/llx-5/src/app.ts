import { state } from "./model";
import { clean, err, makeTask, member, ok, task, validTask } from "./util";
import { taskRoutes } from "./taskroutes";
export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };

export function createApp(options: Options = {}): App {
  const s = state(), clock = options.now ?? Date.now;
  const fetch = async (req: Request): Promise<Response> => {
    const u = new URL(req.url), path = u.pathname.split("/").filter(Boolean), m = req.method;
    let b: any = {};
    if (["POST", "PATCH"].includes(m) && req.body) {
      try { b = await req.json(); } catch { return err("validation", 400); }
      if (!b || typeof b !== "object" || Array.isArray(b)) return err("validation", 400);
    }
    const auth = req.headers.get("Authorization") ?? "", token = auth.startsWith("Bearer ") ? auth.slice(7) : "";
    const uid = s.tokens.get(token), me = s.users.find(x => x.id === uid), now = new Date(clock()).toISOString();
    const respond = (x: any, status = 200, etag?: string) => ok(x, status, etag ? { ETag: `"${etag}"` } : {});
    if (path[0] === "users" && path.length === 1) {
      if (m !== "POST") return method("POST");
      const email = typeof b.email === "string" ? b.email.trim().toLowerCase() : "";
      const name = typeof b.name === "string" ? b.name.trim() : "";
      if (!email.includes("@") || !email.split("@")[1]?.includes(".")) return err("validation", 400, "email");
      if (typeof b.password !== "string" || b.password.length < 8) return err("validation", 400, "password");
      if (!name || name.length > 100) return err("validation", 400, "name");
      if (s.users.some(x => x.email === email)) return err("conflict", 409, "email");
      const x = { id: ++s.userIds, email, password: b.password, name }; s.users.push(x);
      return respond({ id: x.id, email, name }, 201);
    }
    if (path[0] === "projects" && path.length === 1 && !["GET", "POST"].includes(m))
      return method("GET, POST");
    if (path[0] === "sessions" && path.length === 1) {
      if (m === "DELETE") {
        if (!me) return err("unauthorized", 401);
        s.tokens.delete(token);
        return new Response(null, { status: 204 });
      }
      if (m !== "POST") return err("method_not_allowed", 405);
      const x = s.users.find(x => x.email === String(b.email).toLowerCase() && x.password === b.password);
      if (!x) return err("unauthorized", 401);
      const t = `${x.id}-${crypto.randomUUID()}`; s.tokens.set(t, x.id); return respond({ token: t }, 201);
    }
    if (path[0] === "me" && m !== "GET") return method("GET");
    if (path[0] === "users" && m !== "POST") return method("POST");
    if (!["projects", "tasks", "me", "sessions"].includes(path[0] ?? "")) return err("not_found", 404);
    if (!me) return err("unauthorized", 401);
    if (token) {
      const minute = Math.floor(clock() / 60000), q = s.limits.get(token);
      const count = q?.minute === minute ? q.count : 0;
      if (count >= 100) return ok({ error: { code: "rate_limited", message: "rate limited" } }, 429,
        { "Retry-After": String(Math.ceil((60000 - clock() % 60000) / 1000)),
          "X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "0" });
      s.limits.set(token, { minute, count: count + 1 });
      const result = path[0] === "me" ? respond({ id: me.id, email: me.email, name: me.name })
        : await route(s, path, m, b, me.id, req, now, respond);
      result.headers.set("X-RateLimit-Limit", "100");
      result.headers.set("X-RateLimit-Remaining", String(99 - count));
      return result;
    }
    if (path[0] === "me" && m === "GET") return respond({ id: me.id, email: me.email, name: me.name });
    return route(s, path, m, b, me.id, req, now, respond);
  };
  return { fetch };
}

function method(allow: string): Response {
  return Response.json({ error: { code: "method_not_allowed", message: "method not allowed" } },
    { status: 405, headers: { Allow: allow } });
}

function route(s: ReturnType<typeof state>, p: string[], m: string, b: any, uid: number,
  req: Request, now: string, out: (x: any, n?: number, e?: string) => Response): Response {
  const no = () => err("not_found", 404), project = (id: number) => s.projects.find(x => x.id === id);
  if (p[0] === "projects" && p.length === 1) {
    if (m === "GET") return out(s.projects.filter(x => member(s, x.id, uid)).sort((a,b)=>a.id-b.id));
    if (m !== "POST") return err("method_not_allowed",405);
    const name = typeof b.name === "string" ? b.name.trim() : "";
    if (!name || name.length > 100) return err("validation",400,"name");
    if (typeof b.key !== "string" || !/^[A-Z]{2,6}$/.test(b.key)) return err("validation",400,"key");
    if (s.projects.some(x=>x.key===b.key)) return err("conflict",409,"key");
    const x = { id: ++s.projectIds, name, key: b.key, ownerId: uid, createdAt: now };
    s.projects.push(x);
    s.members.set(x.id,new Map([[uid,"owner"]])); return out(x,201);
  }
  if (p[0] === "projects" && p[1]) {
    const id = /^\d+$/.test(p[1]) ? Number(p[1]) : -1, pr=project(id);
    if (!pr || !member(s,id,uid)) return no();
    if (p.length===2 && m==="GET") return out(pr);
    if (p[2]==="members") {
      if(p.length===3&&m==="GET") return out([...s.members.get(id)!.entries()]
        .sort((a,b)=>a[0]-b[0]).map(([userId,role])=>({userId,role})));
      if(p.length===3&&m==="POST") {
        if(pr.ownerId!==uid)return err("forbidden",403);
        if(!s.users.some(x=>x.id===b.userId))return err("validation",400,"userId");
        if(!["member","viewer"].includes(b.role))return err("validation",400,"role");
        if(member(s,id,b.userId))return err("conflict",409);
        s.members.get(id)!.set(b.userId,b.role); return out({userId:b.userId,role:b.role},201);
      }
      if(p.length===4&&m==="DELETE") {
        if(pr.ownerId!==uid)return err("forbidden",403);
        const n=Number(p[3]); if(!member(s,id,n)||n===uid)return no();
        s.members.get(id)!.delete(n);
        s.tasks.filter(t => t.projectId === id && t.assigneeId === n)
          .forEach(t => t.assigneeId = null);
        return new Response(null,{status:204});
      }
    }
  }
  return taskRoutes(s,p,m,b,uid,req,now,out);
}
