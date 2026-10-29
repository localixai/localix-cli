import { Store, Task, User } from "./store";
import { fail, object, Fault } from "./http";
import { accounts } from "./routes-users";
import { projects } from "./routes-projects";
import { tasks } from "./routes-tasks";

export type Ctx = {
  db: Store; path: string; method: string; query: URLSearchParams;
  body: Record<string, any>; uid?: number; token: string; headers: Headers;
};
export type Result = { status: number; body?: any; headers?: Record<string, string> };
export async function dispatch(c: Ctx): Promise<Result> {
  const { path, method, db } = c;
  if (path === "/users") {
    if (method !== "POST") return methodError("POST");
    return accounts(c);
  }
  if (path === "/sessions") return accounts(c);
  if (path === "/projects" && method !== "GET" && method !== "POST") return methodError("GET, POST");
  if (path === "/me" || path === "/sessions/current") {
    if (method !== "GET" && method !== "DELETE") return methodError("GET, DELETE");
    const uid = auth(c);
    if (method === "DELETE") { db.tokens.delete(c.token); return { status: 204 }; }
    return { status: 200, body: publicUser(db.users.find(u => u.id === uid)!) };
  }
  if (path.startsWith("/projects")) return await projects(c);
  if (path.startsWith("/tasks")) return await tasks(c);
  fail(404, "not_found", "Not found");
}
export function auth(c: Ctx): number {
  if (c.uid === undefined) fail(401, "unauthorized", "Authentication required");
  return c.uid;
}
export function publicUser(u: User) { return { id: u.id, email: u.email, name: u.name }; }
export function methodError(allow: string): never {
  throw new Fault(405, "method_not_allowed", "Method not allowed", undefined, allow);
}
export function member(c: Ctx, pid: number, write = false) {
  const uid = auth(c), role = c.db.role(pid, uid);
  if (!role) fail(404, "not_found", "Project not found");
  if (write && role === "viewer") fail(403, "forbidden", "Forbidden");
  return role;
}
export function validation(field: string): never {
  fail(400, "validation", "Invalid " + field, field);
}
export function body(c: Ctx) { return object(c.body); }
