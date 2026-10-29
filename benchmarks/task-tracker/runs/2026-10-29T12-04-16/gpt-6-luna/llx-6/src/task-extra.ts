import { type Ctx, auth, body, changed, etag, fail, match, noContent, ok, taskByKey, valid } from "./http";
import { iso } from "./model";
import { view } from "./task-core";
export async function taskExtra(c: Ctx, path: string, method: string): Promise<Response | null> {
  let m = path.match(/^\/tasks\/([^/]+)\/(transitions|comments|history|dependencies)(?:\/([^/]+))?$/);
  if (!m) return null;
  const key = m[1], op = m[2], tail = m[3];
  if (!key || !op) return null;
  const t = taskByKey(c, key);
  if (op === "history" && method === "GET") return ok(c.s.history.get(key) ?? []);
  if (op === "comments") {
    const list = c.s.comments.get(key)!;
    if (method === "GET") return ok(list);
    if (method === "POST") {
      const user = auth(c), b = await body(c);
      valid(typeof b.body === "string" && b.body.trim().length > 0 && b.body.length <= 2000, "body");
      const item = { id: list.length + 1, authorId: user, body: b.body.trim(), createdAt: iso(c.s) };
      list.push(item); return ok(item, 201);
    }
  }
  if (op === "transitions" && method === "POST") {
    const user = auth(c, "member", t.projectId); match(c, t);
    const b = await body(c);
    valid(["open", "in_progress", "done"].includes(b.to), "to");
    const allowed: Record<string, string[]> = { open: ["in_progress"], in_progress: ["open", "done"], done: ["open"] };
    if (!allowed[t.status]?.includes(b.to)) fail(409, "invalid_transition", "Invalid transition");
    if (b.to === "done" && t.deps.some((k: string) => c.s.tasks.some((x) => x.key === k && x.status !== "done"))) {
      fail(409, "blocked", "Dependencies are incomplete");
    }
    changed(c, t, user, "status", t.status, b.to);
    t.status = b.to; t.version++; t.updatedAt = iso(c.s);
    return ok(view(t), 200, etag(t));
  }
  if (op === "dependencies") {
    if (method === "GET") {
      const dependsOn = [...t.deps].sort((a, b) => c.s.tasks.find((x) => x.key === a)!.number -
        c.s.tasks.find((x) => x.key === b)!.number);
      const blocks = c.s.tasks.filter((x) => x.deps.includes(key)).sort((a, b) => a.number - b.number)
        .map((x) => x.key);
      return ok({ key, dependsOn, blocks });
    }
    if (method === "POST") {
      auth(c, "member", t.projectId); const b = await body(c);
      const d = c.s.tasks.find((x) => x.key === b.dependsOn);
      valid(typeof b.dependsOn === "string" && !!d && d.projectId === t.projectId && d !== t,
        "dependsOn");
      if (!d) return null;
      if (t.deps.includes(d.key)) fail(409, "conflict", "Already depends on task");
      const reaches = (key: string, seen = new Set<string>()): boolean => {
        if (key === t.key) return true;
        if (seen.has(key)) return false;
        seen.add(key);
        return c.s.tasks.find((x) => x.key === key)?.deps.some((x) => reaches(x, seen)) ?? false;
      };
      if (reaches(d.key)) fail(409, "dependency_cycle", "Dependency cycle");
      t.deps.push(d.key); return ok({ key, dependsOn: t.deps, blocks: [] }, 201);
    }
    if (method === "DELETE" && tail) {
      auth(c, "member", t.projectId);
      if (!t.deps.includes(tail)) fail(404, "not_found", "Dependency not found");
      t.deps = t.deps.filter((x) => x !== tail); return noContent();
    }
  }
  return null;
}
