import type { State, Task } from "./model";
import { access, answer, body, fail, findTask, member, stamp, taskReply } from "./common";
import { create, validate } from "./task_helpers";
export async function taskRoutes(s: State, method: string, path: string, url: URL,
  uid: number, req: Request) {
  const b = method === "GET" || method === "DELETE" ? {} : await body(req);
  const tm = path.match(/^\/tasks\/([A-Z]+-\d+)(?:\/(.*))?$/);
  if (tm) {
    const t = s.tasks.find(x => x.key === tm[1]);
    if (!t || !access(s, t.projectId, uid)) return fail(404, "not_found");
    const extra = tm[2] ?? "";
    if (extra === "" && method === "GET") return taskReply(t, s);
    if (extra === "history" && method === "GET") return answer(t.history);
    if (extra === "comments") {
      const list = s.comments.get(t.key) ?? [];
      if (method === "GET") return answer(list);
      if (method === "POST") {
        const text = typeof b?.body === "string" ? b.body.trim() : "";
        if (!text || text.length > 2000) return fail(400, "validation", "Invalid body", "body");
        const item = { id: list.length + 1, authorId: uid, body: text, createdAt: stamp(s) };
        list.push(item); s.comments.set(t.key, list);
        return answer(item, 201);
      }
    }
    if (extra === "" && method === "DELETE") {
      if (member(s, t.projectId, uid) !== "owner" && t.createdBy !== uid)
        return fail(403, "forbidden");
      s.tasks.splice(s.tasks.indexOf(t), 1);
      s.tasks.forEach(x => { x.deps = x.deps.filter(k => k !== t.key); });
      return answer(null, 204);
    }
    if (extra === "" && method === "PATCH") {
      if (!access(s, t.projectId, uid, true)) return fail(403, "forbidden");
      if (!b) return fail(400, "validation");
      const allowed = ["title", "description", "priority", "labels", "assigneeId"];
      const unknown = Object.keys(b ?? {}).find(k => !allowed.includes(k));
      if (unknown) return fail(400, "validation", "Unknown field", unknown);
      const err = validate(b);
      if (err) return fail(400, "validation", "Invalid task", err);
      const match = req.headers.get("If-Match");
      if (match && match !== `"${t.version}"`) return fail(412, "precondition_failed");
      const fields = [...allowed].sort();
      let changed = false;
      for (const field of fields) {
        if (!(field in (b ?? {}))) continue;
        let value = b[field];
        if (field === "title") value = value.trim();
        if (field === "labels") value = [...new Set(value.map((x: string) =>
          x.trim().toLowerCase()))].sort();
        if (field === "assigneeId" && value !== null && !access(s, t.projectId, value))
          return fail(400, "validation", "Invalid assignee", "assigneeId");
        if (JSON.stringify((t as any)[field]) !== JSON.stringify(value)) {
          t.history.push({ at: stamp(s), by: uid, field, from: (t as any)[field], to: value });
          (t as any)[field] = value; changed = true;
        }
      }
      if (changed) { t.version++; t.updatedAt = stamp(s); }
      return taskReply(t, s);
    }
    if (extra === "transitions" && method === "POST") {
      if (!access(s, t.projectId, uid, true)) return fail(403, "forbidden");
      if (!b) return fail(400, "validation");
      if (!["open", "in_progress", "done"].includes(b?.to))
        return fail(400, "validation", "Invalid status", "to");
      const match = req.headers.get("If-Match");
      if (match && match !== `"${t.version}"`) return fail(412, "precondition_failed");
      const next: Record<string, string[]> = {
        open: ["in_progress"], in_progress: ["open", "done"], done: ["open"],
      };
      if (!next[t.status]?.includes(b.to)) return fail(409, "invalid_transition");
      if (b.to === "done" && t.deps.some(key => s.tasks.find(x => x.key === key)?.status !== "done"))
        return fail(409, "blocked");
      t.history.push({ at: stamp(s), by: uid, field: "status", from: t.status, to: b.to });
      t.status = b.to; t.version++; t.updatedAt = stamp(s);
      return taskReply(t, s);
    }
    if (extra === "dependencies") {
      const others = s.tasks.filter(x => x.projectId === t.projectId);
      const result = () => ({ key: t.key, dependsOn: [...t.deps].sort((a,b) =>
        Number(a.split("-").at(-1)) - Number(b.split("-").at(-1))),
        blocks: others.filter(x => x.deps.includes(t.key)).map(x => x.key)
          .sort((a,b) => Number(a.split("-").at(-1)) - Number(b.split("-").at(-1))) });
      if (method === "GET") return answer(result());
      if (method === "POST") {
        if (!access(s, t.projectId, uid, true)) return fail(403, "forbidden");
        if (!b) return fail(400, "validation");
        const other = s.tasks.find(x => x.key === b?.dependsOn);
        if (!other || other.projectId !== t.projectId || other === t)
          return fail(400, "validation", "Invalid dependency", "dependsOn");
        if (t.deps.includes(other.key)) return fail(409, "conflict");
        const reaches = (from: Task, target: string, seen = new Set<string>()): boolean =>
          from.deps.includes(target) || from.deps.some(k => {
            if (seen.has(k)) return false;
            seen.add(k);
            const next = s.tasks.find(x => x.key === k);
            return !!next && reaches(next, target, seen);
          });
        if (reaches(other, t.key)) return fail(409, "dependency_cycle");
        t.deps.push(other.key); return answer(result(), 201);
      }
    }
    const del = extra.match(/^dependencies\/([A-Z]+-\d+)$/);
    if (del && method === "DELETE") {
      if (!access(s, t.projectId, uid, true)) return fail(403, "forbidden");
      if (!t.deps.includes(del[1]!)) return fail(404, "not_found");
      t.deps = t.deps.filter(x => x !== del[1]);
      return answer(null, 204);
    }
  }
  let m = path.match(/^\/projects\/(\d+)\/(tasks|stats)(?:\/(bulk))?$/);
  if (m && m[2] === "tasks") {
    const p = +m[1]!, role = member(s, p, uid);
    if (!role) return fail(404, "not_found");
    if (method === "POST") {
      if (!access(s, p, uid, true)) return fail(403, "forbidden");
      if (m[3] === "bulk") {
        if (!b) return fail(400, "validation", "Invalid tasks", "tasks");
        const items = b?.tasks;
        if (!Array.isArray(items) || !items.length || items.length > 50)
          return fail(400, "validation", "Invalid tasks", "tasks");
        const bad = items.map((x, i) => validate(x, `tasks[${i}].`)).find(x => x);
        if (bad) return fail(400, "validation", "Invalid task", bad);
        return answer({ items: items.map(x => {
          const t = create(s, p, uid, x);
          const { deps, history, ...item } = t;
          return item;
        }) }, 201);
      }
      if (!b) return fail(400, "validation");
      const err = validate(b);
      if (err || !b?.title) return fail(400, "validation", "Invalid task", err ?? "title");
      if (b.assigneeId != null && !access(s, p, b.assigneeId))
        return fail(400, "validation", "Invalid assignee", "assigneeId");
      const idem = req.headers.get("Idempotency-Key");
      const ik = `${uid}:${idem}`;
      if (idem) {
        const old = s.idem.get(ik);
        if (old && s.now() - old.at < 86400000) {
          if (old.body !== JSON.stringify(b)) return fail(422, "idempotency_mismatch");
          const { deps, history, ...item } = old.task;
          return answer(item, 201, { ETag: '"1"', "Idempotent-Replayed": "true" });
        }
      }
      const t = create(s, p, uid, b);
      if (idem) s.idem.set(ik, { body: JSON.stringify(b), task: t, at: s.now() });
      return taskReply(t, s, 201);
    }
    if (method === "GET") {
      let list = s.tasks.filter(t => t.projectId === p);
      const q = url.searchParams;
      if (q.has("status") && !["open", "in_progress", "done"].includes(q.get("status")!))
        return fail(400, "validation", "Invalid status", "status");
      if (q.has("sort") && !["-created", "priority"].includes(q.get("sort")!))
        return fail(400, "validation", "Invalid sort", "sort");
      if (q.has("cursor") && !/^\d+$/.test(q.get("cursor")!))
        return fail(400, "validation", "Invalid cursor", "cursor");
      if (q.has("assignee") && !["me", "none"].includes(q.get("assignee")!) &&
        !/^\d+$/.test(q.get("assignee")!))
        return fail(400, "validation", "Invalid assignee", "assignee");
      if (q.has("status")) list = list.filter(t => t.status === q.get("status"));
      if (q.has("label")) list = list.filter(t => t.labels.includes(q.get("label")!.toLowerCase()));
      if (q.has("q")) list = list.filter(t =>
        `${t.title} ${t.description}`.toLowerCase().includes(q.get("q")!.toLowerCase()));
      if (q.has("assignee")) {
        const a = q.get("assignee")!;
        list = list.filter(t => a === "none" ? t.assigneeId === null :
          t.assigneeId === (a === "me" ? uid : Number(a)));
      }
      if (q.get("sort") === "-created") list.reverse();
      if (q.get("sort") === "priority") list.sort((a, b) =>
        ["high", "normal", "low"].indexOf(a.priority) -
        ["high", "normal", "low"].indexOf(b.priority) || a.number - b.number);
      const limit = Number(q.get("limit") ?? 20), start = Number(q.get("cursor") ?? 0);
      if (!Number.isInteger(limit) || limit < 1 || limit > 100 || start < 0)
        return fail(400, "validation", "Invalid limit", "limit");
      const items = list.slice(start, start + limit);
      return answer({ items: items.map(t => { const { deps, history, ...x } = t; return x; }),
        nextCursor: start + limit < list.length ? String(start + limit) : null });
    }
  }
  m = path.match(/^\/projects\/(\d+)\/stats$/);
  if (m && method === "GET") {
    const p = +m[1]!;
    if (!access(s, p, uid)) return fail(404, "not_found");
    const ts = s.tasks.filter(t => t.projectId === p);
    const counts = (key: string, values: string[]) => Object.fromEntries(values.map(v =>
      [v, ts.filter(t => (t as any)[key] === v).length]));
    const assignees = new Map<number, number>();
    ts.forEach(t => { if (t.assigneeId) assignees.set(t.assigneeId, (assignees.get(t.assigneeId) ?? 0) + 1); });
    return answer({ total: ts.length, byStatus: counts("status", ["open", "in_progress", "done"]),
      byPriority: counts("priority", ["high", "normal", "low"]),
      byAssignee: [...assignees].sort((a,b) => b[1]-a[1] || a[0]-b[0])
        .map(([userId,count]) => ({ userId,count })) });
  }
  return null;
}
