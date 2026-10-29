import type { Ctx } from "./context";
import { iso, taskDto, write } from "./context";
import { bad, fail, json } from "./http";
import type { Project, Task } from "./model";
import { validate } from "./tasks";

const etag = (t: Task) => ({ ETag: `"${t.version}"` });

export function patch(c: Ctx, t: Task, p: Project): Response {
  const allowed = ["title", "description", "priority", "labels", "assigneeId"];
  for (const k of Object.keys(c.body)) if (!allowed.includes(k)) bad(k);
  const v = validate({ ...t, ...c.body }, p, c);
  const changes: any[] = [];
  for (const k of allowed)
    if (k in c.body) {
      const old = (t as any)[k],
        next = (v as any)[k];
      if (JSON.stringify(old) !== JSON.stringify(next))
        changes.push({ field: k, from: old, to: next });
    }
  changes.sort((a, b) => a.field.localeCompare(b.field));
  for (const ch of changes) {
    (t as any)[ch.field] = ch.to;
    t.history.push({ at: iso(c), by: c.user.id, ...ch });
  }
  if (changes.length) {
    t.version++;
    t.updatedAt = iso(c);
  }
  return json(taskDto(t), 200, etag(t));
}
export function transition(c: Ctx, t: Task): Response {
  const to = c.body.to;
  if (!["open", "in_progress", "done"].includes(to)) bad("to");
  const allowed: Record<string, string[]> = {
    open: ["in_progress"],
    in_progress: ["open", "done"],
    done: ["open"],
  };
  if (!allowed[t.status]!.includes(to))
    fail(409, "invalid_transition", "Invalid transition");
  if (
    to === "done" &&
    [...t.deps].some((k) =>
      c.s.tasks.some((x) => x.key === k && x.status !== "done"),
    )
  ) {
    fail(409, "blocked", "Dependencies are not done");
  }
  const from = t.status;
  t.status = to;
  t.version++;
  t.updatedAt = iso(c);
  t.history.push({ at: iso(c), by: c.user.id, field: "status", from, to });
  return json(taskDto(t), 200, etag(t));
}
export function comments(c: Ctx, t: Task, method: string): Response {
  if (method === "GET") return json(t.comments);
  if (method !== "POST") return undefined as never;
  const body = c.body.body;
  if (typeof body !== "string" || !body.trim() || body.length > 2000)
    bad("body");
  const comment = {
    id: t.comments.length + 1,
    authorId: c.user.id,
    body: body.trim(),
    createdAt: iso(c),
  };
  t.comments.push(comment);
  return json(comment, 201);
}
export function dependencies(
  c: Ctx,
  t: Task,
  method: string,
  key: string,
): Response {
  if (method === "GET") {
    const sort = (a: string, b: string) =>
      Number(a.split("-").pop()) - Number(b.split("-").pop());
    return json({
      key: t.key,
      dependsOn: [...t.deps].sort(sort),
      blocks: c.s.tasks
        .filter((x) => x.deps.has(t.key) && !x.deleted)
        .map((x) => x.key)
        .sort(sort),
    });
  }
  if (method === "DELETE") {
    if (!t.deps.delete(key)) fail(404, "not_found", "Dependency not found");
    return new Response(null, { status: 204 });
  }
  if (method !== "POST") return undefined as never;
  const dep = c.s.tasks.find((x) => x.key === c.body.dependsOn && !x.deleted);
  const p = c.s.projects.find((x) => x.id === t.projectId)!;
  write(p, c.user.id);
  if (!dep) return bad("dependsOn");
  if (dep.projectId !== t.projectId || dep === t) bad("dependsOn");
  if (t.deps.has(dep.key)) fail(409, "conflict", "Dependency exists");
  const reaches = (
    from: Task,
    target: string,
    seen = new Set<string>(),
  ): boolean => {
    if (from.key === target) return true;
    if (seen.has(from.key)) return false;
    seen.add(from.key);
    return [...from.deps].some((k) => {
      const x = c.s.tasks.find((y) => y.key === k);
      return !!x && reaches(x, target, seen);
    });
  };
  if (reaches(dep, t.key)) fail(409, "dependency_cycle", "Dependency cycle");
  t.deps.add(dep.key);
  return json({ key: t.key, dependsOn: [...t.deps], blocks: [] }, 201);
}
