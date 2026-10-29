import { err, iso, key, ok, publicTask, type Project, type State, type Task } from "./model";
import { validateTask } from "./task-input";

const projectOf = (s: State, id: number) => s.projects.find((p) => p.id === id);
const role = (s: State, p: Project, u: number) =>
  u === p.ownerId ? "owner" : s.memberships.get(key(p.id, u));
const member = (s: State, p: Project, u: number) => !!role(s, p, u);
export function taskRoute(
  s: State,
  m: string,
  path: string,
  b: any,
  u: number | undefined,
  token: string,
  headers: Headers,
): Response | null {
  const mt = path.match(/^\/tasks\/([A-Z]+-\d+)(?:\/(.*))?$/);
  if (!mt) return null;
  const t = s.tasks.find((x) => {
    const p = projectOf(s, x.projectId);
    return p && `${p.key}-${x.number}` === mt[1] && !x.deleted;
  });
  if (!t) return err("not_found", "not found", 404);
  const p = projectOf(s, t.projectId)!;
  if (!u) return err("unauthorized", "unauthorized", 401);
  if (!member(s, p, u)) return err("not_found", "not found", 404);
  const sub = mt[2] ?? "";
  if (!sub && m === "GET") return ok(publicTask(t, p), 200, { ETag: `"${t.version}"` });
  if (!sub && m === "DELETE") {
    if (role(s, p, u) !== "owner" && t.createdBy !== u) return err("forbidden", "forbidden", 403);
    t.deleted = true;
    for (const x of s.tasks) x.deps = x.deps.filter((id) => id !== t.id);
    return new Response(null, { status: 204 });
  }
  if ((!sub && m === "PATCH") || (sub === "transitions" && m === "POST")) {
    if (role(s, p, u) === "viewer") return err("forbidden", "forbidden", 403);
    const ifMatch = headers.get("If-Match") ?? undefined;
    if (ifMatch && ifMatch !== `"${t.version}"`)
      return err("precondition_failed", "version changed", 412);
    const before = { ...t, labels: [...t.labels] };
    let changes: any = {};
    if (sub === "transitions") {
      if (!["open", "in_progress", "done"].includes(b.to))
        return err("validation", "invalid status", 400, "to");
      const allowed =
        (t.status === "open" && b.to === "in_progress") ||
        (t.status === "in_progress" && ["open", "done"].includes(b.to)) ||
        (t.status === "done" && b.to === "open");
      if (!allowed) return err("invalid_transition", "invalid transition", 409);
      if (
        b.to === "done" &&
        t.deps.some((id) => {
          const d = s.tasks.find((x) => x.id === id);
          return d && !d.deleted && d.status !== "done";
        })
      )
        return err("blocked", "blocked", 409);
      changes = { status: b.to };
    } else {
      for (const k of Object.keys(b))
        if (!["title", "description", "priority", "labels", "assigneeId"].includes(k))
          return err("validation", "unknown field", 400, k);
      const f = validateTask({ ...t, ...b });
      if (f) return err("validation", "invalid task", 400, f);
      if (b.assigneeId !== undefined && b.assigneeId !== null && !member(s, p, b.assigneeId))
        return err("validation", "invalid assignee", 400, "assigneeId");
      changes = { ...b };
      if (b.labels)
        changes.labels = [...new Set(b.labels.map((x: string) => x.trim().toLowerCase()))].sort();
      if (b.title) changes.title = b.title.trim();
    }
    const fields = Object.keys(changes)
      .sort()
      .filter((k) => JSON.stringify((t as any)[k]) !== JSON.stringify(changes[k]));
    if (fields.length) {
      for (const f of fields) {
        t.history.push({ at: iso(s), by: u, field: f, from: (before as any)[f], to: changes[f] });
        (t as any)[f] = changes[f];
      }
      t.version++;
      t.updatedAt = iso(s);
    }
    return ok(publicTask(t, p), 200, { ETag: `"${t.version}"` });
  }
  if (sub === "history" && m === "GET") return ok(t.history);
  if (sub === "dependencies" && m === "GET")
    return ok({
      key: mt[1],
      dependsOn: t.deps
        .map((id) => `${p.key}-${s.tasks.find((x) => x.id === id)?.number}`)
        .sort((a, b) => Number(a.split("-")[1]) - Number(b.split("-")[1])),
      blocks: s.tasks
        .filter((x) => x.deps.includes(t.id) && !x.deleted)
        .map((x) => `${p.key}-${x.number}`)
        .sort((a, b) => Number(a.split("-")[1]) - Number(b.split("-")[1])),
    });
  if (sub === "dependencies" && m === "POST") {
    if (role(s, p, u) === "viewer") return err("forbidden", "forbidden", 403);
    const d = s.tasks.find((x) => `${p.key}-${x.number}` === b.dependsOn && !x.deleted);
    if (!d || d.id === t.id || d.projectId !== t.projectId)
      return err("validation", "invalid dependency", 400, "dependsOn");
    if (t.deps.includes(d.id)) return err("conflict", "dependency exists", 409);
    const reaches = (from: number, target: number, seen = new Set<number>()): boolean => {
      if (from === target) return true;
      if (seen.has(from)) return false;
      seen.add(from);
      return (s.tasks.find((x) => x.id === from)?.deps ?? []).some((x) => reaches(x, target, seen));
    };
    if (reaches(d.id, t.id)) return err("dependency_cycle", "dependency cycle", 409);
    t.deps.push(d.id);
    return ok(
      {
        key: mt[1],
        dependsOn: t.deps.map((id) => `${p.key}-${s.tasks.find((x) => x.id === id)?.number}`),
        blocks: [],
      },
      201,
    );
  }
  const del = sub.match(/^dependencies\/(.+)$/);
  if (del && m === "DELETE") {
    const d = s.tasks.find((x) => `${p.key}-${x.number}` === del[1]);
    if (!d || !t.deps.includes(d.id)) return err("not_found", "not found", 404);
    t.deps = t.deps.filter((x) => x !== d.id);
    return new Response(null, { status: 204 });
  }
  if (sub === "comments" && m === "GET") return ok(s.comments.get(t.id) ?? []);
  if (sub === "comments" && m === "POST") {
    const text = typeof b.body === "string" ? b.body.trim() : "";
    if (!text || text.length > 2000) return err("validation", "invalid body", 400, "body");
    let list = s.comments.get(t.id);
    if (!list) s.comments.set(t.id, (list = []));
    const item = {
      id: s.tasks.reduce((n, x) => n + (s.comments.get(x.id)?.length ?? 0), 0) + 1,
      authorId: u,
      body: text,
      createdAt: iso(s),
    };
    list.push(item);
    return ok(item, 201);
  }
  return err("not_found", "not found", 404);
}
