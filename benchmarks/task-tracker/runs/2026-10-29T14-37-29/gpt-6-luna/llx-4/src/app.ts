import { handleTasks } from "./tasks";

export type Options = {
  /** The current time in milliseconds since the epoch; `Date.now` when not given. */
  now?: () => number;
};

export type App = {
  fetch(request: Request): Response | Promise<Response>;
};

export function createApp(options: Options = {}): App {
  const now = options.now ?? Date.now;
  const users: any[] = [];
  const projects: any[] = [];
  const tasks: any[] = [];
  const sessions = new Map<string, number>();
  const idempotency = new Map<string, { at: number; body: string; result: any }>();
  const limits = new Map<string, { minute: number; count: number }>();
  let sequence = 0;
  const response = (body: unknown, status = 200, headers?: Record<string, string>) =>
    new Response(body === null ? null : JSON.stringify(body), {
      status,
      headers: { "Content-Type": "application/json", ...Object.fromEntries(new Headers(headers)) },
    });
  const error = (status: number, code: string, message: string, field?: string) =>
    response({ error: { code, message, ...(field ? { field } : {}) } }, status);
  return {
    async fetch(request) {
      const url = new URL(request.url);
      const path = url.pathname.split("/").filter(Boolean);
      const method = request.method;
      let body: any = {};
      if (request.body) {
        try {
          body = await request.json();
          if (!body || typeof body !== "object" || Array.isArray(body))
            return error(400, "validation", "Invalid body");
        } catch {
          return error(400, "validation", "Invalid JSON");
        }
      }
      if ((path[0] === "users" && path.length === 1 && method !== "POST") ||
        (path[0] === "me" && method !== "GET")) {
        const allowed = path[0] === "users" ? "POST" : "GET";
        return new Response(JSON.stringify({ error: { code: "method_not_allowed",
          message: "Method not allowed" } }), { status: 405, headers: { Allow: allowed } });
      }
      if (path[0] === "projects" && path.length === 1 && !["GET", "POST"].includes(method))
        return new Response(JSON.stringify({ error: { code: "method_not_allowed",
          message: "Method not allowed" } }), { status: 405, headers: { Allow: "GET, POST" } });
      const token = request.headers.get("Authorization")?.replace(/^Bearer /, "");
      const userId = token ? sessions.get(token) : undefined;
      const user = users.find((item) => item.id === userId);
      if (path[0] === "projects" && path[2] === "stats" && method === "GET") {
        if (!user) return error(401, "unauthorized", "Sign in required");
        const p = projects.find((item) => item.id === Number(path[1]));
        if (!p || (p.ownerId !== userId && !p.members.some((m: any) => m.userId === userId)))
          return error(404, "not_found", "Project not found");
        const rows = tasks.filter((t) => t.projectId === p.id);
        const count = (field: string, values: string[]) =>
          Object.fromEntries(values.map((v) => [v, rows.filter((t) => t[field] === v).length]));
        const assignees = new Map<number, number>();
        rows.forEach((t) => t.assigneeId && assignees.set(t.assigneeId, (assignees.get(t.assigneeId) ?? 0) + 1));
        return response({ total: rows.length, byStatus: count("status", ["open", "in_progress", "done"]),
          byPriority: count("priority", ["high", "normal", "low"]),
          byAssignee: [...assignees].map(([userId, count]) => ({ userId, count }))
            .sort((a, b) => b.count - a.count || a.userId - b.userId) });
      }
      const auth = () => user ?? null;
      const membership = (project: any) => project?.ownerId === userId
        ? "owner" : project?.members.find((m: any) => m.userId === userId)?.role;
      if (path[0] === "users" && path.length === 1 && method === "POST") {
        const email = typeof body.email === "string" ? body.email.trim().toLowerCase() : "";
        const name = typeof body.name === "string" ? body.name.trim() : "";
        if (!email.includes("@") || !email.split("@")[1]?.includes("."))
          return error(400, "validation", "Invalid email", "email");
        if (typeof body.password !== "string" || body.password.length < 8)
          return error(400, "validation", "Password is too short", "password");
        if (!name || name.length > 100) return error(400, "validation", "Invalid name", "name");
        if (users.some((u) => u.email === email))
          return response({ error: { code: "conflict", message: "Email already exists", field: "email" } }, 409);
        const created = { id: users.length + 1, email, name };
        users.push({ ...created, password: body.password });
        return response(created, 201);
      }
      if (path[0] === "sessions" && method === "POST") {
        const found = users.find((u) => u.email === String(body.email).toLowerCase());
        if (!found || found.password !== body.password)
          return error(401, "unauthorized", "Invalid credentials");
        const value = `token-${++sequence}`;
        sessions.set(value, found.id);
        return response({ token: value }, 201);
      }
      if (path[0] === "sessions" && method === "DELETE")
        return token ? (sessions.delete(token), response(null, 204)) : error(401, "unauthorized", "Sign in required");
      if (path[0] === "me" && method === "GET") {
        if (!auth()) return error(401, "unauthorized", "Sign in required");
        const minute = Math.floor(now() / 60_000);
        const state = limits.get(token!) ?? { minute, count: 0 };
        if (state.minute !== minute) {
          state.minute = minute;
          state.count = 0;
        }
        if (state.count >= 100) {
          const wait = Math.ceil(((minute + 1) * 60_000 - now()) / 1000);
          return new Response(JSON.stringify({ error: { code: "rate_limited", message: "rate_limited" } }),
            { status: 429, headers: { "Content-Type": "application/json", "Retry-After": String(wait),
              "X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "0" } });
        }
        state.count++;
        limits.set(token!, state);
        return response({ id: user.id, email: user.email, name: user.name }, 200,
          { "X-RateLimit-Limit": "100", "X-RateLimit-Remaining": String(100 - state.count) });
      }
      if (path[0] === "projects" && path.length === 1 && method === "POST") {
        if (!auth()) return error(401, "unauthorized", "Sign in required");
        const key = typeof body.key === "string" ? body.key.trim() : "";
        const name = typeof body.name === "string" ? body.name.trim() : "";
        if (!/^[A-Z]{2,6}$/.test(key)) return error(400, "validation", "Invalid key", "key");
        if (!name || name.length > 100) return error(400, "validation", "Invalid name", "name");
        if (projects.some((p) => p.key === key))
          return response({ error: { code: "conflict", message: "Key exists", field: "key" } }, 409);
        const item = { id: projects.length + 1, key, name, ownerId: userId,
          createdAt: new Date(now()).toISOString(), members: [] };
        projects.push(item);
        const { members: _, ...result } = item;
        return response(result, 201);
      }
      if (path[0] === "projects") {
        if (!auth()) return error(401, "unauthorized", "Sign in required");
        const project = /^\d+$/.test(path[1] ?? "")
          ? projects.find((p) => p.id === Number(path[1]))
          : projects.find((p) => p.key === path[1]);
        if (path.length === 1 && method === "GET") return response(projects.filter((p) =>
          p.ownerId === userId || p.members.some((m: any) => m.userId === userId))
          .map(({ members: _, ...p }) => p));
        if (!project || !membership(project)) return error(404, "not_found", "Project not found");
        if (path.length === 2 && method === "GET") {
          const { members: _, ...result } = project;
          return response(result);
        }
        if (path[2] === "members" && method === "POST") {
          if (project.ownerId !== userId) return error(403, "forbidden", "Owner required");
          if (!["member", "viewer"].includes(body.role)) return error(400, "validation", "Invalid role", "role");
          if (!users.some((u) => u.id === body.userId)) return error(400, "validation", "User not found", "userId");
          if (project.members.some((m: any) => m.userId === body.userId) || project.ownerId === body.userId)
            return error(409, "conflict", "Already a member");
          const item = { userId: body.userId, role: body.role };
          project.members.push(item);
          return response(item, 201);
        }
        if (path[2] === "members" && method === "GET")
          return response([{ userId: project.ownerId, role: "owner" },
            ...project.members.slice().sort((a: any, b: any) =>
              ({ member: 0, viewer: 1 } as any)[a.role] - ({ member: 0, viewer: 1 } as any)[b.role])]);
        if (path[2] === "members" && path[3] && method === "DELETE") {
          if (project.ownerId !== userId) return error(403, "forbidden", "Owner required");
          const index = project.members.findIndex((m: any) => m.userId === Number(path[3]));
          if (index < 0) return error(404, "not_found", "Member not found");
          project.members.splice(index, 1);
          for (const task of tasks) {
            if (task.projectId === project.id && task.assigneeId === Number(path[3])) {
              task.assigneeId = null;
              task.version++;
              task.updatedAt = new Date(now()).toISOString();
            }
          }
          return response(null, 204);
        }
      }
      const taskReply = handleTasks({
        path, method, body, request, url, tasks, projects, users, userId, token: token ?? null,
        idempotency,
        now: now(), sequence: () => ++sequence,
      });
      if (taskReply) return taskReply;
      return error(404, "not_found", "Route not found");
    },
  };
}

